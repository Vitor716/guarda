//! A verificação completa (motor + baseline + gate), compartilhada pelo `check` e pelos hooks.

use crate::baseline::{self, Baseline};
use crate::cobertura;
use crate::codigo_novo::{self, LinhasNovas};
use crate::config::{self, Config};
use crate::descoberta;
use crate::modelo::{Severidade, Violacao};
use crate::motor::{self, Escopo};
use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub enum Pedido {
    Tudo,
    Arquivos(Vec<String>),
    /// O que mudou contra HEAD (git), incluindo arquivos novos.
    Alterados,
}

#[derive(Default)]
pub struct Opcoes {
    pub sem_baseline: bool,
    pub atualizar_baseline: bool,
    /// Roda os testes antes, para o relatório de cobertura estar em dia.
    pub cobertura: bool,
}

pub struct Resultado {
    pub novas: Vec<Violacao>,
    pub congeladas: usize,
    pub corrigidas: usize,
    pub falhou: bool,
    pub tempo: Duration,
    /// Observação para o rodapé (ex.: cobertura desatualizada).
    pub nota: Option<String>,
}

pub fn caminho_baseline(raiz: &Path, cfg: &Config) -> PathBuf {
    raiz.join(&cfg.gate.baseline)
}

/// Tudo o que a verificação encontrou, antes de comparar com o baseline.
pub struct Coleta {
    pub rel: motor::Relatorio,
    /// Violações que o baseline pode congelar (legado).
    pub congelaveis: Vec<Violacao>,
    /// Violações sobre a mudança atual, nunca congeladas (COV000, COV001, densidade, GRD001).
    pub da_mudanca: Vec<Violacao>,
    pub notas: Vec<String>,
    /// Arquivos (e rótulos como `[cobertura]`) verificados nesta execução.
    pub analisados: HashSet<String>,
}

const DA_MUDANCA: &[&str] = &["COV000", "COV001", "GRD001"];

type Testes = Result<(Option<String>, Option<cobertura::Relatorio>)>;

pub fn coletar(raiz: &Path, cfg: &Config, pedido: Pedido, op: &Opcoes) -> Result<Coleta> {
    std::thread::scope(|s| coletar_em(s, raiz, cfg, pedido, op))
}

fn coletar_em<'s, 'a: 's>(
    s: &'s std::thread::Scope<'s, 'a>,
    raiz: &'a Path,
    cfg: &'a Config,
    pedido: Pedido,
    op: &'a Opcoes,
) -> Result<Coleta> {
    // Em paralelo com o motor: o ancestral comum no git e os testes/relatório de cobertura.
    // Cada processo git custa dezenas de ms no Windows; esperar em série somava centenas.
    let precisa_novo = cfg.cobertura.is_some() || cfg.duplicacao.is_some();
    let base_t = precisa_novo.then(|| s.spawn(move || codigo_novo::base(raiz, cfg.referencia())));
    let cob_t = cfg.cobertura.as_ref().map(|cc| {
        s.spawn(move || -> Testes {
            let falha = if op.cobertura {
                cobertura::rodar_testes(raiz, cc)?
            } else {
                None
            };
            Ok((falha, cobertura::carregar(raiz, cc)?))
        })
    });

    let mut extras = Vec::new();
    let globais = !matches!(pedido, Pedido::Arquivos(_));
    let escopo = match pedido {
        Pedido::Tudo => Escopo::Tudo,
        Pedido::Arquivos(v) => Escopo::Arquivos(v),
        Pedido::Alterados => {
            let alterados = descoberta::alterados(raiz)?;
            extras = grd001(&alterados, cfg);
            Escopo::Arquivos(alterados)
        }
    };
    let mut rel = motor::executar(raiz, cfg, &escopo)?;
    let mut analisados: HashSet<String> = rel.analises.iter().map(|x| x.arquivo.clone()).collect();
    if globais {
        analisados.insert("[cobertura]".into());
    }
    let lista: Vec<String> = rel.analises.iter().map(|x| x.arquivo.clone()).collect();

    // Código novo (cobertura e duplicação), pelo git.
    let mut notas = Vec::new();
    let novas: Option<LinhasNovas> = base_t.and_then(|t| {
        let filtro = (!matches!(escopo, Escopo::Tudo)).then_some(lista.as_slice());
        let r = t
            .join()
            .expect("thread do git")
            .and_then(|b| codigo_novo::calcular_com_base(raiz, &b, filtro));
        match r {
            Ok(n) => Some(n),
            Err(_) => {
                notas.push("sem git: código novo indisponível".to_string());
                None
            }
        }
    });
    let mut pos_baseline = Vec::new();
    if let (Some(cc), Some(t)) = (&cfg.cobertura, cob_t) {
        let (falha, relatorio) = t.join().expect("thread da cobertura")?;
        if let Some(falha) = falha {
            pos_baseline.push(Violacao::nova(
                "COV000",
                "[testes]",
                1,
                falha,
                "faça a suíte passar antes de terminar",
                "",
                "",
                None,
            ));
        }
        let av = cobertura::avaliar(&cobertura::Entrada {
            cfg: cc,
            raiz,
            relatorio: relatorio.as_ref(),
            analisados: &lista,
            novas: novas.as_ref(),
            globais,
            projeto: &rel.projeto,
        });
        rel.violacoes.extend(av.violacoes);
        notas.extend(av.nota);
    }
    if let (Some(d), Some(n), true) = (&cfg.duplicacao, &novas, globais)
        && let Some(v) = densidade(&rel, n, d.codigo_novo_maximo)
    {
        pos_baseline.push(v);
    }
    pos_baseline.extend(extras);

    // Separa o que é da mudança atual; aplica regras desligadas e severidade do guarda.yml.
    let mut congelaveis = Vec::new();
    for v in std::mem::take(&mut rel.violacoes) {
        if DA_MUDANCA.contains(&v.codigo) {
            pos_baseline.push(v);
        } else {
            congelaveis.push(v);
        }
    }
    pos_baseline.retain(|v| !cfg.regras.desligar.iter().any(|d| d == v.codigo));
    for v in pos_baseline.iter_mut().chain(congelaveis.iter_mut()) {
        if let Some(s) = cfg.regras.severidade.get(v.codigo) {
            v.severidade = *s;
        }
    }
    congelaveis.retain(|v| !cfg.regras.desligar.iter().any(|d| d == v.codigo));
    congelaveis.sort_by(|a, b| a.chave_ordem().cmp(&b.chave_ordem()));
    Ok(Coleta {
        rel,
        congelaveis,
        da_mudanca: pos_baseline,
        notas,
        analisados,
    })
}

