//! `guarda hook <evento> --agente <agente>`: lê o JSON do evento no stdin, traduz para um
//! formato único, decide e responde no protocolo de cada agente.
//!
//! - `proteger` (antes da ferramenta): recusa mexer em guarda.yml, .guarda/**, no plugin de
//!   cobertura do build e nos hooks da guarda; recusa `guarda baseline`/`guarda init`.
//! - `pos-edicao` (depois da ferramenta): verifica só os arquivos editados.
//! - `final` (antes de encerrar): verifica o que mudou; segura a IA até
//!   `gate.tentativas-final` vezes e depois libera com relatório.
//!
//! Protocolos (conferidos na documentação de cada agente em 2026-11-24):
//!
//! | | Claude Code | Codex | Antigravity |
//! |---|---|---|---|
//! | recusar ferramenta | exit 2 + stderr | exit 2 + stderr | `{"decision":"deny","reason"}` |
//! | corrigir após edição | exit 2 + stderr | exit 2 + stderr | sem canal (não registrado) |
//! | não encerrar | exit 2 + stderr | `{"decision":"block","reason"}` | `{"decision":"continue","reason"}` |
//! | avisar você | `{"systemMessage"}` | `{"systemMessage"}` | `{"decision":"stop","reason"}` |
//!
//! Erro da própria guarda nunca bloqueia: exit 1 (não bloqueante) ou a resposta de liberar.

use crate::baseline;
use crate::config::{self, Config};
use crate::gate::{self, Opcoes, Pedido};
use crate::lang::Linguagem;
use crate::modelo::{Severidade, Violacao};
use crate::saida;
use anyhow::{Result, bail};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agente {
    Claude,
    Codex,
    Antigravity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fase {
    Proteger,
    PosEdicao,
    Final,
}

impl Fase {
    pub fn de_texto(s: &str) -> Result<Self> {
        Ok(match s {
            "proteger" => Self::Proteger,
            "pos-edicao" => Self::PosEdicao,
            "final" => Self::Final,
            outro => bail!("evento desconhecido: {outro} (use proteger, pos-edicao ou final)"),
        })
    }
}

/// O que a guarda decidiu, independente do agente.
#[derive(Debug, PartialEq)]
pub enum Decisao {
    Liberar,
    /// Libera, mas avisa você.
    Avisar(String),
    /// A ferramenta não roda (antes) ou a IA deve corrigir (depois) ou continuar (final).
    Barrar(String),
}

/// A resposta já no protocolo do agente.
pub struct Saida {
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub codigo: u8,
}

pub fn responder(agente: Agente, fase: Fase, d: &Decisao) -> Saida {
    let json_ = |v: Value| Saida {
        stdout: Some(v.to_string()),
        stderr: None,
        codigo: 0,
    };
    let nada = Saida {
        stdout: None,
        stderr: None,
        codigo: 0,
    };
    let exit2 = |t: &str| Saida {
        stdout: None,
        stderr: Some(t.to_string()),
        codigo: 2,
    };
    match (agente, fase, d) {
        (Agente::Antigravity, Fase::Proteger, Decisao::Barrar(t)) => {
            json_(json!({"decision": "deny", "reason": t}))
        }
        (Agente::Antigravity, Fase::Proteger, _) => json_(json!({"decision": "allow"})),
        (Agente::Antigravity, Fase::PosEdicao, _) => json_(json!({})),
        (Agente::Antigravity, Fase::Final, Decisao::Barrar(t)) => {
            json_(json!({"decision": "continue", "reason": t}))
        }
        (Agente::Antigravity, Fase::Final, Decisao::Avisar(t)) => {
            json_(json!({"decision": "stop", "reason": t}))
        }
        (Agente::Antigravity, Fase::Final, Decisao::Liberar) => json_(json!({"decision": "stop"})),
        (Agente::Codex, Fase::Final, Decisao::Barrar(t)) => {
            json_(json!({"decision": "block", "reason": t}))
        }
        (_, _, Decisao::Barrar(t)) => exit2(t),
        (_, _, Decisao::Avisar(t)) => json_(json!({ "systemMessage": t })),
        (_, _, Decisao::Liberar) => nada,
    }
}

/// Resposta segura quando a própria guarda falha: nunca prende a IA.
pub fn responder_erro(agente: Agente, fase: Fase, erro: &str) -> Saida {
    match agente {
        Agente::Antigravity => {
            let mut s = responder(agente, fase, &Decisao::Liberar);
            s.stderr = Some(erro.to_string());
            s
        }
        _ => Saida {
            stdout: None,
            stderr: Some(erro.to_string()),
            codigo: 1,
        },
    }
}

pub fn executar(agente: Agente, fase: Fase) -> Result<Decisao> {
    let mut texto = String::new();
    std::io::stdin().read_to_string(&mut texto)?;
    // Pipes do PowerShell 5.1 podem prefixar um BOM.
    let bruto: Value = serde_json::from_str(texto.trim_start_matches('\u{feff}').trim())?;
    let evento = ler_evento(agente, &bruto);
    match fase {
        Fase::Proteger => proteger(&evento),
        Fase::PosEdicao => pos_edicao(&evento),
        Fase::Final => final_(&evento),
    }
}

// ---------------------------------------------------------------------------------------
// Evento normalizado

#[derive(Debug, Clone, PartialEq)]
pub enum Mudanca {
    /// Trechos que saem ou entram (Edit, replace, linhas +/- de um patch).
    Trechos(Vec<String>),
    /// Conteúdo novo do arquivo inteiro (Write, write_to_file, Add File).
    Conteudo(String),
    Apagar,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Edicao {
    pub caminho: PathBuf,
    pub mudanca: Mudanca,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Acao {
    Edicoes(Vec<Edicao>),
    Shell(String),
    Outra,
}

#[derive(Debug, Clone)]
pub struct Evento {
    pub acao: Acao,
    pub sessao: String,
    pub cwd: Option<PathBuf>,
}

fn texto_de<'a>(v: &'a Value, campo: &str) -> Option<&'a str> {
    v.get(campo).and_then(Value::as_str)
}

fn absoluto(cwd: &Option<PathBuf>, p: &str) -> PathBuf {
    let p = PathBuf::from(p);
    if p.is_absolute() {
        p
    } else {
        cwd.clone().unwrap_or_else(|| PathBuf::from(".")).join(p)
    }
}

/// Strings de um valor JSON (para juntar os trechos de edições com vários pedaços).
fn strings(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => vec![s.clone()],
        Value::Array(a) => a.iter().flat_map(strings).collect(),
        Value::Object(o) => o.values().flat_map(strings).collect(),
        _ => Vec::new(),
    }
}

