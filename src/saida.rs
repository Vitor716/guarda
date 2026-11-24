//! Saída feita para a IA: uma linha por violação, caminho uma vez por arquivo,
//! repetições colapsadas, teto de linhas e uma linha só quando está tudo certo.

use crate::modelo::{Severidade, Violacao};
use std::fmt::Write;
use std::time::Duration;

pub struct Resumo {
    pub congeladas: usize,
    pub tempo: Duration,
    pub falhou: bool,
    /// Observação curta para o rodapé (ex.: cobertura desatualizada).
    pub nota: Option<String>,
}

pub fn segundos(d: Duration) -> String {
    format!("{:.2}s", d.as_secs_f64()).replace('.', ",")
}

/// Repetições do mesmo código, mensagem e dica no mesmo arquivo viram uma linha só.
const COLAPSAR_A_PARTIR_DE: usize = 3;

pub fn compacta(vs: &[Violacao], max: usize, r: &Resumo) -> String {
    let mut s = String::new();
    let mut usadas = 0usize;
    let mut ocultas = 0usize;
    let mut i = 0;
    while i < vs.len() {
        let arquivo = &vs[i].arquivo;
        let fim = vs[i..]
            .iter()
            .position(|v| &v.arquivo != arquivo)
            .map_or(vs.len(), |p| i + p);
        let doarq = &vs[i..fim];
        let grupos = agrupar(doarq);
        if usadas + 2 > max {
            ocultas += doarq.len();
        } else {
            let _ = writeln!(s, "{arquivo}");
            usadas += 1;
            // Em ordem de posição; um grupo colapsado aparece no lugar da primeira ocorrência.
            for x in doarq {
                let g = grupos
                    .iter()
                    .find(|g| g.iter().any(|y| std::ptr::eq(*y, x)))
                    .expect("toda violação está num grupo");
                let colapsa = g.len() >= COLAPSAR_A_PARTIR_DE;
                if colapsa && !std::ptr::eq(g[0], x) {
                    continue;
                }
                let peso = if colapsa { g.len() } else { 1 };
                if usadas >= max {
                    ocultas += peso;
                    continue;
                }
                if colapsa {
                    let linhas: Vec<String> = g.iter().map(|y| y.linha.to_string()).collect();
                    let _ = writeln!(
                        s,
                        "{:>3} {} x{} l.{} {} | {}",
                        x.linha,
                        x.codigo,
                        g.len(),
                        linhas.join(","),
                        x.mensagem,
                        x.dica
                    );
                } else {
                    let _ = writeln!(s, "{:>3} {} {} | {}", x.linha, x.codigo, x.mensagem, x.dica);
                }
                usadas += 1;
            }
        }
        i = fim;
    }
    if ocultas > 0 {
        let _ = writeln!(s, "+{ocultas} ocultas (use --max)");
    }
    s.push_str(&rodape(vs, r));
    s.push('\n');
    s
}

/// Agrupa as violações do arquivo, mantendo a ordem da primeira ocorrência de cada grupo.
fn agrupar(vs: &[Violacao]) -> Vec<Vec<&Violacao>> {
    let mut grupos: Vec<Vec<&Violacao>> = Vec::new();
    for v in vs {
        match grupos.iter_mut().find(|g| {
            let p = g[0];
            p.codigo == v.codigo && p.mensagem == v.mensagem && p.dica == v.dica
        }) {
            Some(g) => g.push(v),
            None => grupos.push(vec![v]),
        }
    }
    grupos
}

pub fn rodape(vs: &[Violacao], r: &Resumo) -> String {
    let erros = vs
        .iter()
        .filter(|v| v.severidade == Severidade::Erro)
        .count();
    let avisos = vs.len() - erros;
    let mut arquivos: Vec<&str> = vs.iter().map(|v| v.arquivo.as_str()).collect();
    arquivos.dedup();
    let mut partes = Vec::new();
    if r.falhou {
        partes.push(format!(
            "✗ {erros} {} em {} {}",
            if erros == 1 { "erro" } else { "erros" },
            arquivos.len(),
            if arquivos.len() == 1 {
                "arquivo"
            } else {
                "arquivos"
            }
        ));
    } else {
        partes.push(format!("✓ guarda {erros} erros"));
    }
    if avisos > 0 {
        partes.push(format!(
            "{avisos} {}",
            if avisos == 1 { "aviso" } else { "avisos" }
        ));
    }
    if let Some(n) = &r.nota {
        partes.push(n.clone());
    }
    if r.congeladas > 0 {
        partes.push(format!("baseline {}", r.congeladas));
    }
    partes.push(segundos(r.tempo));
    partes.join(" · ")
}

/// Um objeto JSON por linha.
pub fn json(vs: &[Violacao]) -> String {
    let mut s = String::new();
    for v in vs {
        s.push_str(&serde_json::to_string(v).unwrap_or_default());
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod testes {
    use super::*;

    fn v(arquivo: &str, linha: usize, codigo: &'static str) -> Violacao {
        Violacao::nova(codigo, arquivo, linha, "uso de !!", "troque", "A", "", None)
    }

    #[test]
    fn colapsa_e_resume() {
        let vs = vec![
            v("A.kt", 1, "KT001"),
            v("A.kt", 5, "KT001"),
            v("A.kt", 9, "KT001"),
            v("B.kt", 2, "KT001"),
        ];
        let r = Resumo {
            congeladas: 3,
            tempo: Duration::from_millis(180),
            falhou: true,
            nota: None,
        };
        let s = compacta(&vs, 30, &r);
        assert_eq!(
            s,
            "A.kt\n  1 KT001 x3 l.1,5,9 uso de !! | troque\nB.kt\n  2 KT001 uso de !! | troque\n\
             ✗ 4 erros em 2 arquivos · baseline 3 · 0,18s\n"
        );
    }

    #[test]
    fn sucesso_e_uma_linha() {
        let r = Resumo {
            congeladas: 0,
            tempo: Duration::from_millis(120),
            falhou: false,
            nota: None,
        };
        assert_eq!(compacta(&[], 30, &r), "✓ guarda 0 erros · 0,12s\n");
    }

    #[test]
    fn teto_de_linhas() {
        let vs: Vec<Violacao> = (1..=10)
            .map(|i| v(&format!("F{i:02}.kt"), i, "KT001"))
            .collect();
        let r = Resumo {
            congeladas: 0,
            tempo: Duration::ZERO,
            falhou: true,
            nota: None,
        };
        let s = compacta(&vs, 4, &r);
        assert!(s.contains("+8 ocultas"), "{s}");
    }
}