pub fn verificar(raiz: &Path, cfg: &Config, pedido: Pedido, op: &Opcoes) -> Result<Resultado> {
    let inicio = std::time::Instant::now();
    let Coleta {
        congelaveis,
        da_mudanca,
        notas,
        analisados,
        ..
    } = coletar(raiz, cfg, pedido, op)?;
    let analisados: HashSet<&str> = analisados.iter().map(String::as_str).collect();

    let caminho = caminho_baseline(raiz, cfg);
    let bl = if op.sem_baseline {
        None
    } else {
        Baseline::carregar(&caminho)?
    };
    let (mut novas, congeladas, corrigidas) = match &bl {
        Some(b) => {
            let c = baseline::comparar(b, congelaveis, &analisados);
            if op.atualizar_baseline {
                Baseline {
                    versao: 1,
                    violacoes: c.mantidas,
                }
                .salvar(&caminho)?;
            }
            (c.novas, c.congeladas, c.corrigidas)
        }
        None => (congelaveis, 0, 0),
    };
    novas.extend(da_mudanca);
    novas.sort_by(|a, b| a.chave_ordem().cmp(&b.chave_ordem()));

    let falhou = match cfg.gate.falhar_em {
        Severidade::Aviso => !novas.is_empty(),
        Severidade::Erro => novas.iter().any(|v| v.severidade == Severidade::Erro),
    };
    Ok(Resultado {
        novas,
        congeladas,
        corrigidas,
        falhou,
        tempo: inicio.elapsed(),
        nota: (!notas.is_empty()).then(|| notas.join("; ")),
    })
}

/// SML002 de densidade: linhas duplicadas no código novo acima do máximo (Sonar way: 3%).
fn densidade(rel: &motor::Relatorio, novas: &LinhasNovas, maximo: f64) -> Option<Violacao> {
    let mut total = 0usize;
    let mut duplicadas = 0usize;
    for (arquivo, codigo) in &rel.linhas_codigo {
        let Some(n) = novas.de(arquivo) else { continue };
        let dup = rel.duplicadas.get(arquivo);
        for l in n.intersection(codigo) {
            total += 1;
            if dup.is_some_and(|d| d.contains(l)) {
                duplicadas += 1;
            }
        }
    }
    if total == 0 {
        return None;
    }
    let p = duplicadas as f64 * 100.0 / total as f64;
    if p <= maximo {
        return None;
    }
    let mut v = Violacao::nova(
        "SML002",
        "[duplicação]",
        1,
        format!(
            "código novo com {} de linhas duplicadas>{}",
            cobertura::pct(p),
            cobertura::pct(maximo)
        ),
        "extraia os blocos repetidos apontados nos SML002 dos arquivos",
        "codigo-novo",
        &format!("{duplicadas}/{total}"),
        None,
    );
    v.severidade = Severidade::Erro;
    Some(v)
}

/// GRD001: a configuração ou o baseline mudaram nesta alteração; pede revisão humana no PR.
fn grd001(alterados: &[String], cfg: &Config) -> Vec<Violacao> {
    let baseline = cfg
        .gate
        .baseline
        .trim_start_matches("./")
        .replace('\\', "/");
    alterados
        .iter()
        .filter(|a| a.as_str() == config::ARQUIVO || **a == baseline)
        .map(|a| {
            Violacao::nova(
                "GRD001",
                a,
                1,
                format!("{a} mudou"),
                "revisão humana no PR: a IA não deve afrouxar a regra nem congelar dívida",
                "",
                "",
                None,
            )
        })
        .collect()
}
