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
/// SARIF 2.1.0, para code scanning (GitHub, Azure DevOps) e IDEs.
pub fn sarif(vs: &[Violacao]) -> String {
    use serde_json::json;
    let mut codigos: Vec<&str> = vs.iter().map(|v| v.codigo).collect();
    codigos.sort_unstable();
    codigos.dedup();
    let regras: Vec<serde_json::Value> = codigos
        .iter()
        .map(|c| {
            let nome = crate::modelo::regra(c).map_or(*c, |r| r.nome);
            let mut r = json!({ "id": c, "name": nome, "shortDescription": { "text": nome } });
            if let Some(e) = crate::explicar::explicar(c) {
                r["help"] = json!({ "text": e });
            }
            r
        })
        .collect();
    let resultados: Vec<serde_json::Value> = vs
        .iter()
        .map(|v| {
            let mut r = json!({
                "ruleId": v.codigo,
                "level": if v.severidade == Severidade::Erro { "error" } else { "warning" },
                "message": { "text": format!("{} | {}", v.mensagem, v.dica) },
            });
            // `[cobertura]` e afins não são arquivos: ficam sem local.
            if !v.arquivo.starts_with('[') {
                r["locations"] = json!([{
                    "physicalLocation": {
                        "artifactLocation": { "uri": v.arquivo },
                        "region": { "startLine": v.linha.max(1) }
                    }
                }]);
            }
            r
        })
        .collect();
    let doc = json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": {
                "name": "guarda",
                "version": env!("CARGO_PKG_VERSION"),
                "rules": regras
            }},
            "results": resultados
        }]
    });
    serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n"
}

/// Modo texto para pessoas: a saída compacta com cor (ANSI) e uma linha em branco entre
/// arquivos. Só no terminal e sem `NO_COLOR`.
pub fn texto(vs: &[Violacao], max: usize, r: &Resumo) -> String {
    const NEGRITO: &str = "\x1b[1m";
    const VERMELHO: &str = "\x1b[31m";
    const AMARELO: &str = "\x1b[33m";
    const VERDE: &str = "\x1b[32m";
    const APAGADO: &str = "\x1b[2m";
    const FIM: &str = "\x1b[0m";
    let base = compacta(vs, max, r);
    let mut s = String::new();
    let mut primeiro = true;
    for l in base.lines() {
        if l.starts_with('✓') {
            let _ = writeln!(s, "{VERDE}{l}{FIM}");
        } else if l.starts_with('✗') {
            let _ = writeln!(s, "{VERMELHO}{l}{FIM}");
        } else if l.starts_with('+') {
            let _ = writeln!(s, "{APAGADO}{l}{FIM}");
        } else if !l.starts_with(' ') && !l.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            if !primeiro {
                s.push('\n');
            }
            primeiro = false;
            let _ = writeln!(s, "{NEGRITO}{l}{FIM}");
        } else {
            // "  42 CPX001 mensagem | dica"
            let (antes, dica) = l.split_once(" | ").unwrap_or((l, ""));
            let mut partes = antes.trim_start().splitn(3, ' ');
            let (num, cod, msg) = (
                partes.next().unwrap_or(""),
                partes.next().unwrap_or(""),
                partes.next().unwrap_or(""),
            );
            let erro = vs
                .iter()
                .find(|v| v.codigo == cod)
                .is_some_and(|v| v.severidade == Severidade::Erro);
            let cor = if erro { VERMELHO } else { AMARELO };
            let _ = writeln!(
                s,
                "{APAGADO}{num:>5}{FIM} {cor}{cod}{FIM} {msg}\n      {APAGADO}→ {dica}{FIM}"
            );
        }
    }
    s
}

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
    fn sarif_valido() {
        let mut vs = vec![v("A.kt", 3, "KT001")];
        vs.push(Violacao::nova(
            "COV003",
            "[cobertura]",
            1,
            "projeto 50%<60%",
            "x",
            "geral",
            "",
            Some(500),
        ));
        let s: serde_json::Value = serde_json::from_str(&sarif(&vs)).unwrap();
        assert_eq!(s["version"], "2.1.0");
        let run = &s["runs"][0];
        assert_eq!(run["tool"]["driver"]["rules"].as_array().unwrap().len(), 2);
        let r0 = &run["results"][0];
        assert_eq!(r0["ruleId"], "KT001");
        assert_eq!(r0["level"], "error");
        assert_eq!(
            r0["locations"][0]["physicalLocation"]["region"]["startLine"],
            3
        );
        assert!(
            run["results"][1].get("locations").is_none(),
            "[cobertura] não é arquivo"
        );
    }

    #[test]
    fn texto_tem_cor_e_mesmo_conteudo() {
        let vs = vec![v("A.kt", 3, "KT001")];
        let r = Resumo {
            congeladas: 0,
            tempo: Duration::ZERO,
            falhou: true,
            nota: None,
        };
        let t = texto(&vs, 30, &r);
        assert!(t.contains("\x1b[1mA.kt"), "{t:?}");
        assert!(t.contains("KT001") && t.contains("→ troque"), "{t:?}");
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
