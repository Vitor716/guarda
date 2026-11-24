//! Correção (prováveis bugs), dos itens de tipo BUG do Sonar way, do potential-bugs do
//! detekt e do errorprone do PMD:
//! BUG001 desvio dentro de finally (S1143, detekt ReturnFromFinally, PMD ReturnFromFinallyBlock),
//! BUG002 mesma expressão dos dois lados (S1764), BUG003 auto-atribuição (S1656),
//! BUG004 condição repetida na cadeia (S1862), BUG005 ramos idênticos (S1871/S3923),
//! BUG006 String comparada com == (S4973), BUG007 BigDecimal de double (S2111,
//! PMD AvoidDecimalLiteralsInBigDecimalConstructor), BUG008 InterruptedException ignorada (S2142).

use super::Contexto;
use super::estrutura::percorrer;
use super::excecoes;
use super::fluxo::{eh_if, instrucoes, partes};
use super::sintaxe::chamada;
use crate::lang::{Linguagem, eh_funcao, eh_lambda, filhos, linha, simbolo_dono, texto, visitar};
use crate::modelo::Violacao;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;
use tree_sitter::Node;

fn normal(t: &str) -> String {
    t.chars().filter(|c| !c.is_whitespace()).collect()
}

fn add(
    ctx: &Contexto,
    v: &mut Vec<Violacao>,
    codigo: &'static str,
    n: Node,
    msg: String,
    dica: &str,
    trecho: &str,
) {
    v.push(Violacao::nova(
        codigo,
        ctx.arquivo,
        linha(n),
        msg,
        dica,
        simbolo_dono(n, ctx.fonte),
        trecho,
        None,
    ));
}

fn eh_laco(k: &str) -> bool {
    matches!(
        k,
        "for_statement"
            | "enhanced_for_statement"
            | "while_statement"
            | "do_statement"
            | "do_while_statement"
    )
}

/// BUG001: return/throw (e break/continue que saem do finally) dentro de finally.
fn finally(ctx: &Contexto, bloco: Node, v: &mut Vec<Violacao>) {
    fn rec(ctx: &Contexto, n: Node, em_laco: bool, v: &mut Vec<Violacao>) {
        for c in filhos(n) {
            let k = c.kind();
            if eh_lambda(k) || eh_funcao(k) {
                continue;
            }
            let palavra = match k {
                "return_statement" | "return_expression" => Some("return"),
                "throw_statement" | "throw_expression" => Some("throw"),
                "break_statement" | "continue_statement" if !em_laco => {
                    Some(if k.starts_with("break") {
                        "break"
                    } else {
                        "continue"
                    })
                }
                "identifier" if !em_laco && matches!(texto(c, ctx.fonte), "break" | "continue") => {
                    Some(texto(c, ctx.fonte))
                }
                _ => None,
            };
            if let Some(p) = palavra {
                add(
                    ctx,
                    v,
                    "BUG001",
                    c,
                    format!("{p} dentro de finally"),
                    "o finally engole a exceção do try; tire o desvio do finally",
                    p,
                );
            }
            rec(ctx, c, em_laco || eh_laco(k), v);
        }
    }
    rec(ctx, bloco, false, v);
}

const OPERADORES_SIMETRICOS: &[&str] = &[
    "==", "!=", "<", ">", "<=", ">=", "&&", "||", "-", "/", "%", "^", "&", "|", "===", "!==",
];

/// Cadeia if / else if a partir do primeiro if: (condição, corpo) de cada ramo e o else final.
fn cadeia<'t>(primeiro: Node<'t>) -> (Vec<(Node<'t>, Node<'t>)>, Option<Node<'t>>) {
    let mut ramos = Vec::new();
    let mut fim = None;
    let mut atual = Some(primeiro);
    while let Some(n) = atual {
        let (cond, entao, senao) = partes(n);
        if let (Some(c), Some(e)) = (cond, entao) {
            ramos.push((c, e));
        }
        atual = None;
        match senao {
            Some(s) if eh_if(s.kind()) => atual = Some(s),
            Some(s) => fim = Some(s),
            None => {}
        }
    }
    (ramos, fim)
}

fn cabeca_de_cadeia(n: Node) -> bool {
    let Some(p) = n.parent() else { return true };
    match n.kind() {
        "if_statement" => {
            !(p.kind() == "if_statement" && p.child_by_field_name("alternative") == Some(n))
        }
        _ => !(p.kind() == "if_expression" && n.prev_sibling().is_some_and(|s| s.kind() == "else")),
    }
}

