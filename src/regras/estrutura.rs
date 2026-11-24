//! Métricas de função e de classe (limites do detekt e do PMD):
//! CPX002 ciclomática, CPX003 aninhamento, CPX004 condição, CPX006 classe longa,
//! CPX007 parâmetros, CPX008 funções por classe, CPX009 return, SML001 God Class
//! e JV003 equals sem hashCode.

use super::Contexto;
use crate::lang::{self, Linguagem, eh_funcao, eh_lambda, filhos, linha, simbolo_dono, texto};
use crate::modelo::Violacao;
use std::collections::BTreeSet;
use tree_sitter::Node;

/// Pré-ordem com poda: `f` devolve `false` para não descer nos filhos do nó.
pub fn percorrer<'t>(n: Node<'t>, f: &mut impl FnMut(Node<'t>) -> bool) {
    if f(n) {
        for c in filhos(n) {
            percorrer(c, f);
        }
    }
}

fn eh_classe(kind: &str) -> bool {
    matches!(
        kind,
        "class_declaration" | "enum_declaration" | "record_declaration" | "object_declaration"
    )
}

/// Corpo de outra função ou classe: não pertence à função que está sendo medida.
fn outro_escopo(n: Node) -> bool {
    eh_funcao(n.kind())
        || eh_classe(n.kind())
        || matches!(
            n.kind(),
            "class_body" | "object_literal" | "companion_object"
        )
}

fn logico(n: Node) -> bool {
    n.kind() == "binary_expression"
        && n.child_by_field_name("operator")
            .is_some_and(|o| matches!(o.kind(), "&&" | "||"))
}

/// CPX002: 1 + pontos de decisão (if, laço, case, catch, ternário, && e ||).
pub fn ciclomatica(funcao: Node, ling: Linguagem) -> u32 {
    let Some(corpo) = lang::corpo_da_funcao(funcao) else {
        return 1;
    };
    let mut total = 1;
    percorrer(corpo, &mut |n| {
        if outro_escopo(n) {
            return false;
        }
        let decisao = match ling {
            Linguagem::Java => match n.kind() {
                "if_statement"
                | "for_statement"
                | "enhanced_for_statement"
                | "while_statement"
                | "do_statement"
                | "catch_clause"
                | "ternary_expression" => true,
                "switch_label" => n.child(0).is_some_and(|c| c.kind() == "case"),
                _ => logico(n),
            },
            Linguagem::Kotlin => match n.kind() {
                "if_expression" | "for_statement" | "while_statement" | "do_while_statement"
                | "catch_block" => true,
                "when_entry" => !filhos(n).iter().any(|c| c.kind() == "else"),
                _ => logico(n),
            },
        };
        if decisao {
            total += 1;
        }
        true
    });
    total
}

fn eh_bloco_de_controle(kind: &str) -> bool {
    matches!(
        kind,
        "if_statement"
            | "for_statement"
            | "enhanced_for_statement"
            | "while_statement"
            | "do_statement"
            | "try_statement"
            | "try_with_resources_statement"
            | "switch_expression"
            | "if_expression"
            | "do_while_statement"
            | "try_expression"
            | "when_expression"
    )
}

/// CPX003: maior profundidade de blocos de controle (else if não aprofunda) e a linha.
pub fn aninhamento(funcao: Node) -> (u32, usize) {
    fn rec(n: Node, d: u32, max: &mut (u32, usize)) {
        let else_if = matches!(n.kind(), "if_statement" | "if_expression")
            && n.prev_sibling().is_some_and(|p| p.kind() == "else");
        let novo = if eh_bloco_de_controle(n.kind()) && !else_if {
            d + 1
        } else {
            d
        };
        if novo > max.0 {
            *max = (novo, linha(n));
        }
        for c in filhos(n) {
            if !outro_escopo(c) {
                rec(c, novo, max);
            }
        }
    }
    let mut max = (0, linha(funcao));
    if let Some(corpo) = lang::corpo_da_funcao(funcao) {
        rec(corpo, 0, &mut max);
    }
    max
}

/// CPX009: `return` que saem da função (não os de lambdas nem `return@rotulo`).
pub fn retornos(funcao: Node, fonte: &str) -> u32 {
    let Some(corpo) = lang::corpo_da_funcao(funcao) else {
        return 0;
    };
    let mut n = 0;
    percorrer(corpo, &mut |x| {
        if outro_escopo(x) || eh_lambda(x.kind()) {
            return false;
        }
        match x.kind() {
            "return_statement" => n += 1,
            "return_expression" if !texto(x, fonte).starts_with("return@") => n += 1,
            _ => {}
        }
        true
    });
    n
}