/// Os argumentos da edição sem o caminho (o caminho não é conteúdo editado).
fn sem_caminho(ti: &Value) -> Value {
    let mut t = ti.clone();
    if let Some(o) = t.as_object_mut() {
        o.shift_remove("file_path");
        o.shift_remove("notebook_path");
    }
    t
}

pub fn ler_evento(agente: Agente, v: &Value) -> Evento {
    match agente {
        Agente::Claude | Agente::Codex => {
            let cwd = texto_de(v, "cwd").map(PathBuf::from);
            let ti = v.get("tool_input").cloned().unwrap_or(Value::Null);
            let caminho = || {
                texto_de(&ti, "file_path")
                    .or_else(|| texto_de(&ti, "notebook_path"))
                    .map(|p| absoluto(&cwd, p))
            };
            let acao = match texto_de(v, "tool_name").unwrap_or("") {
                "Bash" | "PowerShell" | "shell" => {
                    Acao::Shell(texto_de(&ti, "command").unwrap_or("").to_string())
                }
                "Edit" | "MultiEdit" | "NotebookEdit" => match caminho() {
                    Some(c) => Acao::Edicoes(vec![Edicao {
                        caminho: c,
                        mudanca: Mudanca::Trechos(strings(&sem_caminho(&ti))),
                    }]),
                    None => Acao::Outra,
                },
                "Write" => match caminho() {
                    Some(c) => Acao::Edicoes(vec![Edicao {
                        caminho: c,
                        mudanca: Mudanca::Conteudo(
                            texto_de(&ti, "content").unwrap_or("").to_string(),
                        ),
                    }]),
                    None => Acao::Outra,
                },
                "apply_patch" => {
                    let patch = texto_de(&ti, "command")
                        .or_else(|| texto_de(&ti, "patch"))
                        .or_else(|| texto_de(&ti, "input"))
                        .unwrap_or("");
                    Acao::Edicoes(ler_patch(patch, &cwd))
                }
                _ => Acao::Outra,
            };
            Evento {
                acao,
                sessao: texto_de(v, "session_id").unwrap_or("").to_string(),
                cwd,
            }
        }
        Agente::Antigravity => {
            let cwd = v
                .get("workspacePaths")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .map(PathBuf::from);
            let chamada = v.get("toolCall").cloned().unwrap_or(Value::Null);
            let args = chamada.get("args").cloned().unwrap_or(Value::Null);
            let alvo = texto_de(&args, "TargetFile").map(|p| absoluto(&cwd, p));
            let edicao = |m: Mudanca| match &alvo {
                Some(c) => Acao::Edicoes(vec![Edicao {
                    caminho: c.clone(),
                    mudanca: m,
                }]),
                None => Acao::Outra,
            };
            let acao = match texto_de(&chamada, "name").unwrap_or("") {
                "run_command" => {
                    Acao::Shell(texto_de(&args, "CommandLine").unwrap_or("").to_string())
                }
                "write_to_file" => edicao(Mudanca::Conteudo(
                    texto_de(&args, "CodeContent").unwrap_or("").to_string(),
                )),
                "replace_file_content" => edicao(Mudanca::Trechos(
                    ["TargetContent", "ReplacementContent"]
                        .iter()
                        .filter_map(|k| texto_de(&args, k).map(str::to_string))
                        .collect(),
                )),
                "multi_replace_file_content" => edicao(Mudanca::Trechos(strings(
                    args.get("ReplacementChunks").unwrap_or(&Value::Null),
                ))),
                _ => Acao::Outra,
            };
            Evento {
                acao,
                sessao: texto_de(v, "conversationId").unwrap_or("").to_string(),
                cwd,
            }
        }
    }
}

