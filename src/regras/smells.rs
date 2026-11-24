//! SML003 número mágico, SML007 código comentado, SML008 membro privado não usado,
//! SML009 TODO/FIXME sem ticket.

use super::Contexto;
use super::estrutura::percorrer;
use crate::lang::{Linguagem, filhos, linha, simbolo_dono, texto, visitar};
use crate::modelo::Violacao;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;
use tree_sitter::Node;

const LITERAIS: &[&str] = &[
    "decimal_integer_literal",
    "hex_integer_literal",
    "octal_integer_literal",
    "binary_integer_literal",
    "decimal_floating_point_literal",
    "number_literal",
    "float_literal",
];

/// detekt MagicNumber: -1, 0, 1 e 2 não são mágicos.
fn trivial(t: &str) -> bool {
    let limpo: String = t
        .trim_end_matches(['L', 'l', 'f', 'F', 'd', 'D', 'u', 'U'])
        .chars()
        .filter(|c| *c != '_')
        .collect();
    match limpo.parse::<f64>() {
        Ok(v) => [0.0, 1.0, 2.0].contains(&v),
        Err(_) => false,
    }
}

fn tem(n: Node, fonte: &str, palavra: &str) -> bool {
    filhos(n)
        .into_iter()
        .find(|c| c.kind() == "modifiers")
        .is_some_and(|m| texto(m, fonte).split_whitespace().any(|w| w == palavra))
}

/// Contexto onde número não é mágico: constante, anotação, hashCode, enum.
fn contexto_aceito(n: Node, fonte: &str) -> bool {
    let mut p = n.parent();
    while let Some(x) = p {
        match x.kind() {
            "annotation" | "marker_annotation" | "file_annotation" | "companion_object"
            | "enum_constant" | "enum_entry" => return true,
            "field_declaration" if tem(x, fonte, "static") && tem(x, fonte, "final") => {
                return true;
            }
            "property_declaration" if tem(x, fonte, "const") => return true,
            // Kotlin: argumento nomeado já diz o que o número é.
            "value_argument" if filhos(x).iter().any(|c| c.kind() == "=") => return true,
            "method_declaration" | "function_declaration" => {
                let nome = x
                    .child_by_field_name("name")
                    .map_or("", |c| texto(c, fonte));
                return nome == "hashCode";
            }
            // O corpo do companion object também é um class_body.
            "class_body" => {
                return x.parent().is_some_and(|c| c.kind() == "companion_object");
            }
            "source_file" | "program" => return false,
            _ => {}
        }
        p = x.parent();
    }
    false
}

static CODIGO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^(?:
            (?:return|if|else|for|while|import|package|val|var|fun|public|private|protected|throw|try|catch|switch|case)\b.*[;{)]?$
          | .*\)\s*;$
          | .*[=;]\s*$
          | [}{]\s*$
          | .*\)\s*\{$
        )",
    )
    .expect("regex")
});

static TICKET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Z][A-Z0-9]+-\d+|#\d+").expect("regex"));
static TODO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(TODO|FIXME)\b").expect("regex"));

fn parece_codigo(linha: &str) -> bool {
    let l = linha.trim();
    if l.is_empty() || l.contains("http://") || l.contains("https://") || l.contains("guarda:") {
        return false;
    }
    // Frase em português ou inglês termina com ponto e tem espaços entre palavras.
    if l.ends_with('.') || l.ends_with(':') {
        return false;
    }
    // Divisor de seção (// ===== Auxiliares =====): mais enfeite que texto.
    let enfeite = l.chars().filter(|c| "=-*#~_/+".contains(*c)).count();
    if enfeite * 10 > l.chars().count() * 4 {
        return false;
    }
    CODIGO.is_match(l)
        && (l.contains('(')
            || l.contains('=')
            || l.contains('{')
            || l.contains('}')
            || l.ends_with(';'))
}

pub fn verificar(ctx: &Contexto, v: &mut Vec<Violacao>) {
    let f = ctx.fonte;
    let mut comentado_ate: usize = 0;
    visitar(ctx.raiz, |n| {
        let k = n.kind();
        if LITERAIS.contains(&k) {
            let t = texto(n, f);
            if !trivial(t) && !contexto_aceito(n, f) {
                v.push(Violacao::nova(
                    "SML003",
                    ctx.arquivo,
                    linha(n),
                    format!("número mágico {t}"),
                    "dê nome ao número numa constante",
                    simbolo_dono(n, f),
                    t,
                    None,
                ));
            }
        } else if k == "line_comment" || k == "block_comment" {
            let t = texto(n, f);
            if TODO.is_match(t) && !TICKET.is_match(t) {
                // Num bloco de comentário, a linha do TODO, não a do `/**`.
                let deslocamento = t.lines().position(|l| TODO.is_match(l)).unwrap_or(0);
                v.push(Violacao::nova(
                    "SML009",
                    ctx.arquivo,
                    linha(n) + deslocamento,
                    "TODO/FIXME sem ticket",
                    "aponte o ticket (ABC-123 ou #123) ou resolva agora",
                    simbolo_dono(n, f),
                    t,
                    None,
                ));
            }
            // Javadoc/KDoc pode ter exemplo de código: fica de fora.
            if t.starts_with("/**") {
                return;
            }
            let corpo = t
                .trim_start_matches("//")
                .trim_start_matches("/*")
                .trim_end_matches("*/");
            let codigo = corpo
                .lines()
                .map(|l| l.trim().trim_start_matches('*'))
                .any(parece_codigo);
            // Comentários de linha seguidos contam uma vez só.
            if codigo && linha(n) > comentado_ate + 1 {
                v.push(Violacao::nova(
                    "SML007",
                    ctx.arquivo,
                    linha(n),
                    "código comentado",
                    "apague: o histórico do git guarda o que saiu",
                    simbolo_dono(n, f),
                    t,
                    None,
                ));
            }
            if codigo {
                comentado_ate = n.end_position().row + 1;
            }
        }
    });
    nao_usados(ctx, v);
}

