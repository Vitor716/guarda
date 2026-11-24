//! Fluxo de controle e polimorfismo (Fowler, *Refactoring*, 2ª ed.):
//! CND001 retorno antecipado (Replace Nested Conditional with Guard Clauses),
//! CND002 else depois de return/throw (ESLint no-else-return, Pylint R1705),
//! CND003 condição que mistura && e || (Decompose Conditional),
//! POL001 cadeia de instanceof/is (Replace Conditional with Polymorphism; IntelliJ
//! "Chain of instanceof checks"), POL002 switch/when repetido (smell Repeated Switches).

use super::Contexto;
use super::estrutura::percorrer;
use crate::lang::{
    self, Linguagem, eh_comentario, eh_funcao, eh_lambda, filhos, linha, simbolo_dono, texto,
    visitar,
};
use crate::modelo::Violacao;
use std::collections::BTreeMap;
use tree_sitter::Node;

/// Instruções no corpo do if para valer a pena inverter (CND001).
const MIN_INSTRUCOES: usize = 3;
/// Ramos com teste de tipo sobre a mesma variável (POL001).
const MIN_RAMOS_DE_TIPO: usize = 3;
/// Casos constantes para o switch/when contar como "switch de tipo" (POL002).
const MIN_CASOS: usize = 3;

pub(crate) fn eh_if(kind: &str) -> bool {
    matches!(kind, "if_statement" | "if_expression")
}

/// (condição, então, senão) de um if das duas gramáticas.
pub(crate) fn partes(n: Node) -> (Option<Node>, Option<Node>, Option<Node>) {
    let cond = n.child_by_field_name("condition");
    if n.kind() == "if_statement" {
        return (
            cond,
            n.child_by_field_name("consequence"),
            n.child_by_field_name("alternative"),
        );
    }
    // Kotlin: if ( condição ) corpo [else corpo], sem nomes de campo nos corpos.
    let (mut entao, mut senao) = (None, None);
    let (mut fechou, mut viu_else) = (false, false);
    for c in filhos(n) {
        match c.kind() {
            ")" => fechou = true,
            "else" => viu_else = true,
            k if !c.is_named() || eh_comentario(k) || Some(c) == cond => {}
            _ if viu_else => senao = senao.or(Some(c)),
            _ if fechou => entao = entao.or(Some(c)),
            _ => {}
        }
    }
    (cond, entao, senao)
}

/// Instruções de um corpo: as do bloco, ou ele mesmo se não for bloco.
pub(crate) fn instrucoes(corpo: Node) -> Vec<Node> {
    if matches!(corpo.kind(), "block" | "control_structure_body") {
        filhos(corpo)
            .into_iter()
            .filter(|c| c.is_named() && !eh_comentario(c.kind()))
            .collect()
    } else {
        vec![corpo]
    }
}

/// Palavra do desvio (return, throw, break, continue), se o nó for um.
fn desvio<'a>(n: Node, fonte: &'a str) -> Option<&'a str> {
    match n.kind() {
        "return_statement" | "return_expression" => Some("return"),
        "throw_statement" | "throw_expression" => Some("throw"),
        "break_statement" => Some("break"),
        "continue_statement" => Some("continue"),
        // A gramática Kotlin lê break/continue soltos como identificador.
        "identifier" => {
            let t = texto(n, fonte);
            let base = t.split('@').next().unwrap_or(t);
            matches!(base, "break" | "continue").then_some(base)
        }
        _ => None,
    }
}

fn termina_em_desvio<'a>(corpo: Node, fonte: &'a str) -> Option<&'a str> {
    instrucoes(corpo).last().and_then(|n| desvio(*n, fonte))
}

/// O if é uma instrução (não o valor de uma expressão) e não é o `else if` de outro.
pub(crate) fn if_de_instrucao(n: Node) -> bool {
    let Some(p) = n.parent() else { return false };
    match n.kind() {
        "if_statement" => p.kind() != "if_statement",
        _ => matches!(
            p.kind(),
            "block" | "statements" | "lambda_literal" | "control_structure_body"
        ),
    }
}

