//! SUP001: supressão sem motivo, e as diretivas `// guarda:ignorar CODIGO motivo: ...`.

use crate::lang::{Linguagem, filhos, linha, texto, visitar};
use crate::modelo::Violacao;
use std::collections::HashSet;
use tree_sitter::Node;

const DIRETIVA: &str = "guarda:ignorar";

/// Diretiva válida: ignora os códigos na linha do comentário e na seguinte.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Supressao {
    pub linha: usize,
    pub codigos: Vec<String>,
}

impl Supressao {
    pub fn cobre(&self, codigo: &str, linha: usize) -> bool {
        (linha == self.linha || linha == self.linha + 1) && self.codigos.iter().any(|c| c == codigo)
    }
}

pub fn verificar(
    raiz: Node,
    fonte: &str,
    ling: Linguagem,
    arquivo: &str,
) -> (Vec<Violacao>, Vec<Supressao>) {
    let mut violacoes = Vec::new();
    let mut supressoes = Vec::new();
    // Linhas (0-based) que têm um comentário de linha.
    let mut com_comentario: HashSet<usize> = HashSet::new();
    let mut anotacoes = Vec::new();

    visitar(raiz, |n| match n.kind() {
        "line_comment" | "block_comment" => {
            let t = texto(n, fonte);
            if n.kind() == "line_comment" {
                com_comentario.insert(n.start_position().row);
            }
            if let Some(i) = t.find(DIRETIVA) {
                match ler_diretiva(&t[i + DIRETIVA.len()..]) {
                    Some(codigos) => supressoes.push(Supressao {
                        linha: linha(n),
                        codigos,
                    }),
                    None => violacoes.push(Violacao::nova(
                        "SUP001",
                        arquivo,
                        linha(n),
                        "guarda:ignorar sem código ou sem motivo",
                        "escreva // guarda:ignorar CODIGO motivo: <por quê>",
                        crate::lang::simbolo_dono(n, fonte),
                        t,
                        None,
                    )),
                }
            }
        }
        "marker_annotation" | "annotation" | "file_annotation" => anotacoes.push(n),
        _ => {}
    });

    let linhas: Vec<&str> = fonte.lines().collect();
    for a in anotacoes {
        let Some(nome) = nome_da_supressao(a, fonte, ling) else {
            continue;
        };
        let row = a.start_position().row;
        let linha_de_cima_so_comentario = row > 0
            && com_comentario.contains(&(row - 1))
            && linhas
                .get(row - 1)
                .is_some_and(|l| l.trim_start().starts_with("//"));
        if com_comentario.contains(&row) || linha_de_cima_so_comentario {
            continue;
        }
        violacoes.push(Violacao::nova(
            "SUP001",
            arquivo,
            linha(a),
            format!("@{nome} sem motivo"),
            "comente o motivo na mesma linha ou na linha de cima",
            crate::lang::simbolo_dono(a, fonte),
            texto(a, fonte),
            None,
        ));
    }
    (violacoes, supressoes)
}

/// `KT001,CPX001 motivo: texto` → códigos, se houver código e motivo não vazio.
fn ler_diretiva(resto: &str) -> Option<Vec<String>> {
    let resto = resto.trim_end_matches("*/");
    let (antes, motivo) = resto.split_once("motivo:")?;
    if motivo.trim().is_empty() {
        return None;
    }
    let codigos: Vec<String> = antes
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    let valido = |c: &String| {
        c.len() >= 5
            && c.chars().take_while(|ch| ch.is_ascii_uppercase()).count() >= 2
            && c.chars().rev().take(3).all(|ch| ch.is_ascii_digit())
    };
    if codigos.is_empty() || !codigos.iter().all(valido) {
        return None;
    }
    Some(codigos)
}

/// Nome da anotação, se ela for de supressão (`SuppressWarnings`, `Suppress`, `SuppressLint`).
fn nome_da_supressao<'a>(a: Node, fonte: &'a str, ling: Linguagem) -> Option<&'a str> {
    let nome = match ling {
        Linguagem::Java => {
            let n = a.child_by_field_name("name")?;
            let t = texto(n, fonte);
            t.rsplit('.').next().unwrap_or(t)
        }
        Linguagem::Kotlin => {
            // @Suppress("X"), @file:Suppress("X"): o nome é o primeiro tipo dentro da anotação.
            let alvo = filhos(a)
                .into_iter()
                .find(|c| matches!(c.kind(), "constructor_invocation" | "type" | "user_type"))?;
            let t = texto(alvo, fonte);
            let t = t.split('(').next().unwrap_or(t).trim();
            t.rsplit('.').next().unwrap_or(t)
        }
    };
    matches!(nome, "SuppressWarnings" | "Suppress" | "SuppressLint").then_some(nome)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn diretiva() {
        assert_eq!(
            ler_diretiva(" KT001 motivo: legado").unwrap(),
            vec!["KT001"]
        );
        assert_eq!(ler_diretiva(" KT001,CPX001 motivo: x").unwrap().len(), 2);
        assert!(ler_diretiva(" KT001").is_none());
        assert!(ler_diretiva(" KT001 motivo:   ").is_none());
        assert!(ler_diretiva(" motivo: sem codigo").is_none());
    }
}
