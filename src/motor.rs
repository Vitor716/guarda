//! Motor: lê cada arquivo uma vez, roda as regras por arquivo em paralelo, monta o índice
//! do projeto e roda as regras de projeto (arquitetura). Resultado sempre ordenado.

use crate::arquitetura::{self, Arquitetura, ArquivoIndice, Pontuacao, Projeto};
use crate::config::Config;
use crate::descoberta;
use crate::lang::{self, Import, Linguagem, Parsers};
use crate::modelo::{Severidade, Violacao};
use crate::regras::{self, Contexto, Supressao};
use anyhow::Result;
use rayon::prelude::*;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::time::{Duration, Instant};

pub struct Analise {
    pub arquivo: String,
    pub ling: Linguagem,
    pub pacote: Option<String>,
    pub imports: Vec<Import>,
    pub violacoes: Vec<Violacao>,
    pub supressoes: Vec<Supressao>,
    pub erro_parse: bool,
    pub linhas: usize,
    pub tempo: Duration,
}

pub fn analisar_arquivo(parsers: &mut Parsers, raiz: &Path, rel: &str, cfg: &Config) -> Analise {
    let inicio = Instant::now();
    let ling = Linguagem::do_caminho(rel).expect("só .java e .kt chegam aqui");
    let mut a = Analise {
        arquivo: rel.to_string(),
        ling,
        pacote: None,
        imports: Vec::new(),
        violacoes: Vec::new(),
        supressoes: Vec::new(),
        erro_parse: false,
        linhas: 0,
        tempo: Duration::ZERO,
    };
    let bruto = match std::fs::read(raiz.join(rel)) {
        Ok(b) => b,
        Err(e) => {
            a.erro_parse = true;
            a.violacoes
                .push(err000(rel, 1, &format!("não consegui ler o arquivo: {e}")));
            return a;
        }
    };
    let fonte = String::from_utf8_lossy(&bruto);
    let fonte = fonte.strip_prefix('\u{feff}').unwrap_or(&fonte);
    a.linhas = fonte.lines().count();
    let Some(arvore) = parsers.ler(ling, fonte) else {
        a.erro_parse = true;
        a.violacoes
            .push(err000(rel, 1, "o parser não devolveu árvore"));
        return a;
    };
    let raiz_no = arvore.root_node();
    a.pacote = lang::pacote(raiz_no, fonte);
    a.imports = lang::imports(raiz_no, fonte);

    if raiz_no.has_error() {
        // Arquivo com erro de sintaxe não é pulado em silêncio: vira ERR000 e as outras
        // regras não rodam nele (árvore parcial gera falso positivo).
        let mut primeira = None;
        let mut trechos = 0;
        lang::visitar(raiz_no, |n| {
            if n.is_error() || n.is_missing() {
                trechos += 1;
                primeira.get_or_insert(lang::linha(n));
            }
        });
        a.erro_parse = true;
        a.violacoes.push(err000(
            rel,
            primeira.unwrap_or(1),
            &format!("o parser não leu o arquivo inteiro ({trechos} trecho(s) com erro)"),
        ));
    } else {
        let ctx = Contexto {
            arquivo: rel,
            fonte,
            ling,
            raiz: raiz_no,
            limites: &cfg.limites,
        };
        let (v, s) = regras::por_arquivo(&ctx);
        a.violacoes = v;
        a.supressoes = s;
    }
    a.tempo = inicio.elapsed();
    a
}

fn err000(arquivo: &str, linha: usize, msg: &str) -> Violacao {
    Violacao::nova(
        "ERR000",
        arquivo,
        linha,
        msg,
        "se o código compila, é limite da gramática: registre o caso",
        "",
        "",
        None,
    )
}

pub enum Escopo {
    Tudo,
    Arquivos(Vec<String>),
}

/// Arquitetura efetivamente usada e como ela foi escolhida.
pub enum Origem {
    Config,
    Detectada,
    /// `auto` sem confiança suficiente: ARQ001 desligada.
    Indefinida,
}

pub struct Relatorio {
    pub violacoes: Vec<Violacao>,
    pub analises: Vec<Analise>,
    pub arquitetura: Option<Arquitetura>,
    pub origem: Origem,
    pub ranking: Vec<Pontuacao>,
    pub projeto: Projeto,
    pub tempo: Duration,
}

