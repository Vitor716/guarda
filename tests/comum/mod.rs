//! Auxiliares dos testes de ponta a ponta.
#![allow(dead_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Roda o binário; devolve (exit code, stdout, stderr).
pub fn rodar(raiz: &Path, args: &[&str], stdin: Option<&str>) -> (i32, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_guarda"));
    cmd.args(args)
        .current_dir(raiz)
        .env_remove("CLAUDE_PROJECT_DIR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut filho = cmd.spawn().unwrap();
    {
        let mut entrada = filho.stdin.take().unwrap();
        if let Some(t) = stdin {
            entrada.write_all(t.as_bytes()).unwrap();
        }
    }
    let out = filho.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

/// Saída junta (stdout + stderr), como o terminal mostra.
pub fn guarda(raiz: &Path, args: &[&str]) -> (i32, String) {
    let (c, o, e) = rodar(raiz, args, None);
    (c, o + &e)
}

/// Copia o projeto de exemplo para um diretório temporário (o init escreve arquivos).
pub fn projeto() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copiar(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hex"),
        dir.path(),
    );
    dir
}

pub fn copiar(de: &Path, para: &Path) {
    std::fs::create_dir_all(para).unwrap();
    for e in std::fs::read_dir(de).unwrap() {
        let e = e.unwrap();
        let destino = para.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copiar(&e.path(), &destino);
        } else {
            std::fs::copy(e.path(), destino).unwrap();
        }
    }
}

pub fn git(raiz: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(raiz)
        .output()
        .unwrap()
        .status
        .success();
    assert!(ok, "git {args:?}");
}
