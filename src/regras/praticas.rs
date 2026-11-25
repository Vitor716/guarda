//! Boas práticas dos perfis padrão que faltavam: SML010 bloco vazio (Sonar S108/S1186,
//! detekt empty-blocks), SML011 relançar sem a causa (PMD PreserveStackTrace),
//! KT008 Flow/LiveData mutável exposto (Sonar S6305), JV006 literal à direita no equals
//! (PMD LiteralsFirstInComparisons).

use super::Contexto;
use super::estrutura::percorrer;
use super::excecoes;
use super::sintaxe::{anotacoes, chamada, literal, tem_modificador};
use crate::lang::{Linguagem, eh_funcao, eh_lambda, filhos, linha, simbolo_dono, texto, visitar};
use crate::modelo::Violacao;
use tree_sitter::Node;

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

/// Onde um bloco vazio está (None: lugar onde vazio é normal ou já é de outra regra).
fn dono_do_bloco<'t>(b: Node<'t>, fonte: &str) -> Option<&'static str> {
    let p = b.parent()?;
    let lugar = match p.kind() {
        "if_statement" | "if_expression" => "if/else",
        "for_statement" | "enhanced_for_statement" => "for",
        "while_statement" | "do_statement" | "do_while_statement" => "laço",
        "try_statement" | "try_with_resources_statement" | "try_expression" => "try",
        "finally_clause" | "finally_block" => "finally",
        "synchronized_statement" => "synchronized",
        "method_declaration" => "método",
        "function_body" => {
            let f = p.parent().filter(|f| f.kind() == "function_declaration")?;
            if tem_modificador(f, fonte, "override") || tem_modificador(f, fonte, "open") {
                return None;
            }
            "função"
        }
        _ => return None,
    };
    if matches!(lugar, "método" | "função") {
        let f = if lugar == "método" { p } else { p.parent()? };
        let anonima = f
            .parent()
            .and_then(|c| c.parent())
            .is_some_and(|c| c.kind() == "object_creation_expression");
        if anonima {
            return None;
        }
        // Anotado (@Override, @Pointcut, @PostConstruct...), default de interface ou abstrato:
        // vazio é intencional.
        let em_interface = {
            let mut x = f.parent();
            let mut achou = false;
            while let Some(y) = x {
                if y.kind() == "interface_declaration"
                    || (y.kind() == "class_declaration"
                        && filhos(y).iter().any(|c| c.kind() == "interface"))
                {
                    achou = true;
                    break;
                }
                if matches!(
                    y.kind(),
                    "class_declaration" | "object_declaration" | "enum_declaration"
                ) {
                    break;
                }
                x = y.parent();
            }
            achou
        };
        if !anotacoes(f, fonte).is_empty() || em_interface {
            return None;
        }
    }
    Some(lugar)
}

