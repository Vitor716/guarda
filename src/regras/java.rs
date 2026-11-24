//! Regras de Java: JV001 System.out/printStackTrace, JV002 recurso sem try-with-resources,
//! JV004 Optional como campo ou parâmetro, JV005 java.util.Date/Calendar (Java e Kotlin).

use super::Contexto;
use crate::lang::{self, Linguagem, linha, simbolo_dono, texto, visitar};
use crate::modelo::Violacao;
use tree_sitter::Node;

/// Tipos que abrem recurso e precisam de close (lista conservadora do PMD).
const RECURSOS: &[&str] = &[
    "FileInputStream",
    "FileOutputStream",
    "FileReader",
    "FileWriter",
    "BufferedReader",
    "BufferedWriter",
    "BufferedInputStream",
    "BufferedOutputStream",
    "InputStreamReader",
    "OutputStreamWriter",
    "PrintWriter",
    "PrintStream",
    "Scanner",
    "ZipFile",
    "ZipInputStream",
    "ZipOutputStream",
    "RandomAccessFile",
    "ObjectInputStream",
    "ObjectOutputStream",
    "Socket",
    "ServerSocket",
];

fn dentro_de(n: Node, kind: &str) -> bool {
    let mut p = n.parent();
    while let Some(x) = p {
        if x.kind() == kind {
            return true;
        }
        p = x.parent();
    }
    false
}

fn eh_optional(tipo: Option<Node>, fonte: &str) -> bool {
    tipo.is_some_and(|t| {
        let s = texto(t, fonte);
        s == "Optional" || s.starts_with("Optional<") || s.starts_with("java.util.Optional")
    })
}

pub fn verificar(ctx: &Contexto, v: &mut Vec<Violacao>) {
    if ctx.ling == Linguagem::Java {
        java(ctx, v);
    }
    // JV005 vale para as duas linguagens, pelos imports.
    for imp in lang::imports(ctx.raiz, ctx.fonte) {
        let tipo = format!("{}.{}", imp.pacote, imp.nome);
        if matches!(
            tipo.as_str(),
            "java.util.Date" | "java.util.Calendar" | "java.util.GregorianCalendar" | "java.util.*"
        ) && imp.nome != "*"
        {
            v.push(Violacao::nova(
                "JV005",
                ctx.arquivo,
                imp.linha,
                format!("{} (API antiga de data)", imp.nome),
                "use java.time (Instant, LocalDate, ZonedDateTime)",
                "",
                &imp.texto,
                None,
            ));
        }
    }
}

fn java(ctx: &Contexto, v: &mut Vec<Violacao>) {
    let f = ctx.fonte;
    let mut nova = |codigo: &'static str, n: Node, msg: String, dica: &str| {
        v.push(Violacao::nova(
            codigo,
            ctx.arquivo,
            linha(n),
            msg,
            dica,
            simbolo_dono(n, f),
            texto(n, f),
            None,
        ));
    };
    visitar(ctx.raiz, |n| match n.kind() {
        "method_invocation" => {
            let objeto = n.child_by_field_name("object").map(|o| texto(o, f));
            let nome = n.child_by_field_name("name").map_or("", |x| texto(x, f));
            let saida = matches!(objeto, Some("System.out" | "System.err"));
            let pilha = nome == "printStackTrace"
                && n.child_by_field_name("arguments")
                    .is_some_and(|a| a.named_child_count() == 0);
            if saida || pilha {
                let msg = if saida {
                    format!("{}.{nome}", objeto.unwrap_or(""))
                } else {
                    "printStackTrace()".into()
                };
                nova(
                    "JV001",
                    n,
                    msg,
                    "use o logger (com a exceção como último argumento)",
                );
            }
            // Files.newBufferedReader(...) e afins também abrem recurso.
            if objeto == Some("Files")
                && nome.starts_with("new")
                && n.parent()
                    .is_some_and(|p| p.kind() == "variable_declarator")
                && !dentro_de(n, "resource_specification")
            {
                nova(
                    "JV002",
                    n,
                    format!("Files.{nome} fora de try-with-resources"),
                    "abra o recurso em try (...) { }",
                );
            }
        }
        "object_creation_expression" => {
            let tipo = n.child_by_field_name("type").map_or("", |t| texto(t, f));
            let tipo = tipo.split('<').next().unwrap_or(tipo);
            // Só a criação guardada numa variável local; dentro de outra criação
            // (new BufferedReader(new FileReader(f))) o externo é que fecha.
            let local = n
                .parent()
                .is_some_and(|p| p.kind() == "variable_declarator")
                && dentro_de(n, "local_variable_declaration");
            if RECURSOS.contains(&tipo) && local && !dentro_de(n, "resource_specification") {
                nova(
                    "JV002",
                    n,
                    format!("new {tipo} fora de try-with-resources"),
                    "abra o recurso em try (...) { }",
                );
            }
        }
        "field_declaration" => {
            if eh_optional(n.child_by_field_name("type"), f) {
                nova(
                    "JV004",
                    n,
                    "Optional como campo".into(),
                    "guarde o valor anulável e devolva Optional no getter",
                );
            }
        }
        "formal_parameter"
            if eh_optional(n.child_by_field_name("type"), f) => {
                nova(
                    "JV004",
                    n,
                    "Optional como parâmetro".into(),
                    "receba o valor (anulável) ou crie duas versões da função",
                );
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
    fn regras_java() {
        let j = "import java.util.Date;\nclass A {\nprivate Optional<String> o;\nvoid f(Optional<Integer> p) {\nSystem.out.println(1);\ntry { } catch (E e) { e.printStackTrace(); }\nBufferedReader r = new BufferedReader(new FileReader(x));\ntry (BufferedReader ok = new BufferedReader(null)) { }\nvar s = Files.newBufferedReader(p);\n}\n}";
        let v = achar(j, Linguagem::Java);
        assert_eq!(
            v,
            [
                ("JV004".into(), 3),
                ("JV004".into(), 4),
                ("JV001".into(), 5),
                ("JV001".into(), 6),
                ("JV002".into(), 7),
                ("JV002".into(), 9),
                ("JV005".into(), 1),
            ]
        );
    }

    #[test]
    fn data_antiga_no_kotlin() {
        let k = "import java.util.Calendar\nimport java.time.Instant\nfun f() = 1";
        assert_eq!(achar(k, Linguagem::Kotlin), [("JV005".into(), 1)]);
    }
}
