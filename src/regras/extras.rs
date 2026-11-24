//! Regras que vêm do projeto, não do catálogo:
//! - PRB001/PRB002: `regras.proibir` (imports e tipos proibidos em pacotes);
//! - regras personalizadas: arquivos YAML em `regras.personalizadas`, cada um com uma
//!   consulta do tree-sitter (exata e determinística; predicados `#eq?` e `#match?`).

use super::Contexto;
use crate::arquitetura::Padrao;
use crate::config::Config;
use crate::lang::{self, Linguagem, linha, simbolo_dono, texto, visitar};
use crate::modelo::{self, Severidade, Violacao};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Query, QueryCursor};

struct ImportProibido {
    padrao: String,
}

impl ImportProibido {
    /// `a.b.C` exato; `a.b.*` só o pacote; `a.b.**` o pacote e os de dentro.
    fn casa(&self, pacote: &str, nome: &str) -> bool {
        let completo = if nome == "*" {
            pacote.to_string()
        } else {
            format!("{pacote}.{nome}")
        };
        if let Some(p) = self.padrao.strip_suffix(".**") {
            return completo == p || completo.starts_with(&format!("{p}."));
        }
        if let Some(p) = self.padrao.strip_suffix(".*") {
            return pacote == p;
        }
        completo == self.padrao || (nome == "*" && pacote == self.padrao)
    }
}

struct TipoProibido {
    tipo: String,
    pacote: Padrao,
    padrao_texto: String,
    dica: String,
}

pub struct RegraPersonalizada {
    pub codigo: &'static str,
    ling: Linguagem,
    consulta: Query,
    alvo: Option<u32>,
    mensagem: String,
    dica: String,
    severidade: Severidade,
    pacotes: Vec<Padrao>,
}

#[derive(Default)]
pub struct Extras {
    imports: Vec<ImportProibido>,
    tipos: Vec<TipoProibido>,
    pub personalizadas: Vec<RegraPersonalizada>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct ArquivoDeRegra {
    codigo: String,
    linguagem: String,
    mensagem: String,
    #[serde(default)]
    dica: String,
    #[serde(default)]
    severidade: Option<Severidade>,
    #[serde(default)]
    pacotes: Vec<String>,
    consulta: String,
}

impl Extras {
    pub fn carregar(raiz: &Path, cfg: &Config) -> Result<Self> {
        let p = &cfg.regras.proibir;
        let imports = p
            .imports
            .iter()
            .map(|s| ImportProibido {
                padrao: s.trim().to_string(),
            })
            .collect();
        let tipos = p
            .tipos_em_pacote
            .iter()
            .map(|t| {
                Ok(TipoProibido {
                    tipo: t.tipo.clone(),
                    pacote: Padrao::novo(&t.pacote)
                        .with_context(|| format!("regras.proibir: pacote {:?}", t.pacote))?,
                    padrao_texto: t.pacote.clone(),
                    dica: t
                        .dica
                        .clone()
                        .unwrap_or_else(|| format!("não use {} aqui", t.tipo)),
                })
            })
            .collect::<Result<_>>()?;
        let personalizadas = match &cfg.regras.personalizadas {
            Some(dir) => carregar_personalizadas(&raiz.join(dir))?,
            None => Vec::new(),
        };
        Ok(Self {
            imports,
            tipos,
            personalizadas,
        })
    }

