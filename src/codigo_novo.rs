//! Código novo, como o Sonar define: as linhas acrescentadas ou alteradas desde o ancestral
//! comum com a referência (ex.: origin/main), incluindo o que ainda não foi commitado e os
//! arquivos novos não rastreados.

use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Default)]
pub struct LinhasNovas {
    /// arquivo (relativo à raiz, com `/`) → linhas novas (1-based).
    pub por_arquivo: BTreeMap<String, BTreeSet<usize>>,
    /// A referência que de fato foi usada (pode ser uma alternativa à configurada).
    pub referencia: String,
}

impl LinhasNovas {
    pub fn de(&self, arquivo: &str) -> Option<&BTreeSet<usize>> {
        self.por_arquivo.get(arquivo)
    }
}

fn git(raiz: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .args(["-c", "core.quotepath=off"])
        .args(args)
        .current_dir(raiz)
        .output()
        .context("não consegui rodar o git")?;
    if !out.status.success() {
        bail!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Ancestral comum com a primeira referência que existir (e a referência usada); por
/// último, HEAD.
pub fn base(raiz: &Path, referencia: &str) -> Result<(String, String)> {
    let alternativas = [referencia, "origin/main", "main", "origin/master", "master"];
    for r in alternativas {
        if let Ok(b) = git(raiz, &["merge-base", r, "HEAD"]) {
            return Ok((b.trim().to_string(), r.to_string()));
        }
    }
    let head = git(raiz, &["rev-parse", "HEAD"])?;
    Ok((head.trim().to_string(), "HEAD".to_string()))
}

/// Acima disso a lista de arquivos não vai na linha de comando (limite do Windows).
const MAX_ARQUIVOS_NO_COMANDO: usize = 200;

/// Linhas novas do projeto, ou só de `arquivos` (bem mais rápido num branch grande).
pub fn calcular(raiz: &Path, referencia: &str, arquivos: Option<&[String]>) -> Result<LinhasNovas> {
    if arquivos.is_some_and(|a| a.is_empty()) {
        return Ok(LinhasNovas {
            por_arquivo: BTreeMap::new(),
            referencia: referencia.to_string(),
        });
    }
    calcular_com_base(raiz, &base(raiz, referencia)?, arquivos)
}

/// Como `calcular`, com o ancestral comum já conhecido (para buscá-lo em paralelo).
pub fn calcular_com_base(
    raiz: &Path,
    (base, usada): &(String, String),
    arquivos: Option<&[String]>,
) -> Result<LinhasNovas> {
    if arquivos.is_some_and(|a| a.is_empty()) {
        return Ok(LinhasNovas {
            por_arquivo: BTreeMap::new(),
            referencia: usada.clone(),
        });
    }
    let filtro: Vec<&str> = match arquivos {
        Some(a) if a.len() <= MAX_ARQUIVOS_NO_COMANDO => std::iter::once("--")
            .chain(a.iter().map(String::as_str))
            .collect(),
        _ => Vec::new(),
    };
    let mut args_diff = vec![
        "diff",
        "-U0",
        "--no-color",
        "--no-ext-diff",
        "--relative",
        base.as_str(),
    ];
    args_diff.extend(&filtro);
    let mut args_outros = vec!["ls-files", "--others", "--exclude-standard"];
    args_outros.extend(&filtro);
    let (diff, outros) = std::thread::scope(|s| {
        let outros = s.spawn(|| git(raiz, &args_outros));
        let diff = git(raiz, &args_diff);
        (diff, outros.join().expect("thread do git"))
    });
    let mut por_arquivo = ler_diff(&diff?);
    for novo in outros?.lines() {
        let novo = novo.trim();
        if novo.is_empty() {
            continue;
        }
        let n = std::fs::read_to_string(raiz.join(novo))
            .map(|t| t.lines().count())
            .unwrap_or(0);
        por_arquivo.insert(novo.to_string(), (1..=n).collect());
    }
    Ok(LinhasNovas {
        por_arquivo,
        referencia: usada.clone(),
    })
}

/// `git diff -U0`: linhas acrescentadas de cada arquivo, pelos cabeçalhos `@@ -a,b +c,d @@`.
pub fn ler_diff(diff: &str) -> BTreeMap<String, BTreeSet<usize>> {
    let mut m: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    let mut atual: Option<String> = None;
    for linha in diff.lines() {
        if let Some(p) = linha.strip_prefix("+++ ") {
            atual = p.strip_prefix("b/").map(str::to_string);
            if let Some(a) = &atual {
                m.entry(a.clone()).or_default();
            }
        } else if let (Some(a), Some(resto)) = (&atual, linha.strip_prefix("@@ ")) {
            let Some(novo) = resto.split_whitespace().find(|t| t.starts_with('+')) else {
                continue;
            };
            let novo = &novo[1..];
            let (ini, qtd) = match novo.split_once(',') {
                Some((i, q)) => (i.parse().unwrap_or(0), q.parse().unwrap_or(0)),
                None => (novo.parse().unwrap_or(0), 1),
            };
            let linhas = m.entry(a.clone()).or_default();
            linhas.extend(ini..ini + qtd);
        }
    }
    m.retain(|_, v| !v.is_empty());
    m
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn diff_unificado_sem_contexto() {
        let d = "\
diff --git a/src/A.kt b/src/A.kt
--- a/src/A.kt
+++ b/src/A.kt
@@ -3 +3 @@ class A
-a
+b
@@ -10,0 +11,3 @@ fun x()
+c
+d
+e
@@ -20,2 +23,0 @@
-x
-y
diff --git a/src/B.kt b/src/B.kt
deleted file mode 100644
--- a/src/B.kt
+++ /dev/null
";
        let m = ler_diff(d);
        assert_eq!(m.len(), 1);
        assert_eq!(
            m["src/A.kt"].iter().copied().collect::<Vec<_>>(),
            vec![3, 11, 12, 13]
        );
    }
}