fn tem_controle(corpo: Node) -> bool {
    let mut achou = false;
    percorrer(corpo, &mut |x| {
        if eh_lambda(x.kind()) || eh_funcao(x.kind()) {
            return false;
        }
        achou |= matches!(
            x.kind(),
            "if_statement"
                | "if_expression"
                | "for_statement"
                | "enhanced_for_statement"
                | "while_statement"
                | "do_statement"
                | "do_while_statement"
                | "try_statement"
                | "try_expression"
                | "switch_expression"
                | "when_expression"
        );
        !achou
    });
    achou
}

/// Corpo grande o bastante para que envolvê-lo num if custe leitura.
fn corpo_grande(corpo: Node) -> bool {
    instrucoes(corpo).len() >= MIN_INSTRUCOES || tem_controle(corpo)
}

fn bloco_da_funcao(f: Node) -> Option<Node> {
    let corpo = lang::corpo_da_funcao(f)?;
    if corpo.kind() == "function_body" {
        return filhos(corpo).into_iter().find(|c| c.kind() == "block");
    }
    (corpo.kind() == "block").then_some(corpo)
}

/// CND001 (a): a função termina num if sem else que envolve o resto do trabalho.
fn guarda_no_fim(ctx: &Contexto, f: Node, v: &mut Vec<Violacao>) {
    let Some(bloco) = bloco_da_funcao(f) else {
        return;
    };
    let Some(ultimo) = instrucoes(bloco).last().copied() else {
        return;
    };
    if !eh_if(ultimo.kind()) {
        return;
    }
    let (cond, entao, senao) = partes(ultimo);
    let (Some(cond), Some(entao), None) = (cond, entao, senao) else {
        return;
    };
    if !corpo_grande(entao) {
        return;
    }
    v.push(Violacao::nova(
        "CND001",
        ctx.arquivo,
        linha(ultimo),
        "if envolve o resto da função",
        "inverta a condição e retorne cedo (guard clause); o corpo sai do if",
        simbolo_dono(ultimo, ctx.fonte),
        texto(cond, ctx.fonte),
        None,
    ));
}

/// CND001 (b), CND002 e POL001 olham cada if de instrução.
fn ifs(ctx: &Contexto, n: Node, v: &mut Vec<Violacao>) {
    let (cond, entao, senao) = partes(n);
    let (Some(cond), Some(entao)) = (cond, entao) else {
        return;
    };
    let cond_txt = texto(cond, ctx.fonte);
    // CND004: if sem else cujo corpo é só outro if sem else (Sonar S1066).
    if senao.is_none()
        && let [unico] = instrucoes(entao)[..]
    {
        let (_, interno_entao, interno_senao) = partes(unico);
        if eh_if(unico.kind()) && interno_senao.is_none() && interno_entao.is_some() {
            v.push(Violacao::nova(
                "CND004",
                ctx.arquivo,
                linha(n),
                "if dentro de if sem else: dá para juntar",
                "junte as condições com && (ou extraia para uma função com nome)",
                simbolo_dono(n, ctx.fonte),
                cond_txt,
                None,
            ));
        }
    }
    if let Some(senao) = senao.filter(|s| !eh_if(s.kind())) {
        let saida_no_else = instrucoes(senao).len() == 1
            && termina_em_desvio(senao, ctx.fonte).is_some_and(|d| d == "return" || d == "throw");
        if saida_no_else && corpo_grande(entao) {
            v.push(Violacao::nova(
                "CND001",
                ctx.arquivo,
                linha(n),
                "caminho principal dentro do if; a saída está no else",
                "inverta a condição: trate a saída primeiro (guard clause) e tire o else",
                simbolo_dono(n, ctx.fonte),
                cond_txt,
                None,
            ));
        } else if let Some(d) = termina_em_desvio(entao, ctx.fonte) {
            v.push(Violacao::nova(
                "CND002",
                ctx.arquivo,
                linha(senao),
                format!("else depois de {d}"),
                "tire o else: o código dele já só roda quando a condição é falsa",
                simbolo_dono(n, ctx.fonte),
                cond_txt,
                None,
            ));
        }
    }
    cadeia_de_tipos(ctx, n, v);
}