    /// Conteúdo das regras personalizadas, para a chave do cache.
    pub fn impressao(raiz: &Path, cfg: &Config) -> String {
        let Some(dir) = &cfg.regras.personalizadas else {
            return String::new();
        };
        let mut arquivos: Vec<_> = std::fs::read_dir(raiz.join(dir))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect();
        arquivos.sort();
        let mut h = blake3::Hasher::new();
        for a in arquivos {
            h.update(a.to_string_lossy().as_bytes());
            h.update(&std::fs::read(&a).unwrap_or_default());
        }
        h.finalize().to_hex()[..16].to_string()
    }
}

fn codigo_valido(c: &str) -> bool {
    let letras = c.chars().take_while(|x| x.is_ascii_uppercase()).count();
    letras >= 2 && c.len() == letras + 3 && c[letras..].chars().all(|x| x.is_ascii_digit())
}

fn carregar_personalizadas(dir: &Path) -> Result<Vec<RegraPersonalizada>> {
    let mut arquivos: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("regras.personalizadas: não achei {}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("yml" | "yaml")))
        .collect();
    arquivos.sort();
    let mut vistos = BTreeSet::new();
    let mut v = Vec::new();
    for a in arquivos {
        let nome = a
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().to_string());
        let t = std::fs::read_to_string(&a)?;
        let r: ArquivoDeRegra = serde_yaml::from_str(&t).map_err(|e| {
            let l = e.location().map(|l| l.line()).unwrap_or(0);
            anyhow::anyhow!("{nome}:{l} {e}")
        })?;
        if !codigo_valido(&r.codigo) {
            bail!(
                "{nome}: código {:?} inválido (use letras maiúsculas e 3 dígitos, ex.: USR001)",
                r.codigo
            );
        }
        if modelo::regra(&r.codigo).is_some() {
            bail!(
                "{nome}: {} já é uma regra da guarda; escolha outro código",
                r.codigo
            );
        }
        if !vistos.insert(r.codigo.clone()) {
            bail!(
                "{nome}: código {} repetido em outra regra personalizada",
                r.codigo
            );
        }
        let ling = match r.linguagem.as_str() {
            "java" => Linguagem::Java,
            "kotlin" => Linguagem::Kotlin,
            outra => bail!("{nome}: linguagem {outra:?} (use java ou kotlin)"),
        };
        let consulta = Query::new(&ling.gramatica(), &r.consulta)
            .map_err(|e| anyhow::anyhow!("{nome}: consulta inválida: {e}"))?;
        let alvo = consulta.capture_index_for_name("alvo");
        let pacotes = r
            .pacotes
            .iter()
            .map(|p| Padrao::novo(p).with_context(|| format!("{nome}: pacote {p:?}")))
            .collect::<Result<_>>()?;
        v.push(RegraPersonalizada {
            codigo: modelo::interno(&r.codigo),
            ling,
            consulta,
            alvo,
            mensagem: r.mensagem,
            dica: r.dica,
            severidade: r.severidade.unwrap_or(Severidade::Aviso),
            pacotes,
        });
    }
    Ok(v)
}

pub fn verificar(ctx: &Contexto, v: &mut Vec<Violacao>) {
    let Some(extras) = ctx.extras else { return };
    // PRB001: imports proibidos.
    if !extras.imports.is_empty() {
        for imp in lang::imports(ctx.raiz, ctx.fonte) {
            if let Some(p) = extras
                .imports
                .iter()
                .find(|p| p.casa(&imp.pacote, &imp.nome))
            {
                v.push(Violacao::nova(
                    "PRB001",
                    ctx.arquivo,
                    imp.linha,
                    format!("import proibido ({})", p.padrao),
                    "o guarda.yml proíbe este import (regras.proibir.imports)",
                    "",
                    &imp.texto,
                    None,
                ));
            }
        }
    }
    // PRB002: tipos proibidos nos pacotes.
    if let Some(pacote) = ctx.pacote {
        let aqui: Vec<&TipoProibido> = extras
            .tipos
            .iter()
            .filter(|t| t.pacote.casa_sem_projeto(pacote))
            .collect();
        if !aqui.is_empty() {
            let mut linhas = BTreeSet::new();
            visitar(ctx.raiz, |n| {
                let k = n.kind();
                let candidato = match k {
                    "type_identifier"
                    | "integral_type"
                    | "floating_point_type"
                    | "boolean_type" => texto(n, ctx.fonte),
                    "user_type" => {
                        let t = texto(n, ctx.fonte);
                        t.split('<').next().unwrap_or(t).trim()
                    }
                    _ => return,
                };
                if let Some(t) = aqui.iter().find(|t| t.tipo == candidato)
                    && linhas.insert((linha(n), t.tipo.clone()))
                {
                    v.push(Violacao::nova(
                        "PRB002",
                        ctx.arquivo,
                        linha(n),
                        format!("{} proibido em {}", t.tipo, t.padrao_texto),
                        t.dica.clone(),
                        simbolo_dono(n, ctx.fonte),
                        &t.tipo,
                        None,
                    ));
                }
            });
        }
    }
    // Regras personalizadas.
    for r in extras.personalizadas.iter().filter(|r| r.ling == ctx.ling) {
        if !r.pacotes.is_empty()
            && !ctx
                .pacote
                .is_some_and(|p| r.pacotes.iter().any(|x| x.casa_sem_projeto(p)))
        {
            continue;
        }
        let mut cursor = QueryCursor::new();
        let mut achados = cursor.matches(&r.consulta, ctx.raiz, ctx.fonte.as_bytes());
        let mut linhas = BTreeSet::new();
        while let Some(m) = achados.next() {
            let no = match r.alvo {
                Some(i) => m.captures().iter().find(|c| c.index == i).map(|c| c.node),
                None => m.captures().first().map(|c| c.node),
            };
            let Some(no) = no else { continue };
            if !linhas.insert(linha(no)) {
                continue;
            }
            let mut viol = Violacao::nova(
                r.codigo,
                ctx.arquivo,
                linha(no),
                r.mensagem.clone(),
                r.dica.clone(),
                simbolo_dono(no, ctx.fonte),
                texto(no, ctx.fonte),
                None,
            );
            viol.severidade = r.severidade;
            v.push(viol);
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::config::Limites;
    use crate::lang::Parsers;

    fn rodar(
        fonte: &str,
        ling: Linguagem,
        pacote: Option<&str>,
        extras: &Extras,
    ) -> Vec<(String, usize, String)> {
        let mut p = Parsers::novo();
        let t = p.ler(ling, fonte).unwrap();
        let lim = Limites::default();
        let ctx = Contexto {
            arquivo: "A",
            fonte,
            ling,
            raiz: t.root_node(),
            limites: &lim,
            pacote,
            extras: Some(extras),
        };
        let mut v = Vec::new();
        verificar(&ctx, &mut v);
        v.into_iter()
            .map(|x| (x.codigo.to_string(), x.linha, x.mensagem))
            .collect()
    }

    #[test]
    fn proibir_imports_e_tipos() {
        let cfg = Config::de_texto(
            "regras:\n  proibir:\n    imports: [java.util.Date, \"org.joda.**\"]\n    tipos-em-pacote:\n      - tipo: Double\n        pacote: \"..calculo..\"\n        dica: use BigDecimal\n",
        )
        .unwrap();
        let e = Extras::carregar(Path::new("."), &cfg).unwrap();
        let j = "package com.app.calculo;\nimport java.util.Date;\nimport org.joda.time.DateTime;\nimport java.util.List;\nclass C { Double total(Double a) { return a; } }";
        let v = rodar(j, Linguagem::Java, Some("com.app.calculo"), &e);
        assert_eq!(
            v,
            [
                (
                    "PRB001".into(),
                    2,
                    "import proibido (java.util.Date)".into()
                ),
                ("PRB001".into(), 3, "import proibido (org.joda.**)".into()),
                ("PRB002".into(), 5, "Double proibido em ..calculo..".into()),
            ]
        );
        assert!(
            rodar(j, Linguagem::Java, Some("com.app.web"), &e)
                .iter()
                .all(|x| x.0 == "PRB001")
        );
    }

    #[test]
    fn regra_personalizada_por_consulta() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("sem-stacktrace.yml"),
            "codigo: USR001\nlinguagem: java\nmensagem: chamada a printStackTrace\ndica: use o logger\nseveridade: erro\nconsulta: |\n  (method_invocation name: (identifier) @n (#eq? @n \"printStackTrace\")) @alvo\n",
        )
        .unwrap();
        let cfg = Config::de_texto(&format!(
            "regras:\n  personalizadas: {}\n",
            dir.path().to_string_lossy().replace('\\', "/")
        ))
        .unwrap();
        let e = Extras::carregar(Path::new("."), &cfg).unwrap();
        let j = "class A { void f() { try { } catch (E e) { e.printStackTrace(); } g(); } }";
        assert_eq!(
            rodar(j, Linguagem::Java, None, &e),
            [("USR001".into(), 1, "chamada a printStackTrace".into())]
        );
    }

    #[test]
    fn regra_personalizada_invalida_explica() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("x.yml"),
            "codigo: KT001\nlinguagem: java\nmensagem: x\nconsulta: \"(identifier) @a\"\n",
        )
        .unwrap();
        let cfg = Config::de_texto(&format!(
            "regras:\n  personalizadas: {}\n",
            dir.path().to_string_lossy().replace('\\', "/")
        ))
        .unwrap();
        let e = Extras::carregar(Path::new("."), &cfg)
            .err()
            .unwrap()
            .to_string();
        assert!(e.contains("já é uma regra da guarda"), "{e}");
    }
}
