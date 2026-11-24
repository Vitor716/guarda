//! Cobertura (Fase 2): a guarda não mede cobertura (isso exige a JVM e o agente do JaCoCo);
//! ela decide quando rodar os testes, lê o XML do JaCoCo/Kover em streaming e aplica o gate.
//!
//! Conta como o Sonar: cobertura = (linhas cobertas + desvios cobertos) /
//! (linhas executáveis + desvios). Linha executável é a que aparece no XML com instrução.
//!
//! - COV001: código novo (linhas do diff contra a referência) abaixo do mínimo.
//! - COV002: pacote abaixo do mínimo; COV003: projeto abaixo do mínimo (catraca).
//! - COV000: os testes falharam no `--cobertura`.
//!
//! Arquivo alterado depois do relatório não entra na conta (o XML não fala dele): vira nota
//! pedindo `--cobertura`.

use crate::arquitetura::{Padrao, Projeto};
use crate::codigo_novo::LinhasNovas;
use crate::config::{Cobertura, Relatorios};
use crate::modelo::Violacao;
use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use quick_xml::Reader;
use quick_xml::events::Event;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Linha {
    pub mi: u32,
    pub ci: u32,
    pub mb: u32,
    pub cb: u32,
}

impl Linha {
    fn executavel(&self) -> bool {
        self.mi + self.ci > 0
    }
    fn sem_teste(&self) -> bool {
        self.executavel() && self.ci == 0
    }
    fn desvio_parcial(&self) -> bool {
        self.mb > 0 && (self.cb > 0 || self.ci > 0)
    }
}

/// `pacote/Arquivo.java` (como no XML) → linhas.
pub type Linhas = HashMap<String, BTreeMap<usize, Linha>>;

pub struct Relatorio {
    pub arquivos: Linhas,
    /// Data do relatório mais antigo lido: arquivo mais novo que isso está desatualizado.
    pub gerado: SystemTime,
    pub caminhos: Vec<String>,
}

/// Onde o JaCoCo e o Kover gravam o XML, a partir de cada módulo.
const CANDIDATOS: &[&str] = &[
    "target/site/jacoco/jacoco.xml",
    "target/site/jacoco-aggregate/jacoco.xml",
    "target/site/jacoco-ut/jacoco.xml",
    "build/reports/jacoco/test/jacocoTestReport.xml",
    "build/reports/kover/report.xml",
    "build/reports/kover/xml/report.xml",
];

/// Diretórios de módulo (onde há pom.xml ou build.gradle[.kts]), a raiz primeiro.
fn modulos(raiz: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = ignore::WalkBuilder::new(raiz)
        .hidden(true)
        .require_git(false)
        .build()
        .flatten()
        .filter(|e| {
            matches!(
                e.file_name().to_str(),
                Some("pom.xml" | "build.gradle" | "build.gradle.kts")
            )
        })
        .filter_map(|e| e.path().parent().map(Path::to_path_buf))
        .collect();
    v.sort();
    v.dedup();
    if v.is_empty() {
        v.push(raiz.to_path_buf());
    }
    v
}

pub fn achar(raiz: &Path, cfg: &Cobertura) -> Vec<PathBuf> {
    match &cfg.relatorio {
        Relatorios::Varios(v) => v
            .iter()
            .map(|p| raiz.join(p))
            .filter(|p| p.is_file())
            .collect(),
        Relatorios::Um(p) if p != "auto" => {
            let c = raiz.join(p);
            if c.is_file() { vec![c] } else { Vec::new() }
        }
        Relatorios::Um(_) => modulos(raiz)
            .iter()
            .flat_map(|m| CANDIDATOS.iter().map(move |c| m.join(c)))
            .filter(|p| p.is_file())
            .collect(),
    }
}

fn atributo(e: &quick_xml::events::BytesStart, nome: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == nome)
        .map(|a| a.value.to_string())
}