/// Sujeito de um teste de tipo (`x instanceof A`, `x is A`), sem parênteses externos.
fn teste_de_tipo<'a>(cond: Node, fonte: &'a str) -> Option<&'a str> {
    let mut c = cond;
    while c.kind() == "parenthesized_expression" {
        c = c.named_child(0)?;
    }
    match c.kind() {
        "instanceof_expression" | "is_expression" => {
            let l = c.child_by_field_name("left")?;
            // `x !is A` também é is_expression; só conta o teste positivo.
            if texto(c, fonte).contains("!is") {
                return None;
            }
            Some(texto(l, fonte))
        }
        _ => None,
    }
}

/// POL001: if / else if testando o tipo da mesma variável em vários ramos.
fn cadeia_de_tipos(ctx: &Contexto, primeiro: Node, v: &mut Vec<Violacao>) {
    let mut por_sujeito: BTreeMap<&str, usize> = BTreeMap::new();
    let mut atual = Some(primeiro);
    while let Some(n) = atual {
        let (cond, _, senao) = partes(n);
        if let Some(s) = cond.and_then(|c| teste_de_tipo(c, ctx.fonte)) {
            *por_sujeito.entry(s).or_default() += 1;
        }
        atual = senao.filter(|s| eh_if(s.kind()));
    }
    let Some((sujeito, &ramos)) = por_sujeito.iter().max_by_key(|(_, n)| **n) else {
        return;
    };
    if ramos < MIN_RAMOS_DE_TIPO {
        return;
    }
    let (teste, dica) = match ctx.ling {
        Linguagem::Java => (
            "instanceof",
            "mova o comportamento para um método de cada tipo (polimorfismo)",
        ),
        Linguagem::Kotlin => (
            "is",
            "mova o comportamento para os tipos ou use sealed class com when exaustivo",
        ),
    };
    v.push(Violacao::nova(
        "POL001",
        ctx.arquivo,
        linha(primeiro),
        format!("{ramos} ramos com {teste} sobre {sujeito}"),
        dica,
        simbolo_dono(primeiro, ctx.fonte),
        sujeito,
        Some(ramos as u32),
    ));
}

/// CND003: condição de if/while/ternário que mistura && e || (sem contar lambdas).
fn condicao_mista(ctx: &Contexto, n: Node, v: &mut Vec<Violacao>) {
    let Some(cond) = n.child_by_field_name("condition") else {
        return;
    };
    let (mut e, mut ou) = (0u32, 0u32);
    percorrer(cond, &mut |x| {
        if eh_lambda(x.kind()) {
            return false;
        }
        if x.kind() == "binary_expression" {
            match x.child_by_field_name("operator").map(|o| o.kind()) {
                Some("&&") => e += 1,
                Some("||") => ou += 1,
                _ => {}
            }
        }
        true
    });
    // Acima do limite de operandos, a CPX004 já cobra a mesma condição.
    if e == 0 || ou == 0 || e + ou + 1 > ctx.limites.condicao_operadores {
        return;
    }
    v.push(Violacao::nova(
        "CND003",
        ctx.arquivo,
        linha(cond),
        "condição mistura && e ||",
        "extraia a condição para uma função (ou val) com nome de regra",
        simbolo_dono(n, ctx.fonte),
        texto(cond, ctx.fonte),
        None,
    ));
}