/// Anotações que indicam uso por reflexão (Lombok, JPA, Spring, serialização).
fn anotada(n: Node, fonte: &str) -> bool {
    filhos(n)
        .into_iter()
        .find(|c| c.kind() == "modifiers")
        .is_some_and(|m| texto(m, fonte).contains('@'))
}

fn classe_anotada(n: Node, fonte: &str) -> bool {
    let mut p = n.parent();
    while let Some(x) = p {
        if super::estrutura::eh_classe_pub(x.kind()) {
            return anotada(x, fonte);
        }
        p = x.parent();
    }
    false
}

/// SML008: membro privado cujo nome não aparece em mais nenhum lugar do arquivo.
fn nao_usados(ctx: &Contexto, v: &mut Vec<Violacao>) {
    let f = ctx.fonte;
    let mut ocorrencias: HashMap<&str, usize> = HashMap::new();
    percorrer(ctx.raiz, &mut |n| {
        if matches!(n.kind(), "identifier" | "simple_identifier") {
            *ocorrencias.entry(texto(n, f)).or_default() += 1;
        }
        true
    });
    let mut membros: Vec<(Node, &str, &str)> = Vec::new();
    visitar(ctx.raiz, |n| {
        let privado = match ctx.ling {
            Linguagem::Java => tem(n, f, "private"),
            Linguagem::Kotlin => tem(n, f, "private"),
        };
        if !privado || anotada(n, f) || classe_anotada(n, f) {
            return;
        }
        match n.kind() {
            "field_declaration" => {
                for d in filhos(n)
                    .into_iter()
                    .filter(|c| c.kind() == "variable_declarator")
                {
                    if let Some(nome) = d.child_by_field_name("name") {
                        membros.push((nome, texto(nome, f), "campo"));
                    }
                }
            }
            "method_declaration" | "function_declaration" => {
                if let Some(nome) = n.child_by_field_name("name") {
                    membros.push((nome, texto(nome, f), "função"));
                }
            }
            "property_declaration" => {
                let mut achou = None;
                percorrer(n, &mut |x| {
                    if achou.is_none() && x.kind() == "identifier" {
                        achou = Some(x);
                    }
                    achou.is_none()
                });
                if let Some(nome) = achou {
                    membros.push((nome, texto(nome, f), "propriedade"));
                }
            }
            _ => {}
        }
    });
    for (no, nome, tipo) in membros {
        if nome == "serialVersionUID" || nome.is_empty() {
            continue;
        }
        if ocorrencias.get(nome).copied().unwrap_or(0) <= 1 {
            v.push(Violacao::nova(
                "SML008",
                ctx.arquivo,
                linha(no),
                format!("{tipo} privado {nome} não usado"),
                "apague o que não é usado",
                simbolo_dono(no, f),
                nome,
                None,
            ));
        }
    }
}

#[cfg(test)]
mod testes {
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
        let j = "class A {
    private static final int LIMITE = 18;
    private int naoUsado;
    @Value(\"${x}\") private int injetado;
    // TODO arrumar depois
    // TODO ABC-12 trocar
    // if (x > 0) { enviar(x); }
    // return y;
    // Calcula o total do pedido.
    int f(int idade) { return idade >= 18 ? 1 : 0; }
    private void auxiliar() {}
    public int hashCode() { return 31 * LIMITE; }
}";
        let v = achar(j, Linguagem::Java);
        assert_eq!(
            v,
            [
                ("SML009".into(), 5),
                ("SML007".into(), 7),
                ("SML003".into(), 10),
                ("SML008".into(), 3),
                ("SML008".into(), 11),
            ]
        );
    }

    #[test]
    fn kotlin() {
        let k = "class A {
    companion object {
        const val X = 7
        val Y = 9
    }
    private val p = 30
    fun f() = g(tamanho = 40) + 50
}";
        let v = achar(k, Linguagem::Kotlin);
        assert_eq!(
            v,
            [
                ("SML003".into(), 6),
                ("SML003".into(), 7),
                ("SML008".into(), 6)
            ]
        );
    }

    #[test]
    fn trivial_e_codigo() {
        assert!(trivial("2") && trivial("0L") && trivial("1.0f") && !trivial("3"));
        assert!(parece_codigo("if (x > 0) { enviar(x); }"));
        assert!(parece_codigo("return y;"));
        assert!(!parece_codigo("Calcula o total do pedido."));
        assert!(!parece_codigo("veja https://exemplo.com/a=b"));
        assert!(!parece_codigo(
            "============== Métodos auxiliares =============="
        ));
        assert!(parece_codigo(
            "authorize.requestMatchers(\"/admin/**\").hasRole(\"ADMIN\");"
        ));
    }
}
