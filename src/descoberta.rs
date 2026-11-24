//! Varredura de arquivos (crate `ignore`, a mesma do ripgrep: respeita .gitignore).

use crate::config::Fontes;
use crate::lang::{Import, Linguagem, caminho_do_import, resolver_import};
use anyhow::{Context, Result};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use regex::Regex;
use std::path::Path;
use std::process::Command;
use std::sync::LazyLock;

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

static RE_PACOTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*package\s+([\w.]+)").expect("regex"));
static RE_IMPORT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*import\s+(?:static\s+)?([\w.]+(?:\.\*)?)").expect("regex"));

/// Índice rápido, sem parser: pacote e imports de cada arquivo, lidos por regex.
/// Usado no modo `--arquivos`, quando o resto do projeto não é analisado.
pub fn indice_rapido(
    raiz: &Path,
    arquivos: &[String],
) -> Vec<(String, Option<String>, Vec<Import>)> {
    use rayon::prelude::*;
    arquivos
        .par_iter()
        .map(|rel| {
            let fonte = std::fs::read_to_string(raiz.join(rel)).unwrap_or_default();
            let mut pacote = None;
            let mut imports = Vec::new();
            for (i, l) in fonte.lines().enumerate() {
                if pacote.is_none()
                    && let Some(c) = RE_PACOTE.captures(l) {
                        pacote = Some(c[1].to_string());
                        continue;
                    }
                if let Some(c) = RE_IMPORT.captures(l) {
                    let (caminho, curinga) = caminho_do_import(&format!("import {}", &c[1]));
                    let (pkg, nome) = resolver_import(&caminho, curinga);
                    imports.push(Import {
                        pacote: pkg,
                        nome,
                        linha: i + 1,
                        texto: l.trim().to_string(),
                    });
                }
            }
            (rel.clone(), pacote, imports)
        })
        .collect()
}
