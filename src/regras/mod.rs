//! Regras por arquivo. As regras de projeto (ARQ001) ficam em `arquitetura`.

pub mod complexidade;
pub mod supressao;

use crate::config::Limites;
use crate::lang::{self, Linguagem, eh_funcao, eh_lambda, linha, simbolo_dono, texto, visitar};
use crate::modelo::Violacao;
pub use supressao::Supressao;
use tree_sitter::Node;

pub struct Contexto<'a> {
    pub arquivo: &'a str,
    pub fonte: &'a str,
    pub ling: Linguagem,
    pub raiz: Node<'a>,
    pub limites: &'a Limites,
}

pub fn por_arquivo(ctx: &Contexto) -> (Vec<Violacao>, Vec<Supressao>) {
    let (mut v, supressoes) = supressao::verificar(ctx.raiz, ctx.fonte, ctx.ling, ctx.arquivo);
    funcoes(ctx, &mut v);
    if ctx.ling == Linguagem::Kotlin {
        kt001(ctx, &mut v);
    }
    (v, supressoes)
}

fn dentro_de_funcao(n: Node) -> bool {
    let mut p = n.parent();
    while let Some(x) = p {
        if eh_funcao(x.kind()) || eh_lambda(x.kind()) {
            return true;
        }
        p = x.parent();
    }
    false
}

fn funcoes(ctx: &Contexto, v: &mut Vec<Violacao>) {
    let codigo = lang::linhas_de_codigo(ctx.fonte, ctx.raiz);
    let lim = ctx.limites;
    visitar(ctx.raiz, |n| {
        if !eh_funcao(n.kind()) || lang::corpo_da_funcao(n).is_none() {
            return;
        }
        let pos = n.child_by_field_name("name").unwrap_or(n);
        let simbolo = simbolo_dono(n, ctx.fonte);

        let tamanho = complexidade::linhas(n, &codigo);
        if tamanho > lim.linhas_funcao {
            v.push(Violacao::nova(
                "CPX005",
                ctx.arquivo,
                linha(pos),
                format!("função com {tamanho} linhas>{}", lim.linhas_funcao),
                "divida em funções menores com nomes que expliquem cada passo",
                simbolo.clone(),
                "",
                Some(tamanho),
            ));
        }

        // A complexidade de funções aninhadas entra na da função externa.
        if !dentro_de_funcao(n) {
            let cc = complexidade::cognitiva(n, ctx.fonte, ctx.ling);
            if cc > lim.complexidade_cognitiva {
                v.push(Violacao::nova(
                    "CPX001",
                    ctx.arquivo,
                    linha(pos),
                    format!("complexidade {cc}>{}", lim.complexidade_cognitiva),
                    "extraia trechos em funções menores e reduza o aninhamento",
                    simbolo,
                    "",
                    Some(cc),
                ));
            }
        }
    });
}

/// KT001: `!!` (detekt UnsafeCallOnNullableType).
fn kt001(ctx: &Contexto, v: &mut Vec<Violacao>) {
    let linhas: Vec<&str> = ctx.fonte.lines().collect();
    visitar(ctx.raiz, |n| {
        if n.kind() != "unary_expression" {
            return;
        }
        let Some(op) = n.child_by_field_name("operator") else {
            return;
        };
        if op.kind() != "!!" {
            return;
        }
        let row = op.start_position().row;
        v.push(Violacao::nova(
            "KT001",
            ctx.arquivo,
            row + 1,
            "uso de !!",
            "troque por ?:, ?.let ou requireNotNull com mensagem",
            simbolo_dono(n, ctx.fonte),
            linhas.get(row).copied().unwrap_or(texto(n, ctx.fonte)),
            None,
        ));
    });
}