/// Patch do Codex (`*** Begin Patch` ... `*** End Patch`): arquivos e linhas tocadas.
pub fn ler_patch(patch: &str, cwd: &Option<PathBuf>) -> Vec<Edicao> {
    let mut v: Vec<Edicao> = Vec::new();
    for linha in patch.lines() {
        let cab = |p: &str| linha.strip_prefix(p).map(str::trim);
        if let Some(p) = cab("*** Add File:") {
            v.push(Edicao {
                caminho: absoluto(cwd, p),
                mudanca: Mudanca::Conteudo(String::new()),
            });
        } else if let Some(p) = cab("*** Update File:") {
            v.push(Edicao {
                caminho: absoluto(cwd, p),
                mudanca: Mudanca::Trechos(Vec::new()),
            });
        } else if let Some(p) = cab("*** Delete File:") {
            v.push(Edicao {
                caminho: absoluto(cwd, p),
                mudanca: Mudanca::Apagar,
            });
        } else if let Some(p) = cab("*** Move to:") {
            v.push(Edicao {
                caminho: absoluto(cwd, p),
                mudanca: Mudanca::Trechos(Vec::new()),
            });
        } else if linha.starts_with("***") {
            continue;
        } else if let Some(atual) = v.last_mut() {
            let corpo = linha
                .strip_prefix('+')
                .map(|l| (l, true))
                .or_else(|| linha.strip_prefix('-').map(|l| (l, false)));
            match (&mut atual.mudanca, corpo) {
                (Mudanca::Conteudo(c), Some((l, true))) => {
                    c.push_str(l);
                    c.push('\n');
                }
                (Mudanca::Trechos(t), Some((l, _))) => t.push(l.to_string()),
                _ => {}
            }
        }
    }
    v
}

// ---------------------------------------------------------------------------------------
// Raiz e caminhos

/// Raiz do projeto: o diretório mais próximo, subindo, que tem um guarda.yml.
fn achar_raiz(inicio: &Path) -> Option<PathBuf> {
    let mut d = Some(inicio);
    while let Some(x) = d {
        if x.join(config::ARQUIVO).is_file() {
            return Some(x.to_path_buf());
        }
        d = x.parent();
    }
    None
}

