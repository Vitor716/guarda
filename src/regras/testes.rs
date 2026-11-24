//! Regras de código de teste (só rodam em `fontes.testes`):
//! TST001 teste sem asserção (PMD JUnitTestsShouldIncludeAssert), TST002 Thread.sleep em
//! teste (Sonar S2925), TST003 teste desativado sem motivo (Sonar S1607).

use super::Contexto;
use super::estrutura::percorrer;
use crate::lang::{self, filhos, linha, simbolo_dono, texto, visitar};
use crate::modelo::Violacao;
use regex::Regex;
use std::sync::LazyLock;
use tree_sitter::Node;

const ANOTACOES_DE_TESTE: &[&str] = &["Test", "ParameterizedTest", "RepeatedTest"];
const DESATIVADO: &[&str] = &["Disabled", "Ignore"];

/// Chamadas que verificam algo: JUnit, AssertJ, Hamcrest, Mockito/MockK, Kotest, MockMvc...
static ASSERCAO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(assert\w*|verify\w*|expect\w*|should\w*|fail|then|andExpect\w*|check\w*|coVerify\w*|confirmVerified|await\w*)$",
    )
    .expect("regex")
});

/// (nome, tem argumentos não vazios, linha) das anotações do nó.
fn anotacoes<'a>(n: Node, fonte: &'a str) -> Vec<(&'a str, bool, usize)> {
    let Some(m) = filhos(n).into_iter().find(|c| c.kind() == "modifiers") else {
        return Vec::new();
    };
    filhos(m)
        .into_iter()
        .filter(|a| matches!(a.kind(), "marker_annotation" | "annotation"))
        .map(|a| {
            let t = texto(a, fonte).trim_start_matches('@');
            let (nome, args) = match t.split_once('(') {
                Some((n, resto)) => (n, resto.trim_end_matches(')').trim()),
                None => (t, ""),
            };
            let nome = nome.trim().rsplit('.').next().unwrap_or(nome);
            let com_motivo = !args.is_empty() && args != "\"\"";
            (nome, com_motivo, linha(a))
        })
        .collect()
}

/// Nome da função chamada (o último segmento), nas duas linguagens.
fn chamada<'a>(n: Node, fonte: &'a str) -> Option<&'a str> {
    match n.kind() {
        "method_invocation" => n.child_by_field_name("name").map(|x| texto(x, fonte)),
        "call_expression" => n.named_child(0).map(|c| {
            let t = texto(c, fonte);
            t.rsplit('.').next().unwrap_or(t)
        }),
        // Kotlin infixo: `valor shouldBe 1`
        "infix_expression" => filhos(n)
            .into_iter()
            .filter(|c| c.kind() == "identifier")
            .nth(1)
            .or_else(|| n.named_child(1))
            .map(|x| texto(x, fonte)),
        _ => None,
    }
}

fn verifica_algo(funcao: Node, fonte: &str) -> bool {
    let Some(corpo) = lang::corpo_da_funcao(funcao) else {
        return true;
    };
    let mut achou = false;
    percorrer(corpo, &mut |n| {
        if let Some(nome) = chamada(n, fonte)
            && ASSERCAO.is_match(nome)
        {
            achou = true;
        }
        !achou
    });
    achou
}

