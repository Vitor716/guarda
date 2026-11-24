//! SML004 catch genérico, SML005 exceção engolida ou catch vazio, SML006 exceção genérica
//! lançada (detekt TooGenericExceptionCaught, SwallowedException, EmptyCatchBlock,
//! TooGenericExceptionThrown).

use super::Contexto;
use super::estrutura::percorrer;
use crate::lang::{Linguagem, eh_comentario, filhos, linha, simbolo_dono, texto, visitar};
use crate::modelo::Violacao;
use tree_sitter::Node;

/// Lista do detekt TooGenericExceptionCaught.
const GENERICAS_PEGAS: &[&str] = &[
    "ArrayIndexOutOfBoundsException",
    "Error",
    "Exception",
    "IllegalMonitorStateException",
    "IndexOutOfBoundsException",
    "NullPointerException",
    "RuntimeException",
    "Throwable",
];

/// Lista do detekt TooGenericExceptionThrown.
const GENERICAS_LANCADAS: &[&str] = &["Error", "Exception", "RuntimeException", "Throwable"];

/// detekt SwallowedException.ignoredExceptionTypes: exceções que é normal tratar sem repassar.
const PODEM_SER_ENGOLIDAS: &[&str] = &[
    "InterruptedException",
    "MalformedURLException",
    "NumberFormatException",
    "ParseException",
];

fn simples(t: &str) -> &str {
    let t = t.trim();
    let t = t.split('<').next().unwrap_or(t);
    t.rsplit('.').next().unwrap_or(t)
}

/// (tipos pegos, nome da variável, bloco) de um catch.
fn catch<'t>(
    n: Node<'t>,
    fonte: &'t str,
    ling: Linguagem,
) -> Option<(Vec<&'t str>, &'t str, Node<'t>)> {
    match ling {
        Linguagem::Java => {
            let param = filhos(n)
                .into_iter()
                .find(|c| c.kind() == "catch_formal_parameter")?;
            let tipos = filhos(param)
                .into_iter()
                .find(|c| c.kind() == "catch_type")
                .map(|t| {
                    filhos(t)
                        .into_iter()
                        .filter(|x| x.is_named())
                        .map(|x| simples(texto(x, fonte)))
                        .collect()
                })
                .unwrap_or_default();
            let nome = param
                .child_by_field_name("name")
                .map_or("", |x| texto(x, fonte));
            Some((tipos, nome, n.child_by_field_name("body")?))
        }
        Linguagem::Kotlin => {
            let fs = filhos(n);
            let nome = fs
                .iter()
                .find(|c| c.kind() == "identifier")
                .map_or("", |x| texto(*x, fonte));
            let tipos = fs
                .iter()
                .filter(|c| matches!(c.kind(), "user_type" | "type"))
                .map(|x| simples(texto(*x, fonte)))
                .collect();
            let bloco = fs.into_iter().find(|c| c.kind() == "block")?;
            Some((tipos, nome, bloco))
        }
    }
}

fn ignorada_de_proposito(nome: &str) -> bool {
    let n = nome.to_lowercase();
    n == "_" || n.starts_with("ignore") || n.starts_with("expected")
}

pub fn verificar(ctx: &Contexto, v: &mut Vec<Violacao>) {
    visitar(ctx.raiz, |n| match n.kind() {
        "catch_clause" | "catch_block" => {
            let Some((tipos, nome, bloco)) = catch(n, ctx.fonte, ctx.ling) else {
                return;
            };
            if let Some(t) = tipos.iter().find(|t| GENERICAS_PEGAS.contains(t)) {
                v.push(Violacao::nova(
                    "SML004",
                    ctx.arquivo,
                    linha(n),
                    format!("catch de {t}"),
                    "pegue só as exceções que este trecho sabe tratar",
                    simbolo_dono(n, ctx.fonte),
                    t,
                    None,
                ));
            }
            if ignorada_de_proposito(nome)
                || (!tipos.is_empty() && tipos.iter().all(|t| PODEM_SER_ENGOLIDAS.contains(t)))
            {
                return;
            }
            let conteudo: Vec<Node> = filhos(bloco)
                .into_iter()
                .filter(|c| c.is_named() && !eh_comentario(c.kind()))
                .collect();
            let mut usa = false;
            let mut lanca = false;
            percorrer(bloco, &mut |x| {
                if matches!(x.kind(), "throw_statement" | "throw_expression") {
                    lanca = true;
                }
                if x.kind() == "identifier" && texto(x, ctx.fonte) == nome {
                    usa = true;
                }
                !(usa && lanca)
            });
            let msg = if conteudo.is_empty() {
                Some("catch vazio")
            } else if !usa && !lanca {
                Some("exceção engolida: o catch não usa nem relança a exceção")
            } else {
                None
            };
            if let Some(m) = msg {
                v.push(Violacao::nova(
                    "SML005",
                    ctx.arquivo,
                    linha(n),
                    m,
                    "passe a exceção ao log ou relance com contexto; se for de propósito, chame-a de ignored",
                    simbolo_dono(n, ctx.fonte),
                    m,
                    None,
                ));
            }
        }
        "throw_statement" | "throw_expression" => {
            let tipo = filhos(n).into_iter().find_map(|c| match c.kind() {
                "object_creation_expression" => {
                    c.child_by_field_name("type").map(|t| texto(t, ctx.fonte))
                }
                "call_expression" => c
                    .named_child(0)
                    .filter(|x| x.kind() == "identifier")
                    .map(|x| texto(x, ctx.fonte)),
                _ => None,
            });
            if let Some(t) = tipo.map(simples).filter(|t| GENERICAS_LANCADAS.contains(t)) {
                v.push(Violacao::nova(
                    "SML006",
                    ctx.arquivo,
                    linha(n),
                    format!("lança {t} genérica"),
                    "crie uma exceção com nome do problema (ex.: UsuarioNaoEncontrado)",
                    simbolo_dono(n, ctx.fonte),
                    texto(n, ctx.fonte),
                    None,
                ));
            }
        }
        _ => {}
    });
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
        };
        let mut v = Vec::new();
        verificar(&ctx, &mut v);
        v.into_iter()
            .map(|x| (x.codigo.to_string(), x.linha))
            .collect()
    }

    #[test]
    fn java() {
        let j = "class A { void f() {\ntry { a(); } catch (Exception e) { }\ntry { a(); } catch (IOException e) { log.warn(\"x\"); }\ntry { a(); } catch (IOException e) { log.warn(\"x\", e); }\ntry { a(); } catch (IOException ignored) { }\nthrow new RuntimeException(\"x\");\n} }";
        let v = achar(j, Linguagem::Java);
        assert_eq!(
            v,
            [
                ("SML004".into(), 2),
                ("SML005".into(), 2),
                ("SML005".into(), 3),
                ("SML006".into(), 6)
            ]
        );
    }

    #[test]
    fn kotlin() {
        let k = "fun f() {\ntry { a() } catch (e: Throwable) { throw Falha(e) }\ntry { a() } catch (e: IOException) { }\nthrow Exception(\"x\")\n}";
        let v = achar(k, Linguagem::Kotlin);
        assert_eq!(
            v,
            [
                ("SML004".into(), 2),
                ("SML005".into(), 3),
                ("SML006".into(), 4)
            ]
        );
    }
}