fn raiz_da_sessao(e: &Evento) -> Option<PathBuf> {
    let candidatos = [
        e.cwd.clone(),
        std::env::var_os("CLAUDE_PROJECT_DIR").map(PathBuf::from),
        std::env::current_dir().ok(),
    ];
    candidatos
        .into_iter()
        .flatten()
        .find_map(|p| achar_raiz(&p))
}

fn relativo(raiz: &Path, caminho: &Path) -> Option<String> {
    let raiz = std::path::absolute(raiz).ok()?;
    let caminho = std::path::absolute(caminho).ok()?;
    let r = raiz.to_string_lossy().replace('\\', "/").to_lowercase();
    let c = caminho.to_string_lossy().replace('\\', "/");
    // Windows não diferencia maiúsculas no caminho.
    if !c.to_lowercase().starts_with(&r) {
        return None;
    }
    Some(c[r.len()..].trim_start_matches('/').to_string())
}

fn protegido(rel: &str) -> bool {
    let r = rel.to_lowercase();
    r == config::ARQUIVO || r == ".guarda" || r.starts_with(".guarda/")
}

/// Trecho sensível de um arquivo que a IA pode editar, mas não nessa parte.
fn agulha(rel: &str) -> Option<(&'static Regex, &'static str)> {
    static COBERTURA: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)jacoco|kover").expect("regex"));
    static GUARDA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"guarda").expect("regex"));
    let r = rel.to_lowercase();
    let nome = r.rsplit('/').next().unwrap_or(&r);
    let pasta = r.rsplit_once('/').map_or("", |(p, _)| p);
    match (pasta, nome) {
        (_, "pom.xml" | "build.gradle" | "build.gradle.kts") => {
            Some((&COBERTURA, "o plugin de cobertura do build é protegido"))
        }
        (".claude", "settings.json" | "settings.local.json")
        | (".codex" | ".agents", "hooks.json")
        | (".agents", "mcp_config.json")
        | ("", ".mcp.json") => Some((&GUARDA, "a integração da guarda com o agente é protegida")),
        (".git/hooks", "pre-commit") => Some((&GUARDA, "o pre-commit da guarda é protegido")),
        _ => None,
    }
}

fn linhas_com(re: &Regex, t: &str) -> Vec<String> {
    t.lines()
        .filter(|l| re.is_match(l))
        .map(|l| l.trim().to_string())
        .collect()
}

/// A edição mexe em linhas com a agulha?
fn mexe_na_agulha(re: &Regex, m: &Mudanca, caminho: &Path) -> bool {
    let atual = || std::fs::read_to_string(caminho).unwrap_or_default();
    match m {
        Mudanca::Trechos(t) => t.iter().any(|s| re.is_match(s)),
        Mudanca::Conteudo(novo) => {
            let antes = linhas_com(re, &atual());
            !antes.is_empty() && antes != linhas_com(re, novo)
        }
        Mudanca::Apagar => !linhas_com(re, &atual()).is_empty(),
    }
}

static GUARDA_PROIBIDO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bguarda(?:\.exe)?\s+(baseline|init)\b").expect("regex"));
static CITA_PROTEGIDO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)guarda\.ya?ml|\.guarda\b|\.claude[\\/]settings|\.codex[\\/]hooks|\.agents[\\/](hooks|mcp_config)|\.mcp\.json|hooks[\\/]pre-commit",
    )
    .expect("regex")
});
static MUTA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
        \b(rm|del|erase|rmdir|rd|mv|move|cp|copy|ren|rename|sed|perl|tee|truncate|touch|chmod|
           remove-item|move-item|copy-item|rename-item|set-content|add-content|out-file|
           clear-content|new-item|ni|sc|ac)\b
        | \bgit\s+(checkout|restore|reset|rm|mv|stash)\b
        | \bgit\s+commit\b.*--no-verify
        | >",
    )
    .expect("regex")
});
static PULA_VERIFICACAO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bgit\b.*\s(--no-verify|-n)\b").expect("regex"));