pub fn verificar(ctx: &Contexto, v: &mut Vec<Violacao>) {
    let f = ctx.fonte;
    visitar(ctx.raiz, |n| match n.kind() {
        "method_declaration" | "function_declaration" | "class_declaration" => {
            let anots = anotacoes(n, f);
            let eh_teste = anots
                .iter()
                .any(|(nome, _, _)| ANOTACOES_DE_TESTE.contains(nome));
            // JUnit 4: @Test(expected = X.class) já é a asserção.
            let espera_excecao = anots.iter().any(|(nome, args, _)| *nome == "Test" && *args)
                && texto(n, f).contains("expected");
            let pos = n.child_by_field_name("name").unwrap_or(n);
            // contextLoads() vazio é o teste de fumaça do Spring: passa se o contexto sobe.
            let fumaca = texto(pos, f) == "contextLoads";
            if eh_teste
                && !espera_excecao
                && !fumaca
                && n.kind() != "class_declaration"
                && !verifica_algo(n, f)
            {
                v.push(Violacao::nova(
                    "TST001",
                    ctx.arquivo,
                    linha(pos),
                    "teste sem asserção",
                    "verifique o resultado (assertEquals, assertThat, verify, shouldBe...)",
                    simbolo_dono(n, f),
                    "",
                    None,
                ));
            }
            for (nome, com_motivo, l) in anots {
                if DESATIVADO.contains(&nome) && !com_motivo {
                    v.push(Violacao::nova(
                        "TST003",
                        ctx.arquivo,
                        l,
                        format!("@{nome} sem motivo"),
                        "diga por que está desativado e o ticket para reativar: @Disabled(\"ABC-12: ...\")",
                        simbolo_dono(n, f),
                        nome,
                        None,
                    ));
                }
            }
        }
        "method_invocation" => {
            let objeto = n.child_by_field_name("object").map(|o| texto(o, f));
            let nome = n.child_by_field_name("name").map(|x| texto(x, f));
            if objeto == Some("Thread") && nome == Some("sleep") {
                thread_sleep(ctx, n, v);
            }
        }
        "navigation_expression" if texto(n, f) == "Thread.sleep" => thread_sleep(ctx, n, v),
        _ => {}
    });
}

fn thread_sleep(ctx: &Contexto, n: Node, v: &mut Vec<Violacao>) {
    v.push(Violacao::nova(
        "TST002",
        ctx.arquivo,
        linha(n),
        "Thread.sleep em teste",
        "espere pela condição (Awaitility, CountDownLatch, runTest/advanceTimeBy)",
        simbolo_dono(n, ctx.fonte),
        "",
        None,
    ));
}

#[cfg(test)]
mod casos {
    use super::*;
    use crate::config::Limites;
    use crate::lang::{Linguagem, Parsers};

    fn achar(fonte: &str, ling: Linguagem) -> Vec<(String, usize)> {
        let mut p = Parsers::novo();
        let t = p.ler(ling, fonte).unwrap();
        assert!(!t.root_node().has_error(), "sintaxe");
        let lim = Limites::default();
        let ctx = Contexto {
            arquivo: "T",
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
    fn junit() {
        let j = "class T {
    @Test void semAssercao() { servico.salvar(x); }
    @Test void comAssercao() { assertEquals(1, f()); }
    @Test void comAssertJ() { assertThat(f()).isEqualTo(1); }
    @Test void comMock() { s.f(); verify(repo).save(any()); }
    @Test(expected = IllegalStateException.class) void junit4() { s.f(); }
    @Disabled @Test void desligado() { assertTrue(true); }
    @Disabled(\"ABC-1: instável no CI\") @Test void comMotivo() { assertTrue(true); }
    @Test void dorme() throws Exception { Thread.sleep(100); assertTrue(true); }
    @Test void stub() { when(r.f()).thenReturn(1); }
}";
        assert_eq!(
            achar(j, Linguagem::Java),
            [
                ("TST001".into(), 2),
                ("TST003".into(), 7),
                ("TST002".into(), 9),
                ("TST001".into(), 10),
            ]
        );
    }

    #[test]
    fn kotlin() {
        let k = "class T {
    @Test fun `sem assercao`() { servico.salvar(x) }
    @Test fun `kotest`() { f() shouldBe 1 }
    @Test fun `mockk`() { s.f(); coVerify { repo.save(any()) } }
    @Ignore @Test fun `desligado`() { assertEquals(1, 1) }
}";
        assert_eq!(
            achar(k, Linguagem::Kotlin),
            [("TST001".into(), 2), ("TST003".into(), 5)]
        );
    }
}