/// Lê o XML do JaCoCo (o Kover usa o mesmo formato) em streaming.
pub fn ler_jacoco(xml: &str) -> Result<Linhas> {
    let mut r = Reader::from_str(xml);
    let mut pacote = String::new();
    let mut arquivo: Option<String> = None;
    let mut m: Linhas = HashMap::new();
    loop {
        match r.read_event().context("XML de cobertura inválido")? {
            Event::Start(e) | Event::Empty(e) => match e.name().as_ref() {
                "package" => pacote = atributo(&e, "name").unwrap_or_default(),
                "sourcefile" => {
                    let nome = atributo(&e, "name").unwrap_or_default();
                    arquivo = Some(if pacote.is_empty() {
                        nome
                    } else {
                        format!("{pacote}/{nome}")
                    });
                }
                "line" => {
                    if let Some(a) = &arquivo {
                        let n = |k: &str| atributo(&e, k).and_then(|v| v.parse().ok()).unwrap_or(0);
                        let nr: usize =
                            atributo(&e, "nr").and_then(|v| v.parse().ok()).unwrap_or(0);
                        let l = Linha {
                            mi: n("mi"),
                            ci: n("ci"),
                            mb: n("mb"),
                            cb: n("cb"),
                        };
                        let atual = m.entry(a.clone()).or_default().entry(nr).or_default();
                        // Vários relatórios do mesmo arquivo: vale o mais coberto.
                        if l.ci + l.cb >= atual.ci + atual.cb {
                            *atual = l;
                        }
                    }
                }
                _ => {}
            },
            Event::End(e) => match e.name().as_ref() {
                "sourcefile" => arquivo = None,
                "package" => pacote.clear(),
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(m)
}

pub fn carregar(raiz: &Path, cfg: &Cobertura) -> Result<Option<Relatorio>> {
    let caminhos = achar(raiz, cfg);
    if caminhos.is_empty() {
        return Ok(None);
    }
    let mut arquivos: Linhas = HashMap::new();
    let mut gerado = SystemTime::now();
    for c in &caminhos {
        let xml = std::fs::read_to_string(c).with_context(|| format!("lendo {}", c.display()))?;
        for (k, linhas) in ler_jacoco(&xml)? {
            let destino = arquivos.entry(k).or_default();
            for (nr, l) in linhas {
                let atual = destino.entry(nr).or_default();
                if l.ci + l.cb >= atual.ci + atual.cb {
                    *atual = l;
                }
            }
        }
        if let Ok(t) = std::fs::metadata(c).and_then(|m| m.modified()) {
            gerado = gerado.min(t);
        }
    }
    let caminhos = caminhos
        .iter()
        .map(|c| {
            c.strip_prefix(raiz)
                .unwrap_or(c)
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    Ok(Some(Relatorio {
        arquivos,
        gerado,
        caminhos,
    }))
}

/// Comando que roda os testes e gera o XML, pelo build do projeto.
pub fn comando_de_testes(raiz: &Path, cfg: &Cobertura) -> Option<String> {
    if let Some(c) = &cfg.comando {
        return Some(c.clone());
    }
    let windows = cfg!(windows);
    let gradle = raiz.join("gradlew").is_file() || raiz.join("gradlew.bat").is_file();
    if gradle {
        let textos: String = ["build.gradle", "build.gradle.kts"]
            .iter()
            .filter_map(|f| std::fs::read_to_string(raiz.join(f)).ok())
            .collect();
        let tarefa = if textos.contains("kover") {
            "koverXmlReport"
        } else {
            "jacocoTestReport"
        };
        let gw = if windows { "gradlew.bat" } else { "./gradlew" };
        return Some(format!("{gw} test {tarefa}"));
    }
    if raiz.join("pom.xml").is_file() {
        let mvn = if raiz.join("mvnw").is_file() || raiz.join("mvnw.cmd").is_file() {
            if windows { "mvnw.cmd" } else { "./mvnw" }
        } else {
            "mvn"
        };
        return Some(format!("{mvn} -q test jacoco:report"));
    }
    None
}

/// Roda os testes. `Ok(None)` se passaram; `Ok(Some(saída))` com o fim da saída se falharam.
pub fn rodar_testes(raiz: &Path, cfg: &Cobertura) -> Result<Option<String>> {
    let cmd = comando_de_testes(raiz, cfg)
        .context("não sei rodar os testes deste projeto: defina cobertura.comando")?;
    let out = if cfg!(windows) {
        Command::new("cmd")
            .args(["/C", &cmd])
            .current_dir(raiz)
            .output()
    } else {
        Command::new("sh")
            .args(["-c", &cmd])
            .current_dir(raiz)
            .output()
    }
    .with_context(|| format!("não consegui rodar `{cmd}`"))?;
    if out.status.success() {
        return Ok(None);
    }
    let mut t = String::from_utf8_lossy(&out.stdout).to_string();
    t.push_str(&String::from_utf8_lossy(&out.stderr));
    let fim: Vec<&str> = t.lines().filter(|l| !l.trim().is_empty()).collect();
    let fim = fim[fim.len().saturating_sub(15)..].join("\n");
    Ok(Some(format!("`{cmd}` falhou:\n{fim}")))
}

#[derive(Default, Clone, Copy)]
struct Conta {
    linhas: u32,
    cobertas: u32,
    desvios: u32,
    desvios_cobertos: u32,
}

impl Conta {
    fn somar(&mut self, l: &Linha) {
        if l.executavel() {
            self.linhas += 1;
            if l.ci > 0 {
                self.cobertas += 1;
            }
        }
        self.desvios += l.mb + l.cb;
        self.desvios_cobertos += l.cb;
    }
    fn juntar(&mut self, o: &Conta) {
        self.linhas += o.linhas;
        self.cobertas += o.cobertas;
        self.desvios += o.desvios;
        self.desvios_cobertos += o.desvios_cobertos;
    }
    fn pct(&self) -> Option<f64> {
        let total = self.linhas + self.desvios;
        (total > 0).then(|| (self.cobertas + self.desvios_cobertos) as f64 * 100.0 / total as f64)
    }
}

pub fn pct(v: f64) -> String {
    format!("{v:.1}%").replace('.', ",")
}

/// Faixas compactas: 42,43,44,58 → "42-44,58".
pub fn faixas(linhas: &[usize]) -> String {
    let mut partes = Vec::new();
    let mut i = 0;
    while i < linhas.len() {
        let ini = linhas[i];
        let mut fim = ini;
        while i + 1 < linhas.len() && linhas[i + 1] == fim + 1 {
            i += 1;
            fim = linhas[i];
        }
        partes.push(if ini == fim {
            ini.to_string()
        } else {
            format!("{ini}-{fim}")
        });
        i += 1;
    }
    partes.join(",")
}

/// Até 6 faixas; o resto vira `+N`, para a linha caber na saída da IA.
pub fn faixas_curtas(linhas: &[usize]) -> String {
    const MAX: usize = 6;
    let todas = faixas(linhas);
    let partes: Vec<&str> = todas.split(',').collect();
    if partes.len() <= MAX {
        return todas;
    }
    format!("{} +{} faixas", partes[..MAX].join(","), partes.len() - MAX)
}

fn globs(padroes: &[String]) -> GlobSet {
    let mut b = GlobSetBuilder::new();
    for p in padroes {
        if let Ok(g) = Glob::new(p) {
            b.add(g);
        }
    }
    b.build().unwrap_or_else(|_| GlobSet::empty())
}

/// Chave do XML (`pacote/Arquivo.java`) que corresponde ao arquivo do projeto.
fn chave<'a>(rel: &str, por_nome: &'a HashMap<&str, Vec<&'a String>>) -> Option<&'a String> {
    let nome = rel.rsplit('/').next()?;
    por_nome
        .get(nome)?
        .iter()
        .copied()
        .find(|k| rel == k.as_str() || rel.ends_with(&format!("/{k}")))
}

pub struct Avaliacao {
    pub violacoes: Vec<Violacao>,
    pub nota: Option<String>,
}

pub struct Entrada<'a> {
    pub cfg: &'a Cobertura,
    pub raiz: &'a Path,
    pub relatorio: Option<&'a Relatorio>,
    /// Arquivos verificados nesta execução.
    pub analisados: &'a [String],
    pub novas: Option<&'a LinhasNovas>,
    /// Avalia também COV002 e COV003 (projeto inteiro).
    pub globais: bool,
    pub projeto: &'a Projeto,
}

const ROTULO: &str = "[cobertura]";

pub fn avaliar(e: &Entrada) -> Avaliacao {
    let ignorar = globs(&e.cfg.ignorar);
    let mut v = Vec::new();
    let Some(rel) = e.relatorio else {
        let tem_novo = e
            .novas
            .is_some_and(|n| e.analisados.iter().any(|a| n.de(a).is_some()));
        return Avaliacao {
            violacoes: v,
            nota: tem_novo
                .then(|| "sem relatório de cobertura: rode `guarda check --cobertura`".into()),
        };
    };
    let mut por_nome: HashMap<&str, Vec<&String>> = HashMap::new();
    for k in rel.arquivos.keys() {
        por_nome
            .entry(k.rsplit('/').next().unwrap_or(k))
            .or_default()
            .push(k);
    }

    // COV001: código novo.
    let mut desatualizados = 0;
    let mut total = Conta::default();
    let mut por_arquivo: Vec<(&String, Conta, Vec<usize>, Vec<usize>)> = Vec::new();
    if let Some(novas) = e.novas {
        for a in e.analisados {
            if ignorar.is_match(a) {
                continue;
            }
            let Some(linhas_novas) = novas.de(a) else {
                continue;
            };
            let modificado = std::fs::metadata(e.raiz.join(a))
                .and_then(|m| m.modified())
                .ok();
            if modificado.is_some_and(|m| m > rel.gerado) {
                desatualizados += 1;
                continue;
            }
            let Some(k) = chave(a, &por_nome) else {
                continue;
            };
            let linhas = &rel.arquivos[k];
            let mut c = Conta::default();
            let mut sem_teste = Vec::new();
            let mut parciais = Vec::new();
            for n in linhas_novas {
                if let Some(l) = linhas.get(n) {
                    c.somar(l);
                    if l.sem_teste() {
                        sem_teste.push(*n);
                    } else if l.desvio_parcial() {
                        parciais.push(*n);
                    }
                }
            }
            total.juntar(&c);
            por_arquivo.push((a, c, sem_teste, parciais));
        }
    }
    let minimo = e.cfg.codigo_novo.minimo;
    if let Some(p) = total.pct()
        && p < minimo
    {
        // Uma linha com o número do gate; por arquivo, só onde agir.
        v.push(Violacao::nova(
            "COV001",
            ROTULO,
            1,
            format!(
                "código novo {}<{} ({} arquivo(s) com linha sem teste)",
                pct(p),
                pct(minimo),
                por_arquivo
                    .iter()
                    .filter(|x| !x.2.is_empty() || !x.3.is_empty())
                    .count()
            ),
            "cubra as linhas apontadas em cada arquivo",
            "codigo-novo",
            "",
            None,
        ));
        for (a, _, sem_teste, parciais) in &por_arquivo {
            if sem_teste.is_empty() && parciais.is_empty() {
                continue;
            }
            let primeira = sem_teste.first().or(parciais.first()).copied().unwrap_or(1);
            let mut msg = Vec::new();
            if !sem_teste.is_empty() {
                msg.push(format!("l.{} sem teste", faixas_curtas(sem_teste)));
            }
            if !parciais.is_empty() {
                msg.push(format!(
                    "l.{} com desvio sem teste",
                    faixas_curtas(parciais)
                ));
            }
            let dica = if parciais.is_empty() {
                "escreva um teste que passe por essas linhas".to_string()
            } else {
                format!(
                    "teste o outro caminho do if/&&/|| nas l.{}",
                    faixas_curtas(parciais)
                )
            };
            v.push(Violacao::nova(
                "COV001",
                a,
                primeira,
                msg.join(", "),
                dica,
                nome_do_arquivo(a),
                &format!("{}|{}", faixas(sem_teste), faixas(parciais)),
                None,
            ));
        }
    }

    // COV002 e COV003: projeto inteiro, no último relatório.
    if e.globais {
        let mut geral = Conta::default();
        let mut pacotes: Vec<(String, Padrao, f64, Conta)> = e
            .cfg
            .por_pacote
            .iter()
            .filter_map(|(p, m)| {
                Padrao::novo(p)
                    .ok()
                    .map(|pd| (p.clone(), pd, *m, Conta::default()))
            })
            .collect();
        let mut chaves: Vec<&String> = rel.arquivos.keys().collect();
        chaves.sort();
        for k in chaves {
            if ignorar.is_match(k) || ignorar.is_match(format!("x/{k}")) {
                continue;
            }
            let mut c = Conta::default();
            for l in rel.arquivos[k].values() {
                c.somar(l);
            }
            geral.juntar(&c);
            let pacote = k.rsplit_once('/').map_or("", |(p, _)| p).replace('/', ".");
            for (_, padrao, _, conta) in &mut pacotes {
                if padrao.casa_pacote(&pacote, e.projeto) {
                    conta.juntar(&c);
                }
            }
        }
        for (nome, _, minimo, conta) in &pacotes {
            if let Some(p) = conta.pct()
                && p < *minimo
            {
                v.push(cobertura_global(
                    "COV002",
                    &format!("pacote {nome}"),
                    nome,
                    p,
                    *minimo,
                ));
            }
        }
        if let (Some(m), Some(p)) = (e.cfg.geral_minimo, geral.pct())
            && p < m
        {
            v.push(cobertura_global("COV003", "projeto", "geral", p, m));
        }
    }

    let nota = (desatualizados > 0).then(|| {
        format!("cobertura desatualizada em {desatualizados} arquivo(s): rode `guarda check --cobertura`")
    });
    Avaliacao { violacoes: v, nota }
}

fn cobertura_global(
    codigo: &'static str,
    oque: &str,
    simbolo: &str,
    p: f64,
    minimo: f64,
) -> Violacao {
    // Catraca sobre a parte descoberta (por mil): pode cair, nunca subir.
    let descoberto = ((100.0 - p) * 10.0).round() as u32;
    Violacao::nova(
        codigo,
        ROTULO,
        1,
        format!("{oque} {}<{}", pct(p), pct(minimo)),
        "acrescente testes para o que não está coberto (veja o relatório do JaCoCo)",
        simbolo,
        "",
        Some(descoberto),
    )
}

fn nome_do_arquivo(rel: &str) -> &str {
    let nome = rel.rsplit('/').next().unwrap_or(rel);
    nome.split('.').next().unwrap_or(nome)
}

/// Linhas que o relatório considera executáveis, por arquivo do projeto (para o `medir`).
pub fn resumo_geral(rel: &Relatorio) -> Option<f64> {
    let mut c = Conta::default();
    for linhas in rel.arquivos.values() {
        for l in linhas.values() {
            c.somar(l);
        }
    }
    c.pct()
}

#[cfg(test)]
mod testes {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><!DOCTYPE report PUBLIC "-//JACOCO//DTD Report 1.1//EN" "report.dtd"><report name="x"><sessioninfo id="s" start="1" dump="2"/><package name="com/app/calc"><class name="com/app/calc/Icms" sourcefilename="Icms.kt"><method name="f" desc="()V" line="3"><counter type="LINE" missed="1" covered="2"/></method></class><sourcefile name="Icms.kt"><line nr="3" mi="0" ci="4" mb="0" cb="0"/><line nr="4" mi="0" ci="2" mb="1" cb="1"/><line nr="5" mi="3" ci="0" mb="0" cb="0"/><counter type="LINE" missed="1" covered="2"/></sourcefile></package><package name="com/app"><sourcefile name="App.kt"><line nr="1" mi="0" ci="1" mb="0" cb="0"/></sourcefile></package></report>"#;

    #[test]
    fn le_jacoco() {
        let m = ler_jacoco(XML).unwrap();
        assert_eq!(m.len(), 2);
        let icms = &m["com/app/calc/Icms.kt"];
        assert_eq!(
            icms[&4],
            Linha {
                mi: 0,
                ci: 2,
                mb: 1,
                cb: 1
            }
        );
        assert!(icms[&5].sem_teste());
        assert!(icms[&4].desvio_parcial());
    }

    #[test]
    fn conta_como_o_sonar() {
        let m = ler_jacoco(XML).unwrap();
        let mut c = Conta::default();
        for l in m["com/app/calc/Icms.kt"].values() {
            c.somar(l);
        }
        // 3 linhas (2 cobertas) + 2 desvios (1 coberto) = 3/5
        assert_eq!(c.pct(), Some(60.0));
    }

    #[test]
    fn faixas_compactas() {
        assert_eq!(faixas(&[42, 43, 44, 58]), "42-44,58");
        assert_eq!(faixas(&[7]), "7");
        let muitas: Vec<usize> = (0..10).map(|i| i * 3).collect();
        assert_eq!(faixas_curtas(&muitas), "0,3,6,9,12,15 +4 faixas");
    }

    #[test]
    fn acha_a_chave_pelo_sufixo() {
        let m = ler_jacoco(XML).unwrap();
        let mut por_nome: HashMap<&str, Vec<&String>> = HashMap::new();
        for k in m.keys() {
            por_nome
                .entry(k.rsplit('/').next().unwrap())
                .or_default()
                .push(k);
        }
        assert_eq!(
            chave("modulo/src/main/kotlin/com/app/calc/Icms.kt", &por_nome).map(String::as_str),
            Some("com/app/calc/Icms.kt")
        );
        assert!(chave("src/main/kotlin/outro/Icms.kt", &por_nome).is_none());
    }
}