/// Motivo para recusar o comando de shell, se houver.
pub fn comando_proibido(cmd: &str) -> Option<&'static str> {
    if let Some(c) = GUARDA_PROIBIDO.captures(cmd) {
        return Some(if c[1].eq_ignore_ascii_case("baseline") {
            "congelar dívida no baseline é decisão humana"
        } else {
            "guarda init reescreve a configuração"
        });
    }
    if PULA_VERIFICACAO.is_match(cmd) && cmd.contains("commit") {
        return Some("commit com --no-verify pula o pre-commit da guarda");
    }
    if CITA_PROTEGIDO.is_match(cmd) && MUTA.is_match(cmd) {
        return Some("guarda.yml, .guarda/ e a integração da guarda são protegidos");
    }
    None
}

fn recusa(motivo: &str) -> Decisao {
    Decisao::Barrar(format!(
        "guarda: {motivo}. Não contorne: siga com a tarefa sem mexer nisso ou peça a um humano."
    ))
}

// ---------------------------------------------------------------------------------------
// Decisões

pub fn proteger(e: &Evento) -> Result<Decisao> {
    match &e.acao {
        Acao::Shell(cmd) => {
            if raiz_da_sessao(e).is_none() {
                return Ok(Decisao::Liberar);
            }
            Ok(comando_proibido(cmd).map_or(Decisao::Liberar, recusa))
        }
        Acao::Edicoes(eds) => {
            for ed in eds {
                let inicio = ed.caminho.parent().unwrap_or(&ed.caminho);
                let Some(raiz) = achar_raiz(inicio).or_else(|| raiz_da_sessao(e)) else {
                    continue;
                };
                let Some(rel) = relativo(&raiz, &ed.caminho) else {
                    continue;
                };
                if protegido(&rel) {
                    return Ok(recusa(&format!("{rel} é protegido")));
                }
                if let Some((re, motivo)) = agulha(&rel)
                    && mexe_na_agulha(re, &ed.mudanca, &ed.caminho)
                {
                    return Ok(recusa(motivo));
                }
            }
            Ok(Decisao::Liberar)
        }
        Acao::Outra => Ok(Decisao::Liberar),
    }
}

fn pos_edicao(e: &Evento) -> Result<Decisao> {
    let Acao::Edicoes(eds) = &e.acao else {
        return Ok(Decisao::Liberar);
    };
    let codigo: Vec<&Edicao> = eds
        .iter()
        .filter(|ed| ed.mudanca != Mudanca::Apagar)
        .filter(|ed| Linguagem::do_caminho(&ed.caminho.to_string_lossy()).is_some())
        .collect();
    let Some(primeira) = codigo.first() else {
        return Ok(Decisao::Liberar);
    };
    let Some(raiz) = achar_raiz(primeira.caminho.parent().unwrap_or(&primeira.caminho)) else {
        return Ok(Decisao::Liberar);
    };
    let rels: Vec<String> = codigo
        .iter()
        .filter_map(|ed| relativo(&raiz, &ed.caminho))
        .collect();
    if rels.is_empty() {
        return Ok(Decisao::Liberar);
    }
    let cfg = Config::carregar(&raiz)?;
    let r = gate::verificar(&raiz, &cfg, Pedido::Arquivos(rels), &Opcoes::default())?;
    if !r.falhou {
        return Ok(Decisao::Liberar);
    }
    let resumo = saida::Resumo {
        congeladas: r.congeladas,
        tempo: r.tempo,
        falhou: true,
        nota: None,
    };
    Ok(Decisao::Barrar(format!(
        "{}Corrija agora, enquanto o contexto está fresco.",
        saida::compacta(&r.novas, 30, &resumo)
    )))
}

const ESTADO_FINAL: &str = ".guarda/cache/final.json";
const RELATORIO_FINAL: &str = ".guarda/cache/final.txt";

#[derive(Default, Serialize, Deserialize)]
struct EstadoFinal {
    sessao: String,
    tentativas: u32,
    /// Violações já liberadas ou avisadas nesta sessão: não seguram a IA de novo.
    liberadas: Vec<String>,
}

fn digital(v: &Violacao) -> String {
    let e = baseline::entrada(v);
    format!("{}:{}:{}", e.codigo, e.arquivo, e.hash)
}