/// Um switch/when com sujeito: (sujeito normalizado, casos constantes, nó).
fn switch_constante<'t>(n: Node<'t>, fonte: &'t str) -> Option<(String, usize)> {
    let (sujeito, casos) = match n.kind() {
        "switch_expression" | "switch_statement" => {
            let corpo = n.child_by_field_name("body")?;
            let mut casos = 0;
            for grupo in filhos(corpo) {
                for rotulo in filhos(grupo)
                    .into_iter()
                    .filter(|c| c.kind() == "switch_label")
                {
                    let padrao = filhos(rotulo)
                        .iter()
                        .any(|c| matches!(c.kind(), "pattern" | "type_pattern" | "record_pattern"));
                    if !padrao && texto(rotulo, fonte).starts_with("case") {
                        casos += 1;
                    }
                }
            }
            (n.child_by_field_name("condition")?, casos)
        }
        "when_expression" => {
            let sujeito = filhos(n).into_iter().find(|c| c.kind() == "when_subject")?;
            let casos = filhos(n)
                .into_iter()
                .filter(|e| e.kind() == "when_entry")
                .filter(|e| {
                    let fs = filhos(*e);
                    !fs.iter().any(|c| c.kind() == "else")
                        && !fs
                            .iter()
                            .any(|c| matches!(c.kind(), "type_test" | "range_test"))
                })
                .count();
            (sujeito, casos)
        }
        _ => return None,
    };
    if casos < MIN_CASOS {
        return None;
    }
    let s: String = texto(sujeito, fonte)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let s = s.trim_start_matches('(').trim_end_matches(')');
    // `when (val x = expr)`: o sujeito é a expressão.
    let s = s.split_once('=').map_or(s, |(_, e)| e);
    let s = s.strip_prefix("this.").unwrap_or(s);
    Some((s.to_string(), casos))
}

/// POL002: o mesmo valor decidido por switch/when em mais de um lugar do arquivo.
fn switches_repetidos(ctx: &Contexto, v: &mut Vec<Violacao>) {
    let mut por_sujeito: BTreeMap<String, Vec<Node>> = BTreeMap::new();
    visitar(ctx.raiz, |n| {
        if let Some((s, _)) = switch_constante(n, ctx.fonte) {
            por_sujeito.entry(s).or_default().push(n);
        }
    });
    for (sujeito, nos) in por_sujeito {
        if nos.len() < 2 {
            continue;
        }
        let linhas: Vec<String> = nos.iter().map(|n| linha(*n).to_string()).collect();
        let palavra = match ctx.ling {
            Linguagem::Java => "switch",
            Linguagem::Kotlin => "when",
        };
        for n in &nos {
            v.push(Violacao::nova(
                "POL002",
                ctx.arquivo,
                linha(*n),
                format!(
                    "{palavra} sobre {sujeito} repetido (linhas {})",
                    linhas.join(", ")
                ),
                "troque os switches por polimorfismo: um método por tipo (enum com método abstrato ou sealed class)",
                simbolo_dono(*n, ctx.fonte),
                &sujeito,
                Some(nos.len() as u32),
            ));
        }
    }
}

pub fn verificar(ctx: &Contexto, v: &mut Vec<Violacao>) {
    visitar(ctx.raiz, |n| {
        let k = n.kind();
        if eh_funcao(k) {
            guarda_no_fim(ctx, n, v);
        } else if eh_if(k) {
            if if_de_instrucao(n) {
                ifs(ctx, n, v);
            }
            condicao_mista(ctx, n, v);
        } else if matches!(
            k,
            "while_statement" | "do_statement" | "do_while_statement" | "ternary_expression"
        ) {
            condicao_mista(ctx, n, v);
        }
    });
    switches_repetidos(ctx, v);
}

#[cfg(test)]
mod casos {
    use super::*;
    use crate::config::Limites;
    use crate::lang::Parsers;

    fn achar(fonte: &str, ling: Linguagem) -> Vec<(String, usize, String)> {
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
            .map(|x| (x.codigo.to_string(), x.linha, x.mensagem))
            .collect()
    }

    fn codigos(v: &[(String, usize, String)]) -> Vec<(&str, usize)> {
        v.iter().map(|(c, l, _)| (c.as_str(), *l)).collect()
    }

