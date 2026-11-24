//! Varredura de arquivos (crate `ignore`, a mesma do ripgrep: respeita .gitignore).

use crate::config::Fontes;
use crate::lang::Linguagem;
use anyhow::{Context, Result};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use std::path::Path;
use std::process::Command;

fn globs(padroes: &[String]) -> Result<GlobSet> {
    let mut b = GlobSetBuilder::new();
    for p in padroes {
        b.add(
            GlobBuilder::new(p)
                .literal_separator(true)
                .build()
                .with_context(|| format!("glob inválido em fontes: {p:?}"))?,
        );
    }
    Ok(b.build()?)
}

/// Caminho relativo à raiz, sempre com `/`.
pub fn relativo(raiz: &Path, caminho: &Path) -> Option<String> {
    let r = caminho.strip_prefix(raiz).ok()?;
    Some(r.to_string_lossy().replace('\\', "/"))
}

/// Arquivos de produção Kotlin e Java, ordenados. Sem nenhum em `fontes.producao`
/// (layout fora do padrão), analisa tudo que não for teste.
pub fn listar(raiz: &Path, fontes: &Fontes) -> Result<Vec<String>> {
    let producao = globs(&fontes.producao)?;
    let testes = globs(&fontes.testes)?;
    let ignorar = globs(&fontes.ignorar)?;
    let mut todos = Vec::new();
    for e in WalkBuilder::new(raiz)
        .hidden(true)
        .require_git(false)
        .build()
    {
        let e = e?;
        if !e.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let Some(rel) = relativo(raiz, e.path()) else {
            continue;
        };
        if Linguagem::do_caminho(&rel).is_none() || ignorar.is_match(&rel) {
            continue;
        }
        todos.push(rel);
    }
    let mut sel: Vec<String> = todos
        .iter()
        .filter(|r| producao.is_match(r))
        .cloned()
        .collect();
    if sel.is_empty() {
        sel = todos.into_iter().filter(|r| !testes.is_match(r)).collect();
    }
    sel.sort();
    Ok(sel)
}

/// Arquivos alterados contra HEAD, incluindo os novos não rastreados.
pub fn alterados(raiz: &Path) -> Result<Vec<String>> {
    let git = |args: &[&str]| -> Result<Vec<String>> {
        let out = Command::new("git")
            .args(args)
            .current_dir(raiz)
            .output()
            .context("não consegui rodar o git")?;
        if !out.status.success() {
            anyhow::bail!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect())
    };
    let mut v = git(&[
        "-c",
        "core.quotepath=off",
        "diff",
        "--name-only",
        "--relative",
        "HEAD",
    ])?;
    v.extend(git(&[
        "-c",
        "core.quotepath=off",
        "ls-files",
        "--others",
        "--exclude-standard",
    ])?);
    v.sort();
    v.dedup();
    Ok(v)
}