fn final_(e: &Evento) -> Result<Decisao> {
    let Some(raiz) = raiz_da_sessao(e) else {
        return Ok(Decisao::Liberar);
    };
    let cfg = Config::carregar(&raiz)?;
    let op = Opcoes {
        cobertura: cfg.cobertura_no_final(),
        ..Default::default()
    };
    let r = match gate::verificar(&raiz, &cfg, Pedido::Alterados, &op) {
        Ok(r) => r,
        // Sem git (ou sem commit): verifica o projeto todo.
        Err(_) => gate::verificar(&raiz, &cfg, Pedido::Tudo, &op)?,
    };
    let caminho_estado = raiz.join(ESTADO_FINAL);
    let mut estado: EstadoFinal = std::fs::read_to_string(&caminho_estado)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .filter(|x: &EstadoFinal| x.sessao == e.sessao)
        .unwrap_or_else(|| EstadoFinal {
            sessao: e.sessao.clone(),
            ..Default::default()
        });

    let pendentes: Vec<Violacao> = r
        .novas
        .iter()
        .filter(|v| !estado.liberadas.contains(&digital(v)))
        .cloned()
        .collect();
    let bloqueia = match cfg.gate.falhar_em {
        Severidade::Aviso => !pendentes.is_empty(),
        Severidade::Erro => pendentes.iter().any(|v| v.severidade == Severidade::Erro),
    };
    let salvar = |x: &EstadoFinal| -> Result<()> {
        std::fs::create_dir_all(caminho_estado.parent().expect("pai"))?;
        std::fs::write(&caminho_estado, serde_json::to_string(x)?)?;
        Ok(())
    };
    let marcar = |x: &mut EstadoFinal, vs: &[Violacao]| {
        x.liberadas.extend(vs.iter().map(digital));
        x.liberadas.sort();
        x.liberadas.dedup();
    };

    if !bloqueia {
        estado.tentativas = 0;
        // Avisos (ex.: GRD001) não seguram a IA; você fica sabendo uma vez por sessão.
        marcar(&mut estado, &pendentes);
        salvar(&estado)?;
        let avisos: Vec<String> = pendentes
            .iter()
            .map(|v| format!("{} {}:{} {}", v.codigo, v.arquivo, v.linha, v.mensagem))
            .collect();
        return Ok(if avisos.is_empty() {
            Decisao::Liberar
        } else {
            Decisao::Avisar(format!("guarda: {}", avisos.join("; ")))
        });
    }

    estado.tentativas += 1;
    let limite = cfg.gate.tentativas_final;
    let resumo = saida::Resumo {
        congeladas: r.congeladas,
        tempo: r.tempo,
        falhou: true,
        nota: None,
    };
    let relatorio = saida::compacta(&pendentes, 30, &resumo);
    if estado.tentativas > limite {
        std::fs::write(raiz.join(RELATORIO_FINAL), &relatorio)?;
        marcar(&mut estado, &pendentes);
        estado.tentativas = 0;
        salvar(&estado)?;
        return Ok(Decisao::Avisar(format!(
            "guarda: liberei o encerramento depois de {limite} tentativas; {} violação(ões) \
             pendente(s). Relatório em {RELATORIO_FINAL}.",
            pendentes.len()
        )));
    }
    salvar(&estado)?;
    Ok(Decisao::Barrar(format!(
        "{relatorio}A tarefa não termina com violação nova (tentativa {} de {limite}). \
         Corrija e confira com `guarda check --alterados`.",
        estado.tentativas
    )))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn comandos() {
        assert!(comando_proibido("guarda baseline gerar").is_some());
        assert!(comando_proibido("cd x && guarda.exe init --forcar").is_some());
        assert!(comando_proibido("rm -rf .guarda").is_some());
        assert!(comando_proibido("echo 'x' > guarda.yml").is_some());
        assert!(comando_proibido("Remove-Item .guarda\\baseline.json").is_some());
        assert!(comando_proibido("git checkout -- guarda.yml").is_some());
        assert!(comando_proibido("git commit --no-verify -m x").is_some());
        assert!(comando_proibido("rm .codex/hooks.json").is_some());
        assert!(comando_proibido("cat guarda.yml").is_none());
        assert!(comando_proibido("guarda check --alterados").is_none());
        assert!(comando_proibido("git commit -m 'x'").is_none());
        assert!(comando_proibido("cargo test").is_none());
    }

    #[test]
    fn caminhos_protegidos() {
        assert!(protegido("guarda.yml"));
        assert!(protegido(".guarda/baseline.json"));
        assert!(protegido(".GUARDA/x"));
        assert!(!protegido("src/main/guarda.kt"));
    }

    #[test]
    fn agulhas() {
        let (re, _) = agulha("pom.xml").unwrap();
        let tira = Mudanca::Trechos(vec![
            "<artifactId>jacoco-maven-plugin</artifactId>".into(),
            "".into(),
        ]);
        assert!(mexe_na_agulha(re, &tira, Path::new("x")));
        let versao = Mudanca::Trechos(vec![
            "<version>1</version>".into(),
            "<version>2</version>".into(),
        ]);
        assert!(!mexe_na_agulha(re, &versao, Path::new("x")));
        for p in [
            ".claude/settings.json",
            ".codex/hooks.json",
            ".agents/hooks.json",
            ".mcp.json",
            ".git/hooks/pre-commit",
        ] {
            assert!(agulha(p).is_some(), "{p}");
        }
        assert!(agulha("src/settings.json").is_none());
    }

    #[test]
    fn patch_do_codex() {
        let patch = "*** Begin Patch\n*** Update File: src/A.kt\n@@\n-val a = 1\n+val a = b!!\n*** Add File: src/B.kt\n+class B\n*** Delete File: guarda.yml\n*** End Patch";
        let cwd = Some(PathBuf::from("/p"));
        let v = ler_patch(patch, &cwd);
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].caminho, PathBuf::from("/p").join("src/A.kt"));
        assert_eq!(
            v[0].mudanca,
            Mudanca::Trechos(vec!["val a = 1".into(), "val a = b!!".into()])
        );
        assert_eq!(v[1].mudanca, Mudanca::Conteudo("class B\n".into()));
        assert_eq!(v[2].mudanca, Mudanca::Apagar);
    }

    #[test]
    fn eventos_dos_tres_agentes_viram_o_mesmo() {
        let claude = json!({"tool_name": "Edit", "cwd": "/p", "session_id": "s",
            "tool_input": {"file_path": "/p/guarda.yml", "old_string": "a", "new_string": "b"}});
        let anti = json!({"conversationId": "s", "workspacePaths": ["/p"],
            "toolCall": {"name": "replace_file_content",
                "args": {"TargetFile": "/p/guarda.yml", "TargetContent": "a", "ReplacementContent": "b"}}});
        let a = ler_evento(Agente::Claude, &claude);
        let b = ler_evento(Agente::Antigravity, &anti);
        assert_eq!(a.acao, b.acao);
        assert_eq!(a.sessao, b.sessao);
        let shell = json!({"toolCall": {"name": "run_command", "args": {"CommandLine": "ls"}}});
        assert_eq!(
            ler_evento(Agente::Antigravity, &shell).acao,
            Acao::Shell("ls".into())
        );
    }

    #[test]
    fn protocolos() {
        let b = Decisao::Barrar("x".into());
        let s = responder(Agente::Claude, Fase::Proteger, &b);
        assert_eq!((s.codigo, s.stderr.as_deref()), (2, Some("x")));
        let s = responder(Agente::Antigravity, Fase::Proteger, &b);
        assert_eq!(s.codigo, 0);
        assert_eq!(
            s.stdout.as_deref(),
            Some(r#"{"decision":"deny","reason":"x"}"#)
        );
        let s = responder(Agente::Antigravity, Fase::Proteger, &Decisao::Liberar);
        assert_eq!(s.stdout.as_deref(), Some(r#"{"decision":"allow"}"#));
        let s = responder(Agente::Codex, Fase::Final, &b);
        assert_eq!(
            s.stdout.as_deref(),
            Some(r#"{"decision":"block","reason":"x"}"#)
        );
        let s = responder(Agente::Antigravity, Fase::Final, &b);
        assert_eq!(
            s.stdout.as_deref(),
            Some(r#"{"decision":"continue","reason":"x"}"#)
        );
        let s = responder(Agente::Antigravity, Fase::Final, &Decisao::Liberar);
        assert_eq!(s.stdout.as_deref(), Some(r#"{"decision":"stop"}"#));
    }
}