/// CPX007: parâmetros declarados (construtores ficam de fora, como no detekt).
pub fn parametros(funcao: Node) -> Option<u32> {
    match funcao.kind() {
        "method_declaration" => funcao.child_by_field_name("parameters").map(|p| {
            filhos(p)
                .iter()
                .filter(|c| matches!(c.kind(), "formal_parameter" | "spread_parameter"))
                .count() as u32
        }),
        "function_declaration" => filhos(funcao)
            .into_iter()
            .find(|c| c.kind() == "function_value_parameters")
            .map(|p| filhos(p).iter().filter(|c| c.kind() == "parameter").count() as u32),
        _ => None,
    }
}

/// CPX004: condições de if/while com mais operandos de && e || que o limite.
fn condicoes(ctx: &Contexto, funcao: Node, v: &mut Vec<Violacao>) {
    let Some(corpo) = lang::corpo_da_funcao(funcao) else {
        return;
    };
    let lim = ctx.limites.condicao_operadores;
    percorrer(corpo, &mut |n| {
        if outro_escopo(n) {
            return false;
        }
        let cond = match n.kind() {
            "if_statement" | "while_statement" | "do_statement" | "if_expression" => {
                n.child_by_field_name("condition")
            }
            _ => None,
        };
        if let Some(c) = cond {
            let mut ops = 0;
            percorrer(c, &mut |x| {
                if eh_lambda(x.kind()) {
                    return false;
                }
                if logico(x) {
                    ops += 1;
                }
                true
            });
            if ops + 1 > lim {
                v.push(Violacao::nova(
                    "CPX004",
                    ctx.arquivo,
                    linha(c),
                    format!("condição com {} operandos>{lim}", ops + 1),
                    "dê nome às partes da condição em variáveis ou funções",
                    simbolo_dono(n, ctx.fonte),
                    texto(c, ctx.fonte),
                    None,
                ));
            }
        }
        true
    });
}

/// Métricas por função.
pub fn funcao(ctx: &Contexto, f: Node, v: &mut Vec<Violacao>) {
    let lim = ctx.limites;
    let pos = f.child_by_field_name("name").unwrap_or(f);
    let simbolo = simbolo_dono(f, ctx.fonte);
    let mut medida =
        |codigo: &'static str, valor: u32, limite: u32, oque: &str, dica: &str, l: usize| {
            if valor > limite {
                v.push(Violacao::nova(
                    codigo,
                    ctx.arquivo,
                    l,
                    format!("{oque} {valor}>{limite}"),
                    dica,
                    simbolo.clone(),
                    "",
                    Some(valor),
                ));
            }
        };
    medida(
        "CPX002",
        ciclomatica(f, ctx.ling),
        lim.complexidade_ciclomatica,
        "complexidade ciclomática",
        "separe os casos em funções ou troque a cadeia de if por polimorfismo ou tabela",
        linha(pos),
    );
    let (prof, l) = aninhamento(f);
    medida(
        "CPX003",
        prof,
        lim.aninhamento,
        "aninhamento",
        "retorne cedo (guard clause) e extraia o bloco interno em uma função",
        l,
    );
    medida(
        "CPX009",
        retornos(f, ctx.fonte),
        lim.retornos,
        "return",
        "concentre a saída (when/expressão) ou extraia os casos em funções",
        linha(pos),
    );
    if let Some(p) = parametros(f) {
        medida(
            "CPX007",
            p,
            lim.parametros,
            "parâmetros",
            "agrupe os parâmetros que andam juntos num tipo",
            linha(pos),
        );
    }
    condicoes(ctx, f, v);
}

/// Primeira classe acima do nó.
fn classe_dona(n: Node) -> Option<Node> {
    let mut p = n.parent();
    while let Some(x) = p {
        if eh_classe(x.kind()) {
            return Some(x);
        }
        p = x.parent();
    }
    None
}

fn metodos_da_classe<'t>(classe: Node<'t>) -> Vec<Node<'t>> {
    let mut v = Vec::new();
    percorrer(classe, &mut |n| {
        if eh_funcao(n.kind()) {
            if classe_dona(n) == Some(classe) {
                v.push(n);
            }
            return false;
        }
        !(eh_classe(n.kind()) && n != classe)
    });
    v
}

fn eh_construtor(n: Node) -> bool {
    matches!(
        n.kind(),
        "constructor_declaration" | "compact_constructor_declaration" | "secondary_constructor"
    )
}

