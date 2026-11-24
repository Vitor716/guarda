//! Regras por arquivo. As regras de projeto (ARQ001-004) ficam em `arquitetura`.

pub mod complexidade;
pub mod estrutura;
pub mod excecoes;
pub mod extras;
pub mod java;
pub mod kotlin;
pub mod smells;
pub mod supressao;
pub mod testes;

use crate::config::Limites;
use crate::lang::{self, Linguagem, eh_funcao, eh_lambda, linha, simbolo_dono, visitar};
use crate::modelo::Violacao;
pub use supressao::Supressao;
use tree_sitter::Node;

pub struct Contexto<'a> {
    pub arquivo: &'a str,
    pub fonte: &'a str,
    pub ling: Linguagem,
    pub raiz: Node<'a>,
    pub limites: &'a Limites,
    /// Pacote declarado no arquivo.
    pub pacote: Option<&'a str>,
    /// Regras do projeto (proibir, personalizadas).
    pub extras: Option<&'a extras::Extras>,
}

pub fn por_arquivo(ctx: &Contexto) -> (Vec<Violacao>, Vec<Supressao>) {
    let (mut v, supressoes) = supressao::verificar(ctx.raiz, ctx.fonte, ctx.ling, ctx.arquivo);
    funcoes_e_classes(ctx, &mut v);
    excecoes::verificar(ctx, &mut v);
    smells::verificar(ctx, &mut v);
    java::verificar(ctx, &mut v);
    extras::verificar(ctx, &mut v);
    if ctx.ling == Linguagem::Kotlin {
        kotlin::verificar(ctx, &mut v);
    }
    (v, supressoes)
}

/// Código de teste: só as regras de teste (e as diretivas `guarda:ignorar`).
pub fn de_teste(ctx: &Contexto) -> (Vec<Violacao>, Vec<Supressao>) {
    let (v, supressoes) = supressao::verificar(ctx.raiz, ctx.fonte, ctx.ling, ctx.arquivo);
    let mut v: Vec<Violacao> = v.into_iter().filter(|x| x.codigo != "SUP001").collect();
    testes::verificar(ctx, &mut v);
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

fn funcoes_e_classes(ctx: &Contexto, v: &mut Vec<Violacao>) {
    let codigo = lang::linhas_de_codigo(ctx.fonte, ctx.raiz);
    let lim = ctx.limites;
    visitar(ctx.raiz, |n| {
        if estrutura::eh_classe_pub(n.kind()) {
            estrutura::classe(ctx, n, &codigo, v);
            return;
        }
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
        estrutura::funcao(ctx, n, v);
    });
}
