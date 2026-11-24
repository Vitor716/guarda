//! A verificação completa (motor + baseline + gate), compartilhada pelo `check` e pelos hooks.

use crate::baseline::{self, Baseline};
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
}

pub struct Resultado {
    pub novas: Vec<Violacao>,
    pub congeladas: usize,
    pub corrigidas: usize,
    pub falhou: bool,
    pub tempo: Duration,
}

pub fn caminho_baseline(raiz: &Path, cfg: &Config) -> PathBuf {
    raiz.join(&cfg.gate.baseline)
}

pub fn verificar(raiz: &Path, cfg: &Config, pedido: Pedido, op: &Opcoes) -> Result<Resultado> {
    let mut extras = Vec::new();
    let escopo = match pedido {
        Pedido::Tudo => Escopo::Tudo,
        Pedido::Arquivos(v) => Escopo::Arquivos(v),
        Pedido::Alterados => {
            let alterados = descoberta::alterados(raiz)?;
            extras = grd001(&alterados, cfg);
            Escopo::Arquivos(alterados)
        }
    };
    let rel = motor::executar(raiz, cfg, &escopo)?;
    let analisados: HashSet<&str> = rel.analises.iter().map(|x| x.arquivo.as_str()).collect();

    let caminho = caminho_baseline(raiz, cfg);
    let bl = if op.sem_baseline {
        None
    } else {
        Baseline::carregar(&caminho)?
    };
    let (mut novas, congeladas, corrigidas) = match &bl {
        Some(b) => {
            let c = baseline::comparar(b, rel.violacoes.clone(), &analisados);
            if op.atualizar_baseline {
                Baseline {
                    versao: 1,
                    violacoes: c.mantidas,
                }
                .salvar(&caminho)?;
            }
            (c.novas, c.congeladas, c.corrigidas)
        }
        None => (rel.violacoes.clone(), 0, 0),
    };
    novas.extend(
        extras
            .into_iter()
            .filter(|v| !cfg.regras.desligar.iter().any(|d| d == v.codigo)),
    );
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
        tempo: rel.tempo,
    })
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