/// Nomes dos campos da classe (para a coesão do SML001).
fn campos(classe: Node, fonte: &str) -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    percorrer(classe, &mut |n| {
        if n != classe && (eh_classe(n.kind()) || eh_funcao(n.kind())) {
            return false;
        }
        match n.kind() {
            "field_declaration" => {
                for d in filhos(n)
                    .into_iter()
                    .filter(|c| c.kind() == "variable_declarator")
                {
                    if let Some(nome) = d.child_by_field_name("name") {
                        s.insert(texto(nome, fonte).to_string());
                    }
                }
            }
            "property_declaration" | "class_parameter" => {
                let mut achou = None;
                percorrer(n, &mut |x| {
                    if achou.is_none() && x.kind() == "identifier" {
                        achou = Some(texto(x, fonte).to_string());
                    }
                    achou.is_none()
                });
                s.extend(achou);
            }
            _ => {}
        }
        true
    });
    s
}

/// Campos que o método usa (por nome).
fn campos_usados(metodo: Node, campos: &BTreeSet<String>, fonte: &str) -> BTreeSet<String> {
    let mut usados = BTreeSet::new();
    if let Some(corpo) = lang::corpo_da_funcao(metodo) {
        percorrer(corpo, &mut |x| {
            if x.kind() == "identifier" {
                let t = texto(x, fonte);
                if campos.contains(t) {
                    usados.insert(t.to_string());
                }
            }
            true
        });
    }
    usados
}

fn nome_e_aridade<'a>(m: Node, fonte: &'a str) -> (&'a str, Option<u32>) {
    let nome = m
        .child_by_field_name("name")
        .map_or("", |n| texto(n, fonte));
    (nome, parametros(m))
}

/// Métricas por classe.
pub fn classe(ctx: &Contexto, c: Node, codigo: &[bool], v: &mut Vec<Violacao>) {
    let lim = ctx.limites;
    let pos = c.child_by_field_name("name").unwrap_or(c);
    let simbolo = simbolo_dono(pos, ctx.fonte);
    let metodos: Vec<Node> = metodos_da_classe(c)
        .into_iter()
        .filter(|m| !eh_construtor(*m))
        .collect();

    let linhas = super::complexidade::linhas(c, codigo);
    if linhas > lim.linhas_classe {
        v.push(Violacao::nova(
            "CPX006",
            ctx.arquivo,
            linha(pos),
            format!("classe com {linhas} linhas>{}", lim.linhas_classe),
            "separe as responsabilidades em classes menores",
            simbolo.clone(),
            "",
            Some(linhas),
        ));
    }
    let n = metodos.len() as u32;
    if n > lim.funcoes_por_classe {
        v.push(Violacao::nova(
            "CPX008",
            ctx.arquivo,
            linha(pos),
            format!("{n} funções>{}", lim.funcoes_por_classe),
            "separe a classe por caso de uso ou por grupo de funções que usam os mesmos dados",
            simbolo.clone(),
            "",
            Some(n),
        ));
    }

    // JV003: equals(Object) e hashCode() andam juntos.
    if ctx.ling == Linguagem::Java {
        let equals = metodos
            .iter()
            .find(|m| nome_e_aridade(**m, ctx.fonte) == ("equals", Some(1)));
        let hash = metodos
            .iter()
            .find(|m| nome_e_aridade(**m, ctx.fonte) == ("hashCode", Some(0)));
        let sozinho = match (equals, hash) {
            (Some(e), None) => Some((e, "equals sem hashCode")),
            (None, Some(h)) => Some((h, "hashCode sem equals")),
            _ => None,
        };
        if let Some((m, msg)) = sozinho {
            v.push(Violacao::nova(
                "JV003",
                ctx.arquivo,
                linha(m.child_by_field_name("name").unwrap_or(*m)),
                msg,
                "implemente os dois com os mesmos campos (ex.: Objects.hash)",
                simbolo.clone(),
                msg,
                None,
            ));
        }
    }

    // SML001: muito complexa (WMC ≥ 47) e pouco coesa (TCC < 1/3), como o PMD GodClass.
    let wmc: u32 = metodos.iter().map(|m| ciclomatica(*m, ctx.ling)).sum();
    if wmc >= 47 && metodos.len() >= 2 {
        let fs = campos(c, ctx.fonte);
        let usos: Vec<BTreeSet<String>> = metodos
            .iter()
            .map(|m| campos_usados(*m, &fs, ctx.fonte))
            .collect();
        let mut pares = 0u32;
        let mut ligados = 0u32;
        for i in 0..usos.len() {
            for j in i + 1..usos.len() {
                pares += 1;
                if !usos[i].is_disjoint(&usos[j]) {
                    ligados += 1;
                }
            }
        }
        let tcc = ligados as f64 / pares.max(1) as f64;
        if tcc < 1.0 / 3.0 {
            v.push(Violacao::nova(
                "SML001",
                ctx.arquivo,
                linha(pos),
                format!(
                    "classe faz demais: complexidade total {wmc}, coesão {}",
                    format!("{tcc:.2}").replace('.', ",")
                ),
                "quebre a classe pelos grupos de métodos que usam os mesmos campos",
                simbolo,
                "",
                None,
            ));
        }
    }
}