    #[test]
    fn guard_clause_java() {
        let j = "class A {
void salvar(U u) {
  if (u != null) {
    validar(u);
    repo.save(u);
    log(u);
  }
}
void curto(U u) {
  if (u != null) { repo.save(u); }
}
U buscar(int id) {
  if (id > 0) {
    var u = repo.find(id);
    log(u);
    return u;
  } else {
    throw new Invalido(id);
  }
}
}";
        let v = achar(j, Linguagem::Java);
        assert_eq!(codigos(&v), [("CND001", 3), ("CND001", 13)], "{v:?}");
    }

    #[test]
    fn else_depois_de_return() {
        let j = "class A { int f(int x) {
  if (x < 0) {
    return 0;
  } else {
    x = x * 2;
  }
  if (x > 9) { return 9; } else if (x > 5) { return 5; }
  return x;
} }";
        let v = achar(j, Linguagem::Java);
        assert_eq!(codigos(&v), [("CND002", 4)], "{v:?}");
        assert_eq!(v[0].2, "else depois de return");

        let k = "fun f(x: Int): Int {
    val y = if (x > 0) 1 else 2
    for (i in xs) { if (i > 0) { continue } else { soma(i) } }
    return y
}";
        let v = achar(k, Linguagem::Kotlin);
        assert_eq!(codigos(&v), [("CND002", 3)], "{v:?}");
        assert_eq!(v[0].2, "else depois de continue");
    }

    #[test]
    fn guard_clause_kotlin() {
        let k = "fun processar(p: Pedido?) {
    if (p != null) {
        validar(p)
        salvar(p)
        avisar(p)
    }
}
fun valor(p: Pedido) = if (p.ok) 1 else 2";
        let v = achar(k, Linguagem::Kotlin);
        assert_eq!(codigos(&v), [("CND001", 2)], "{v:?}");
    }

    #[test]
    fn condicao_mista() {
        let j = "class A { void f() {
  if (a && b || c) { x(); }
  if (a && b && c) { x(); }
  while (a || (b && c)) { x(); }
  if (a && b || c && d || e) { x(); }
} }";
        let v = achar(j, Linguagem::Java);
        // A última tem 5 operandos: é da CPX004, não da CND003.
        assert_eq!(codigos(&v), [("CND003", 2), ("CND003", 4)], "{v:?}");
    }

    #[test]
    fn cadeia_de_instanceof() {
        let j = "class A { double area(Forma f) {
  if (f instanceof Circulo c) { return c.r(); }
  else if (f instanceof Quadrado q) { return q.l(); }
  else if (f instanceof Triangulo t) { return t.b(); }
  return 0;
} }";
        let v = achar(j, Linguagem::Java);
        assert!(
            v.iter()
                .any(|(c, l, m)| c == "POL001" && *l == 2 && m == "3 ramos com instanceof sobre f"),
            "{v:?}"
        );

        let k = "fun area(f: Forma): Double {
    if (f is Circulo) return 1.0
    else if (f is Quadrado) return 2.0
    else if (f is Triangulo) return 3.0
    return 0.0
}
fun ok(f: Forma) = when (f) { is Circulo -> 1; is Quadrado -> 2; is Triangulo -> 3 }";
        let v = achar(k, Linguagem::Kotlin);
        assert_eq!(
            v.iter().filter(|(c, _, _)| c == "POL001").count(),
            1,
            "when com is é idiomático: {v:?}"
        );
    }

    #[test]
    fn switch_repetido() {
        let k = "class Frete(val tipo: Tipo) {
    fun prazo() = when (tipo) { Tipo.A -> 1; Tipo.B -> 2; Tipo.C -> 3 }
    fun preco() = when (this.tipo) { Tipo.A -> 10; Tipo.B -> 20; Tipo.C -> 5; else -> 30 }
    fun curto() = when (tipo) { Tipo.A -> 1; else -> 2 }
    fun selado(e: Evento) = when (e) { is X -> 1; is Y -> 2; is Z -> 3 }
}";
        let v = achar(k, Linguagem::Kotlin);
        assert_eq!(codigos(&v), [("POL002", 2), ("POL002", 3)], "{v:?}");
        assert_eq!(v[0].2, "when sobre tipo repetido (linhas 2, 3)");

        let j = "class F { int a(T t) { switch (t) { case A: return 1; case B: return 2; case C: return 3; default: return 0; } }
int b(T t) { return switch (t) { case A -> 1; case B -> 2; case C -> 3; default -> 0; }; } }";
        let v = achar(j, Linguagem::Java);
        assert_eq!(codigos(&v), [("POL002", 1), ("POL002", 2)], "{v:?}");
    }
}
