//! Motor: lê cada arquivo uma vez (ou tira do cache), roda as regras por arquivo em paralelo,
//! monta o índice do projeto e roda as regras de projeto (arquitetura). Resultado ordenado.

use crate::arquitetura::{self, Arquitetura, ArquivoIndice, Pontuacao, Projeto};
use crate::cache::{self, Cache, Dados, Entrada, ViolacaoSalva};
use crate::config::Config;
use crate::descoberta;
use crate::lang::{self, Import, Linguagem, Parsers};
use crate::modelo::Violacao;
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
    /// Veio do cache, sem passar pelo parser.
    pub do_cache: bool,
}

impl Analise {
    fn vazia(rel: &str) -> Self {
        Self {
            arquivo: rel.to_string(),
            ling: Linguagem::do_caminho(rel).expect("só .java e .kt chegam aqui"),
            pacote: None,
            imports: Vec::new(),
            violacoes: Vec::new(),
            supressoes: Vec::new(),
            erro_parse: false,
            linhas: 0,
            tempo: Duration::ZERO,
            do_cache: false,
        }
    }

    fn de_cache(rel: &str, d: &Dados) -> Self {
        let mut a = Self::vazia(rel);
        a.pacote = d.pacote.clone();
        a.imports = d.imports.clone();
        a.violacoes = d.violacoes.iter().filter_map(|v| v.para(rel)).collect();
        a.supressoes = d.supressoes.clone();
        a.erro_parse = d.erro_parse;
        a.linhas = d.linhas;
        a.do_cache = true;
        a
    }

    fn dados(&self) -> Dados {
        Dados {
            pacote: self.pacote.clone(),
            imports: self.imports.clone(),
            violacoes: self.violacoes.iter().map(ViolacaoSalva::de).collect(),
            supressoes: self.supressoes.clone(),
            erro_parse: self.erro_parse,
            linhas: self.linhas,
        }
    }
}

/// Roda as regras por arquivo sobre o conteúdo já lido.
pub fn analisar_fonte(parsers: &mut Parsers, rel: &str, bruto: &[u8], cfg: &Config) -> Analise {
    let inicio = Instant::now();
    let mut a = Analise::vazia(rel);
    let fonte = String::from_utf8_lossy(bruto);
    let fonte = fonte.strip_prefix('\u{feff}').unwrap_or(&fonte);
    a.linhas = fonte.lines().count();
    let Some(arvore) = parsers.ler(a.ling, fonte) else {
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
            ling: a.ling,
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

/// Resultado de um arquivo: do cache se o conteúdo não mudou, senão do parser.
/// Devolve também a entrada nova do cache, quando houver.
fn obter(
    parsers: &mut Parsers,
    raiz: &Path,
    rel: &str,
    cfg: &Config,
    c: &Cache,
) -> (Analise, Option<Entrada>) {
    let caminho = raiz.join(rel);
    let assinatura = cache::assinatura(&caminho);
    let anterior = c.get(rel);
    if let (Some(e), Some((t, m))) = (anterior, assinatura)
        && e.tamanho == t
        && e.modificado_ns == m
    {
        return (Analise::de_cache(rel, &e.dados), None);
    }
    let bruto = match std::fs::read(&caminho) {
        Ok(b) => b,
        Err(e) => {
            let mut a = Analise::vazia(rel);
            a.erro_parse = true;
            a.violacoes
                .push(err000(rel, 1, &format!("não consegui ler o arquivo: {e}")));
            return (a, None);
        }
    };
    let (tamanho, modificado_ns) = assinatura.unwrap_or((bruto.len() as u64, 0));
    let hash = cache::hash(&bruto);
    if let Some(e) = anterior.filter(|e| e.hash == hash) {
        // Só a data mudou (checkout, touch): mesmo resultado.
        let nova = Entrada {
            tamanho,
            modificado_ns,
            hash,
            dados: e.dados.clone(),
        };
        return (Analise::de_cache(rel, &e.dados), Some(nova));
    }
    let a = analisar_fonte(parsers, rel, &bruto, cfg);
    let nova = c.ligado().then(|| Entrada {
        tamanho,
        modificado_ns,
        hash,
        dados: a.dados(),
    });
    (a, nova)
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
    /// Só os arquivos do escopo pedido.
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
    executar_com(raiz, cfg, escopo, true)
}

pub fn executar_com(
    raiz: &Path,
    cfg: &Config,
    escopo: &Escopo,
    usar_cache: bool,
) -> Result<Relatorio> {
    let inicio = Instant::now();
    let todos = descoberta::listar(raiz, &cfg.fontes)?;
    let alvo: HashSet<&str> = match escopo {
        Escopo::Tudo => todos.iter().map(String::as_str).collect(),
        Escopo::Arquivos(lista) => {
            let quero: HashSet<&str> = lista.iter().map(String::as_str).collect();
            todos
                .iter()
                .map(String::as_str)
                .filter(|r| quero.contains(r))
                .collect()
        }
    };

    // O projeto inteiro passa pelo cache: é o índice que mantém a arquitetura valendo
    // mesmo quando só um arquivo é verificado.
    let mut c = if usar_cache {
        Cache::carregar(raiz, &cfg.impressao_regras())
    } else {
        Cache::desligado()
    };
    let resultados: Vec<(Analise, Option<Entrada>)> = todos
        .par_iter()
        .map_init(Parsers::novo, |p, rel| obter(p, raiz, rel, cfg, &c))
        .collect();
    let mut todas = Vec::with_capacity(resultados.len());
    for (a, nova) in resultados {
        if let Some(e) = nova {
            c.guardar(a.arquivo.clone(), e);
        }
        todas.push(a);
    }
    c.manter_so(&todos.iter().map(String::as_str).collect());
    c.salvar();

    let indice: Vec<ArquivoIndice> = todas
        .iter()
        .map(|a| ArquivoIndice {
            arquivo: &a.arquivo,
            pacote: a.pacote.as_deref(),
            imports: &a.imports,
        })
        .collect();
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

    let analises: Vec<Analise> = todas
        .into_iter()
        .filter(|a| alvo.contains(a.arquivo.as_str()))
        .collect();
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
        if let Some(s) = cfg.regras.severidade.get(v.codigo) {
            v.severidade = *s;
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
