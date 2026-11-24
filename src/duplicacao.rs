//! Duplicação por tokens (SML002), como o PMD CPD: janelas de N tokens com hash rolante
//! (Karp-Rabin), confirmadas token a token, e as janelas vizinhas juntadas em blocos.
//!
//! Tokens são as folhas da árvore sintática, sem comentários nem package/imports, com o texto
//! exato (como o CPD por padrão: nome diferente não é duplicação).

use crate::lang::{eh_comentario, filhos, visitar};
use std::collections::{BTreeSet, HashMap};
use tree_sitter::Node;

/// (hash do token, linha 1-based).
pub type Token = (u32, u32);

fn fnv(tipo: &str, texto: &[u8]) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for b in tipo.as_bytes().iter().chain([0u8].iter()).chain(texto) {
        h ^= *b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h
}

pub fn tokens(raiz: Node, fonte: &str) -> Vec<Token> {
    // package e imports ficam no topo: tudo antes do fim do último deles fica de fora.
    let limite = filhos(raiz)
        .iter()
        .filter(|n| {
            matches!(
                n.kind(),
                "package_declaration" | "import_declaration" | "package_header" | "import"
            )
        })
        .map(|n| n.end_byte())
        .max()
        .unwrap_or(0);
    let bytes = fonte.as_bytes();
    let mut v = Vec::new();
    visitar(raiz, |n| {
        if n.child_count() > 0 || n.start_byte() < limite || eh_comentario(n.kind()) {
            return;
        }
        if n.end_byte() == n.start_byte() {
            return;
        }
        let texto = bytes.get(n.byte_range()).unwrap_or(&[]);
        v.push((fnv(n.kind(), texto), n.start_position().row as u32 + 1));
    });
    v
}

/// Um bloco duplicado num arquivo.
#[derive(Debug, Clone, PartialEq)]
pub struct Bloco {
    pub linha_ini: usize,
    pub linha_fim: usize,
    /// Onde está a outra cópia (índice do arquivo, linha).
    pub par: (usize, usize),
    /// Hash dos tokens do bloco (impressão digital estável).
    pub digital: u64,
}

const BASE: u64 = 1_000_003;

/// Blocos duplicados de cada arquivo (mesma ordem de `arquivos`).
pub fn detectar(arquivos: &[&[Token]], minimo: usize) -> Vec<Vec<Bloco>> {
    let minimo = minimo.max(1);
    let mut pot: u64 = 1;
    for _ in 1..minimo {
        pot = pot.wrapping_mul(BASE);
    }
    // hash da janela → posições (arquivo, token inicial)
    let mut janelas: HashMap<u64, Vec<(usize, usize)>> = HashMap::new();
    for (f, toks) in arquivos.iter().enumerate() {
        if toks.len() < minimo {
            continue;
        }
        let mut h: u64 = 0;
        for t in &toks[..minimo] {
            h = h.wrapping_mul(BASE).wrapping_add(t.0 as u64);
        }
        janelas.entry(h).or_default().push((f, 0));
        for i in 1..=toks.len() - minimo {
            h = h
                .wrapping_sub((toks[i - 1].0 as u64).wrapping_mul(pot))
                .wrapping_mul(BASE)
                .wrapping_add(toks[i + minimo - 1].0 as u64);
            janelas.entry(h).or_default().push((f, i));
        }
    }

    let igual = |a: (usize, usize), b: (usize, usize)| {
        arquivos[a.0][a.1..a.1 + minimo]
            .iter()
            .zip(&arquivos[b.0][b.1..b.1 + minimo])
            .all(|(x, y)| x.0 == y.0)
    };
    // Para cada token marcado, a posição da cópia (da janela que o marcou primeiro).
    let mut par: Vec<HashMap<usize, (usize, usize)>> = vec![HashMap::new(); arquivos.len()];
    let mut grupos: Vec<&Vec<(usize, usize)>> = janelas.values().filter(|v| v.len() > 1).collect();
    grupos.sort();
    for posicoes in grupos {
        for (i, &a) in posicoes.iter().enumerate() {
            // A cópia é a primeira posição igual que não se sobrepõe a esta.
            let copia = posicoes.iter().enumerate().find(|&(j, &b)| {
                j != i && !(b.0 == a.0 && b.1.abs_diff(a.1) < minimo) && igual(a, b)
            });
            if let Some((_, &b)) = copia {
                for t in a.1..a.1 + minimo {
                    par[a.0].entry(t).or_insert((b.0, b.1 + (t - a.1)));
                }
            }
        }
    }

    par.iter()
        .enumerate()
        .map(|(f, marcados)| {
            let mut idx: Vec<usize> = marcados.keys().copied().collect();
            idx.sort_unstable();
            let mut blocos = Vec::new();
            let mut i = 0;
            while i < idx.len() {
                let ini = idx[i];
                let mut fim = ini;
                while i + 1 < idx.len() && idx[i + 1] == fim + 1 {
                    i += 1;
                    fim = idx[i];
                }
                let toks = &arquivos[f][ini..=fim];
                let mut d: u64 = 0;
                for t in toks {
                    d = d.wrapping_mul(BASE).wrapping_add(t.0 as u64);
                }
                let (pf, pt) = marcados[&ini];
                blocos.push(Bloco {
                    linha_ini: toks[0].1 as usize,
                    linha_fim: toks[toks.len() - 1].1 as usize,
                    par: (pf, arquivos[pf][pt].1 as usize),
                    digital: d,
                });
                i += 1;
            }
            blocos
        })
        .collect()
}

/// Linhas cobertas por blocos duplicados.
pub fn linhas_duplicadas(blocos: &[Bloco]) -> BTreeSet<usize> {
    blocos
        .iter()
        .flat_map(|b| b.linha_ini..=b.linha_fim)
        .collect()
}

/// Linhas que têm pelo menos um token (linhas de código, para a densidade).
pub fn linhas_de_codigo(tokens: &[Token]) -> BTreeSet<usize> {
    tokens.iter().map(|t| t.1 as usize).collect()
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::lang::{Linguagem, Parsers};

    fn toks(fonte: &str) -> Vec<Token> {
        let mut p = Parsers::novo();
        let t = p.ler(Linguagem::Java, fonte).unwrap();
        tokens(t.root_node(), fonte)
    }

    const METODO: &str = "int soma(int[] v) { int t = 0; for (int i = 0; i < v.length; i++) { if (v[i] > 0) { t += v[i]; } } return t; }";

    #[test]
    fn acha_copia_entre_arquivos() {
        let a = toks(&format!(
            "package a;\nimport x.Y;\nclass A {{\n{METODO}\n}}"
        ));
        let b = toks(&format!(
            "package b;\nclass B {{\nvoid outro() {{}}\n{METODO}\n}}"
        ));
        let c = toks("class C { void nada() { int x = 1; } }");
        let blocos = detectar(&[&a, &b, &c], 20);
        assert_eq!(blocos[0].len(), 1, "{blocos:?}");
        assert_eq!(blocos[1].len(), 1);
        assert!(blocos[2].is_empty());
        assert_eq!(blocos[0][0].linha_ini, 4);
        assert_eq!(blocos[0][0].par, (1, 4));
        assert_eq!(blocos[0][0].digital, blocos[1][0].digital);
    }

    #[test]
    fn imports_e_comentarios_nao_contam() {
        let t = toks("package a;\nimport x.Y;\n// oi\nclass A {}");
        assert_eq!(t.len(), 4); // class A { }
    }
}