/// BUG004 e BUG005 numa cadeia de if.
fn ifs(ctx: &Contexto, n: Node, v: &mut Vec<Violacao>) {
    let f = ctx.fonte;
    let (ramos, fim) = cadeia(n);
    let mut vistas: HashMap<String, usize> = HashMap::new();
    for (c, _) in &ramos {
        let t = normal(texto(*c, f));
        if let Some(l) = vistas.get(&t) {
            add(
                ctx,
                v,
                "BUG004",
                *c,
                format!("condição repetida (já testada na linha {l})"),
                "o ramo repetido nunca roda; corrija a condição",
                texto(*c, f),
            );
        } else {
            vistas.insert(t, linha(*c));
        }
    }
    let mut corpos: Vec<Node> = ramos.iter().map(|(_, e)| *e).collect();
    corpos.extend(fim);
    corpos_iguais(ctx, n, &corpos, fim.is_some(), v);
}

fn corpos_iguais(
    ctx: &Contexto,
    dono: Node,
    corpos: &[Node],
    completo: bool,
    v: &mut Vec<Violacao>,
) {
    if corpos.len() < 2 {
        return;
    }
    let f = ctx.fonte;
    let textos: Vec<String> = corpos.iter().map(|c| normal(texto(*c, f))).collect();
    if completo && textos.iter().all(|t| *t == textos[0]) {
        add(
            ctx,
            v,
            "BUG005",
            dono,
            "todos os ramos fazem a mesma coisa".into(),
            "a condição não muda nada: tire o if/when ou corrija um dos ramos",
            &textos[0],
        );
        return;
    }
    for i in 1..corpos.len() {
        let grande = instrucoes(corpos[i]).len() >= 2;
        if let Some(j) = (0..i).find(|j| textos[*j] == textos[i]).filter(|_| grande) {
            add(
                ctx,
                v,
                "BUG005",
                corpos[i],
                format!("ramo igual ao da linha {}", linha(corpos[j])),
                "junte as condições num ramo só ou corrija o ramo copiado",
                &textos[i],
            );
        }
    }
}

/// BUG004 e BUG005 num when do Kotlin.
fn when(ctx: &Contexto, n: Node, v: &mut Vec<Violacao>) {
    let f = ctx.fonte;
    let mut vistas: HashMap<String, usize> = HashMap::new();
    let mut corpos = Vec::new();
    let mut tem_else = false;
    for e in filhos(n).into_iter().filter(|e| e.kind() == "when_entry") {
        tem_else |= filhos(e).iter().any(|c| c.kind() == "else");
        let nomeados: Vec<Node> = filhos(e).into_iter().filter(|c| c.is_named()).collect();
        let Some((corpo, condicoes)) = nomeados.split_last() else {
            continue;
        };
        for c in condicoes {
            let t = normal(texto(*c, f));
            if let Some(l) = vistas.get(&t) {
                add(
                    ctx,
                    v,
                    "BUG004",
                    *c,
                    format!("condição repetida (já testada na linha {l})"),
                    "o ramo repetido nunca roda; corrija a condição",
                    texto(*c, f),
                );
            } else {
                vistas.insert(t, linha(*c));
            }
        }
        corpos.push(*corpo);
    }
    corpos_iguais(ctx, n, &corpos, tem_else, v);
}

static DECIMAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9_]*\.[0-9_]+([eE][+-]?[0-9]+)?[dDfF]?$|^[0-9_]+[dDfF]$").expect("regex")
});