impl Relatorio {
    pub fn confianca(&self) -> Option<f64> {
        let arq = self.arquitetura.as_ref()?;
        self.ranking
            .iter()
            .find(|p| p.preset == arq.nome)
            .map(|p| p.nota)
    }
}

pub fn executar(raiz: &Path, cfg: &Config, escopo: &Escopo) -> Result<Relatorio> {
    let inicio = Instant::now();
    let todos = descoberta::listar(raiz, &cfg.fontes)?;
    let alvo: Vec<String> = match escopo {
        Escopo::Tudo => todos.clone(),
        Escopo::Arquivos(lista) => {
            let quero: HashSet<&String> = lista.iter().collect();
            todos
                .iter()
                .filter(|r| quero.contains(r))
                .cloned()
                .collect()
        }
    };

    let analises: Vec<Analise> = alvo
        .par_iter()
        .map_init(Parsers::novo, |p, rel| analisar_arquivo(p, raiz, rel, cfg))
        .collect();

    // Índice do projeto: no modo parcial, o resto do projeto entra pelo índice rápido.
    let rapido;
    let indice: Vec<ArquivoIndice> = match escopo {
        Escopo::Tudo => analises
            .iter()
            .map(|a| ArquivoIndice {
                arquivo: &a.arquivo,
                pacote: a.pacote.as_deref(),
                imports: &a.imports,
            })
            .collect(),
        Escopo::Arquivos(_) => {
            rapido = descoberta::indice_rapido(raiz, &todos);
            rapido
                .iter()
                .map(|(r, p, i)| ArquivoIndice {
                    arquivo: r,
                    pacote: p.as_deref(),
                    imports: i,
                })
                .collect()
        }
    };
    let pacotes: HashSet<String> = indice
        .iter()
        .filter_map(|a| a.pacote.map(str::to_string))
        .collect();
    let projeto = Projeto::novo(cfg.arquitetura.base.as_deref(), pacotes);

    let ranking = arquitetura::pontuar(&projeto, &indice);
    let (arq, origem) = match arquitetura::da_config(&cfg.arquitetura)? {
        Some(a) => (Some(a), Origem::Config),
        None => match ranking.first() {
            Some(p) if p.nota >= arquitetura::CONFIANCA_MINIMA => {
                (arquitetura::preset(&p.preset), Origem::Detectada)
            }
            _ => (None, Origem::Indefinida),
        },
    };

    let mut violacoes: Vec<Violacao> = Vec::new();
    for a in &analises {
        violacoes.extend(a.violacoes.iter().cloned());
    }
    if let Some(arq) = &arq {
        let alvos: Vec<ArquivoIndice> = analises
            .iter()
            .filter(|a| !a.erro_parse)
            .map(|a| ArquivoIndice {
                arquivo: &a.arquivo,
                pacote: a.pacote.as_deref(),
                imports: &a.imports,
            })
            .collect();
        violacoes.extend(arquitetura::verificar(arq, &projeto, &alvos));
    }

    // Supressões com motivo, regras desligadas e severidade do guarda.yml.
    let supressoes: BTreeMap<&str, &Vec<Supressao>> = analises
        .iter()
        .map(|a| (a.arquivo.as_str(), &a.supressoes))
        .collect();
    let desligadas: HashSet<&str> = cfg.regras.desligar.iter().map(String::as_str).collect();
    violacoes.retain(|v| {
        !desligadas.contains(v.codigo)
            && !supressoes
                .get(v.arquivo.as_str())
                .is_some_and(|ss| ss.iter().any(|s| s.cobre(v.codigo, v.linha)))
    });
    for v in &mut violacoes {
        if let Some(s) = cfg
            .regras
            .severidade
            .get(v.codigo)
            .and_then(|s| Severidade::de_texto(s))
        {
            v.severidade = s;
        }
    }
    violacoes.sort_by(|a, b| a.chave_ordem().cmp(&b.chave_ordem()));

    Ok(Relatorio {
        violacoes,
        analises,
        arquitetura: arq,
        origem,
        ranking,
        projeto,
        tempo: inicio.elapsed(),
    })
}