pub fn verificar(ctx: &Contexto, v: &mut Vec<Violacao>) {
    let f = ctx.fonte;
    visitar(ctx.raiz, |n| match n.kind() {
        "block" if n.named_child_count() == 0 => {
            if let Some(lugar) = dono_do_bloco(n, f) {
                add(
                    ctx,
                    v,
                    "SML010",
                    n,
                    format!("bloco vazio ({lugar})"),
                    "complete, remova ou comente por que fica vazio",
                    lugar,
                );
            }
        }
        "catch_clause" | "catch_block" => {
            let Some((_, nome, bloco)) = excecoes::catch(n, f, ctx.ling) else {
                return;
            };
            if nome.is_empty() || excecoes::ignorada_de_proposito(nome) {
                return;
            }
            let mut usa_no_bloco = false;
            visitar(bloco, |y| {
                usa_no_bloco |= y.kind() == "identifier" && texto(y, f) == nome;
            });
            if usa_no_bloco {
                return;
            }
            percorrer(bloco, &mut |x| {
                if eh_lambda(x.kind()) || eh_funcao(x.kind()) {
                    return false;
                }
                if matches!(x.kind(), "throw_statement" | "throw_expression") {
                    let nova = x
                        .named_child(0)
                        .and_then(|e| chamada(e, f))
                        .filter(|c| c.construtor);
                    if let Some(c) = nova {
                        let usa = c.args.iter().any(|a| {
                            let mut achou = false;
                            visitar(*a, |y| {
                                achou |= y.kind() == "identifier" && texto(y, f) == nome
                            });
                            achou
                        });
                        if !usa && !texto(bloco, f).contains("initCause") {
                            add(
                                ctx,
                                v,
                                "SML011",
                                x,
                                format!("{} lançada sem a causa ({nome})", c.nome),
                                "passe a exceção original como causa: throw X(\"...\", e)",
                                c.nome,
                            );
                        }
                    }
                    return false;
                }
                true
            });
        }
        "property_declaration" if ctx.ling == Linguagem::Kotlin => {
            let t = texto(n, f);
            let mutavel = ["MutableStateFlow", "MutableSharedFlow", "MutableLiveData"]
                .into_iter()
                .find(|m| t.contains(&format!(": {m}")) || t.contains(&format!("= {m}")));
            let dentro_de_classe = n.parent().is_some_and(|p| p.kind() == "class_body");
            let escondido = ["private", "protected", "internal"]
                .iter()
                .any(|m| tem_modificador(n, f, m));
            if let Some(m) = mutavel.filter(|_| dentro_de_classe && !escondido) {
                add(
                    ctx,
                    v,
                    "KT008",
                    n,
                    format!("{m} exposto"),
                    "deixe o mutável private e exponha StateFlow/SharedFlow/LiveData (asStateFlow())",
                    m,
                );
            }
        }
        "method_invocation" if ctx.ling == Linguagem::Java => {
            let Some(c) = chamada(n, f) else { return };
            let receptor_literal = c.receptor.is_some_and(|r| r.kind() == "string_literal");
            if matches!(c.nome, "equals" | "equalsIgnoreCase")
                && c.args.len() == 1
                && literal(c.args[0], f).is_some()
                && c.receptor.is_some()
                && !receptor_literal
            {
                add(
                    ctx,
                    v,
                    "JV006",
                    n,
                    format!("literal à direita no {}", c.nome),
                    "inverta: \"valor\".equals(texto) não quebra com null",
                    c.nome,
                );
            }
        }
        _ => {}
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
  void vazio() { }
  @Override public void hook() { }
  A() { }
  void f(String s) {
    if (s.equals("x")) { }
    if ("x".equals(s)) { x(); }
    try { ler(); } catch (IOException e) { throw new Falha("não leu"); }
    try { ler(); } catch (IOException e) { throw new Falha("não leu", e); }
    try { ler(); } catch (IOException e) { // ignora de propósito
    }
  }
}
interface I { default void x() { } }"#;
        assert_eq!(
            achar(j, Linguagem::Java),
            [
                ("SML010".to_string(), 2),
                ("JV006".into(), 6),
                ("SML010".into(), 6),
                ("SML011".into(), 8)
            ]
        );
    }

    #[test]
    fn kotlin() {
        let k = "class Vm {\n    val estado = MutableStateFlow(0)\n    private val _ok = MutableStateFlow(1)\n    val ok: StateFlow<Int> = _ok\n    override fun onCleared() {}\n    fun f() { if (a) {} }\n}";
        assert_eq!(
            achar(k, Linguagem::Kotlin),
            [("KT008".to_string(), 2), ("SML010".into(), 6)]
        );
    }

    #[test]
    fn calibrado_nos_repos_do_github() {
        // Listener anônimo vazio (Android) e catch que loga a exceção antes de trocar por uma
        // de negócio (crud-cliente, dom-api): intencionais.
        let j = r#"class A {
  void f() {
    b.setOnClick(new DialogInterface.OnClickListener() {
      public void onClick(DialogInterface d, int w) { }
    });
    try { salvar(); } catch (Exception e) { log.error("salvar", e); throw new ErroDeNegocio(ERRO); }
    try { salvar(); } catch (Exception e) { throw new ErroDeNegocio(ERRO); }
  }
}"#;
        assert_eq!(achar(j, Linguagem::Java), [("SML011".to_string(), 7)]);
    }
}