pub fn eh_classe_pub(kind: &str) -> bool {
    eh_classe(kind)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::config::Limites;
    use crate::lang::{Parsers, visitar};

    fn achar(fonte: &str, ling: Linguagem) -> Vec<Violacao> {
        let mut p = Parsers::novo();
        let t = p.ler(ling, fonte).unwrap();
        assert!(!t.root_node().has_error());
        let lim = Limites::default();
        let ctx = Contexto {
            arquivo: "A",
            fonte,
            ling,
            raiz: t.root_node(),
            limites: &lim,
        };
        let (v, _) = super::super::por_arquivo(&ctx);
        v
    }

    fn codigos(v: &[Violacao]) -> Vec<&str> {
        v.iter().map(|x| x.codigo).collect()
    }

    fn primeira_funcao(fonte: &str, ling: Linguagem, f: impl Fn(Node) -> u32) -> u32 {
        let mut p = Parsers::novo();
        let t = p.ler(ling, fonte).unwrap();
        let mut r = None;
        visitar(t.root_node(), |n| {
            if r.is_none() && eh_funcao(n.kind()) {
                r = Some(f(n));
            }
        });
        r.unwrap()
    }

    #[test]
    fn ciclomatica_java_e_kotlin() {
        let j = "class A { int f(int x) { if (x > 0 && x < 9) { return 1; } for (;;) {} switch (x) { case 1: case 2: break; default: } return x > 3 ? 1 : 2; } }";
        // 1 + if + && + for + case + case + ternário
        assert_eq!(
            primeira_funcao(j, Linguagem::Java, |n| ciclomatica(n, Linguagem::Java)),
            7
        );
        let k = "fun f(x: Int) = when (x) { 1 -> a(); 2 -> b(); else -> c() }";
        assert_eq!(
            primeira_funcao(k, Linguagem::Kotlin, |n| ciclomatica(n, Linguagem::Kotlin)),
            3
        );
    }

    #[test]
    fn aninhamento_conta_blocos_e_ignora_else_if() {
        let j = "class A { void f() { if (a) {} else if (b) { for (;;) { while (c) { try { if (d) {} } catch (E e) {} } } } } }";
        assert_eq!(primeira_funcao(j, Linguagem::Java, |n| aninhamento(n).0), 5);
    }

    #[test]
    fn retornos_ignoram_lambda() {
        let k = "fun f(l: List<Int>): Int { l.forEach { if (it > 0) return@forEach }; if (a) return 1; return 2 }";
        assert_eq!(primeira_funcao(k, Linguagem::Kotlin, |n| retornos(n, k)), 2);
    }

    #[test]
    fn parametros_e_condicao() {
        let j = "class A { void f(int a, int b, int c, int d, int e, int f, int g) { if (a > 0 && b > 0 || c > 0 && d > 0 || e > 0) {} } }";
        let v = achar(j, Linguagem::Java);
        assert!(codigos(&v).contains(&"CPX007"), "{v:?}");
        assert!(codigos(&v).contains(&"CPX004"), "{v:?}");
    }

    #[test]
    fn equals_sem_hashcode() {
        let j = "class A { public boolean equals(Object o) { return true; } }";
        let v = achar(j, Linguagem::Java);
        assert_eq!(codigos(&v), ["JV003"]);
        let ok = "class A { public boolean equals(Object o) { return true; } public int hashCode() { return 1; } }";
        assert!(!codigos(&achar(ok, Linguagem::Java)).contains(&"JV003"));
    }

    #[test]
    fn muitas_funcoes() {
        let metodos: String = (0..12).map(|i| format!("void m{i}() {{}} ")).collect();
        let v = achar(
            &format!("class A {{ A() {{}} {metodos} }}"),
            Linguagem::Java,
        );
        assert!(codigos(&v).contains(&"CPX008"), "{v:?}");
    }
}