pub fn verificar(ctx: &Contexto, v: &mut Vec<Violacao>) {
    let f = ctx.fonte;
    visitar(ctx.raiz, |n| {
        match n.kind() {
            "finally_clause" | "finally_block" => finally(ctx, n, v),
            "binary_expression" => {
                let op = n.child_by_field_name("operator").map(|o| texto(o, f));
                let (Some(op), Some(e), Some(d)) = (
                    op,
                    n.child_by_field_name("left"),
                    n.child_by_field_name("right"),
                ) else {
                    return;
                };
                let (te, td) = (normal(texto(e, f)), normal(texto(d, f)));
                if OPERADORES_SIMETRICOS.contains(&op) && te == td && !te.contains('(') {
                    add(
                        ctx,
                        v,
                        "BUG002",
                        n,
                        format!("mesma expressão dos dois lados de {op}"),
                        "um dos lados está errado (copiado?)",
                        texto(n, f),
                    );
                }
                // BUG006: em Java, == compara referência, não o texto.
                if ctx.ling == Linguagem::Java
                    && matches!(op, "==" | "!=")
                    && (e.kind() == "string_literal" || d.kind() == "string_literal")
                {
                    add(
                        ctx,
                        v,
                        "BUG006",
                        n,
                        format!("String comparada com {op}"),
                        "use equals: \"valor\".equals(texto)",
                        texto(n, f),
                    );
                }
            }
            "assignment_expression" | "assignment" => {
                let op = n.child_by_field_name("operator").map(|o| texto(o, f));
                if let (Some("="), Some(e), Some(d)) = (
                    op,
                    n.child_by_field_name("left"),
                    n.child_by_field_name("right"),
                ) {
                    if normal(texto(e, f)) == normal(texto(d, f)) {
                        add(
                            ctx,
                            v,
                            "BUG003",
                            n,
                            "variável atribuída a ela mesma".into(),
                            "faltou o this. (this.x = x) ou o lado direito está errado",
                            texto(n, f),
                        );
                    }
                }
            }
            "if_statement" | "if_expression" if cabeca_de_cadeia(n) => ifs(ctx, n, v),
            "when_expression" => when(ctx, n, v),
            "catch_clause" | "catch_block" => {
                let Some((tipos, _, bloco)) = excecoes::catch(n, f, ctx.ling) else {
                    return;
                };
                if !tipos.contains(&"InterruptedException") {
                    return;
                }
                let mut trata = false;
                percorrer(bloco, &mut |x| {
                    trata |= matches!(x.kind(), "throw_statement" | "throw_expression")
                        || (x.kind() == "identifier" && texto(x, f) == "interrupt");
                    !trata
                });
                if !trata {
                    add(
                        ctx,
                        v,
                        "BUG008",
                        n,
                        "InterruptedException ignorada".into(),
                        "restaure o sinal: Thread.currentThread().interrupt() (ou relance)",
                        "InterruptedException",
                    );
                }
            }
            _ => {}
        }
        if let Some(c) = chamada(n, f) {
            if c.construtor
                && c.nome == "BigDecimal"
                && c.args.len() == 1
                && DECIMAL.is_match(texto(c.args[0], f).trim())
            {
                add(
                    ctx,
                    v,
                    "BUG007",
                    n,
                    format!("BigDecimal de double ({})", texto(c.args[0], f)),
                    "use BigDecimal(\"0.1\") ou BigDecimal.valueOf(0.1): o double já perdeu precisão",
                    "BigDecimal",
                );
            }
        }
    });
}

#[cfg(test)]
mod casos {
    use super::*;
    use crate::config::Limites;
    use crate::lang::Parsers;

    fn achar(fonte: &str, ling: Linguagem) -> Vec<(String, usize)> {
        let mut p = Parsers::novo();
        let t = p.ler(ling, fonte).unwrap();
        assert!(!t.root_node().has_error(), "sintaxe");
        let lim = Limites::default();
        let ctx = Contexto {
            arquivo: "A",
            fonte,
            ling,
            raiz: t.root_node(),
            limites: &lim,
            pacote: None,
            extras: None,
        };
        let mut v = Vec::new();
        verificar(&ctx, &mut v);
        v.into_iter()
            .map(|x| (x.codigo.to_string(), x.linha))
            .collect()
    }

    #[test]
    fn java() {
        let j = r#"class A {
  int f(int a, String s) {
    try { a = a; } finally { return 1; }
  }
  void g(int a, int b, String s) throws Exception {
    if (a == a) { }
    if (s == "x") { }
    if (a > 1) { x(); y(); } else if (b > 1) { z(); } else if (a > 1) { w(); }
    if (b > 2) { x(); y(); } else if (a < 0) { x(); y(); }
    BigDecimal v = new BigDecimal(0.1);
    BigDecimal ok = new BigDecimal("0.1");
    try { Thread.sleep(1); } catch (InterruptedException e) { log.warn("x"); }
    try { Thread.sleep(1); } catch (InterruptedException e) { Thread.currentThread().interrupt(); }
    for (int i : xs) { try { } finally { if (i > 0) continue; } }
  }
}"#;
        let mut v = achar(j, Linguagem::Java);
        v.sort();
        assert_eq!(
            v,
            [
                ("BUG001".to_string(), 3),
                ("BUG001".into(), 14),
                ("BUG002".into(), 6),
                ("BUG003".into(), 3),
                ("BUG004".into(), 8),
                ("BUG005".into(), 9),
                ("BUG006".into(), 7),
                ("BUG007".into(), 10),
                ("BUG008".into(), 12),
            ]
        );
    }

    #[test]
    fn kotlin() {
        let k = "fun f(t: Tipo, a: Int): Int {
    val x = if (a > 0) 1 else 1
    val y = when (t) {
        Tipo.A -> 1
        Tipo.A -> 2
        else -> 3
    }
    for (i in xs) {
        try { } finally { if (i > 0) continue }
    }
    return x + y
}";
        let v = achar(k, Linguagem::Kotlin);
        assert_eq!(
            v,
            [
                ("BUG005".to_string(), 2),
                ("BUG004".into(), 5),
                ("BUG001".into(), 9)
            ],
            "{v:?}"
        );
    }
}
