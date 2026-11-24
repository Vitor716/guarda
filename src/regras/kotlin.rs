//! Regras de Kotlin (detekt): KT001 `!!`, KT002 GlobalScope, KT003 Thread.sleep em
//! corrotina, KT004 suspend fun que retorna Flow, KT005 Dispatchers fixo, KT006 data class
//! com var, KT007 println/printStackTrace.

use super::Contexto;
use crate::lang::{filhos, linha, simbolo_dono, texto, visitar};
use crate::modelo::Violacao;
use tree_sitter::Node;

fn tem_modificador(n: Node, fonte: &str, palavra: &str) -> bool {
    filhos(n)
        .into_iter()
        .find(|c| c.kind() == "modifiers")
        .is_some_and(|m| texto(m, fonte).split_whitespace().any(|w| w == palavra))
}

/// Está numa função suspend ou num lambda passado a um construtor de corrotina?
fn em_corrotina(n: Node, fonte: &str) -> bool {
    let mut p = n.parent();
    while let Some(x) = p {
        match x.kind() {
            "function_declaration" => return tem_modificador(x, fonte, "suspend"),
            "call_expression" => {
                let alvo = x.named_child(0).map_or("", |c| texto(c, fonte));
                let nome = alvo.rsplit('.').next().unwrap_or(alvo);
                if matches!(
                    nome,
                    "launch"
                        | "async"
                        | "runBlocking"
                        | "withContext"
                        | "coroutineScope"
                        | "supervisorScope"
                ) {
                    return true;
                }
            }
            _ => {}
        }
        p = x.parent();
    }
    false
}

fn em_parametro(n: Node) -> bool {
    let mut p = n.parent();
    while let Some(x) = p {
        match x.kind() {
            "parameter" | "class_parameter" => return true,
            "function_body" | "class_body" | "block" => return false,
            _ => {}
        }
        p = x.parent();
    }
    false
}

pub fn verificar(ctx: &Contexto, v: &mut Vec<Violacao>) {
    let f = ctx.fonte;
    let linhas: Vec<&str> = f.lines().collect();
    let mut nova = |codigo: &'static str, n: Node, msg: &str, dica: &str| {
        let l = linha(n);
        v.push(Violacao::nova(
            codigo,
            ctx.arquivo,
            l,
            msg,
            dica,
            simbolo_dono(n, f),
            linhas.get(l - 1).copied().unwrap_or(""),
            None,
        ));
    };
    visitar(ctx.raiz, |n| match n.kind() {
        "unary_expression" => {
            if n.child_by_field_name("operator")
                .is_some_and(|o| o.kind() == "!!")
            {
                nova(
                    "KT001",
                    n,
                    "uso de !!",
                    "troque por ?:, ?.let ou requireNotNull com mensagem",
                );
            }
        }
        "navigation_expression" => match texto(n, f) {
            "GlobalScope.launch" | "GlobalScope.async" => nova(
                "KT002",
                n,
                "corrotina no GlobalScope",
                "use um escopo com ciclo de vida (viewModelScope, um CoroutineScope injetado)",
            ),
            "Thread.sleep" if em_corrotina(n, f) => nova(
                "KT003",
                n,
                "Thread.sleep em corrotina",
                "use delay(): não trava a thread",
            ),
            "Dispatchers.IO" | "Dispatchers.Default" | "Dispatchers.Unconfined"
                if !em_parametro(n) =>
            {
                nova(
                    "KT005",
                    n,
                    "Dispatchers fixo no código",
                    "receba o CoroutineDispatcher por parâmetro (o padrão pode ser Dispatchers.IO)",
                )
            }
            t if t.ends_with(".printStackTrace") => nova(
                "KT007",
                n,
                "printStackTrace() em produção",
                "use o logger, com a exceção como último argumento",
            ),
            _ => {}
        },
        "call_expression" => {
            let alvo = n.named_child(0);
            if alvo.is_some_and(|a| {
                a.kind() == "identifier" && matches!(texto(a, f), "println" | "print")
            }) {
                nova("KT007", n, "println em produção", "use o logger");
            }
        }
        "function_declaration" => {
            let retorno = filhos(n)
                .into_iter()
                .rev()
                .find(|c| matches!(c.kind(), "user_type" | "nullable_type" | "type"))
                .map_or("", |t| texto(t, f));
            let fluxo = ["Flow", "StateFlow", "SharedFlow"]
                .iter()
                .any(|t| retorno == *t || retorno.starts_with(&format!("{t}<")));
            if fluxo && tem_modificador(n, f, "suspend") {
                let pos = n.child_by_field_name("name").unwrap_or(n);
                nova(
                    "KT004",
                    pos,
                    "suspend fun que retorna Flow",
                    "tire o suspend: o Flow já é assíncrono",
                );
            }
        }
        "class_declaration" if tem_modificador(n, f, "data") => {
            let mut vars: Vec<Node> = Vec::new();
            for c in filhos(n) {
                match c.kind() {
                    "primary_constructor" | "class_parameters" => {
                        let params = if c.kind() == "class_parameters" {
                            vec![c]
                        } else {
                            filhos(c)
                                .into_iter()
                                .filter(|x| x.kind() == "class_parameters")
                                .collect()
                        };
                        for ps in params {
                            vars.extend(filhos(ps).into_iter().filter(|p| {
                                p.kind() == "class_parameter"
                                    && filhos(*p).iter().any(|x| x.kind() == "var")
                            }));
                        }
                    }
                    "class_body" => vars.extend(filhos(c).into_iter().filter(|p| {
                        p.kind() == "property_declaration"
                            && filhos(*p).iter().any(|x| x.kind() == "var")
                    })),
                    _ => {}
                }
            }
            for p in vars {
                nova(
                    "KT006",
                    p,
                    "data class com var",
                    "use val e copy(...) para mudar",
                );
            }
        }
        _ => {}
    });
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::config::Limites;
    use crate::lang::{Linguagem, Parsers};

    fn achar(fonte: &str) -> Vec<(String, usize)> {
        let mut p = Parsers::novo();
        let t = p.ler(Linguagem::Kotlin, fonte).unwrap();
        assert!(!t.root_node().has_error(), "sintaxe");
        let lim = Limites::default();
        let ctx = Contexto {
            arquivo: "A",
            fonte,
            ling: Linguagem::Kotlin,
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
    fn regras_kotlin() {
        let k = "data class D(val a: Int, var b: Int)
class R(private val io: CoroutineDispatcher = Dispatchers.IO) {
    suspend fun f(): Flow<Int> {
        GlobalScope.launch { Thread.sleep(10) }
        withContext(Dispatchers.Default) { }
        println(x!!)
        return flow { }
    }
    fun g() { Thread.sleep(5); e.printStackTrace() }
}";
        let v = achar(k);
        assert_eq!(
            v,
            [
                ("KT006".into(), 1),
                ("KT004".into(), 3),
                ("KT002".into(), 4),
                ("KT003".into(), 4),
                ("KT005".into(), 5),
                ("KT007".into(), 6),
                ("KT001".into(), 6),
                ("KT007".into(), 9),
            ]
        );
    }
}
