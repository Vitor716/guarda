//! Motor: lê cada arquivo uma vez (ou tira do cache), roda as regras por arquivo em paralelo,
//! monta o índice do projeto e roda as regras de projeto (arquitetura). Resultado ordenado.

use crate::arquitetura::{self, Arquitetura, ArquivoIndice, Pontuacao, Projeto};
use crate::cache::{self, Cache, Dados, Entrada, ViolacaoSalva};
use crate::config::Config;
use crate::descoberta;
use crate::duplicacao::{self, Token};
use crate::lang::{self, Import, Linguagem, Parsers};
use crate::modelo::Violacao;
use crate::recursos;
use crate::regras::extras::Extras;
use crate::regras::{self, Contexto, Supressao};
use anyhow::Result;
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet, HashSet};
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
    /// Arquivo de teste: só as regras TST rodam nele e ele fica fora do índice.
    pub teste: bool,
    /// Tokens para a detecção de duplicação.
    pub tokens: Vec<Token>,
    /// Anotações das classes (nome, linha), para as regras de arquitetura.
    pub anotacoes: Vec<(String, usize)>,
    /// Exceções instanciadas, para as regras de negócio (NEG001/002).
    pub excecoes: Vec<lang::ExcecaoCriada>,
    /// Classes @Entity e parâmetros @RequestBody, para a SPR004.
    pub entidades: Vec<String>,
    pub corpos: Vec<lang::CorpoRequisicao>,
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
            teste: false,
            tokens: Vec::new(),
            anotacoes: Vec::new(),
            excecoes: Vec::new(),
            entidades: Vec::new(),
            corpos: Vec::new(),
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
        a.tokens = d.tokens.clone();
        a.anotacoes = d.anotacoes.clone();
        a.excecoes = d.excecoes.clone();
        a.entidades = d.entidades.clone();
        a.corpos = d.corpos.clone();
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
            tokens: self.tokens.clone(),
            anotacoes: self.anotacoes.clone(),
            excecoes: self.excecoes.clone(),
            entidades: self.entidades.clone(),
            corpos: self.corpos.clone(),
        }
    }
}

/// Roda as regras por arquivo sobre o conteúdo já lido.
pub fn analisar_fonte(
    parsers: &mut Parsers,
    rel: &str,
    bruto: &[u8],
    cfg: &Config,
    teste: bool,
    extras: &Extras,
) -> Analise {
    let inicio = Instant::now();
    let mut a = Analise::vazia(rel);
    a.teste = teste;
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
    a.anotacoes = lang::anotacoes_de_tipo(raiz_no, fonte);
    a.excecoes = lang::excecoes_criadas(raiz_no, fonte);
    (a.entidades, a.corpos) = lang::entidades_e_corpos(raiz_no, fonte);

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
            pacote: a.pacote.as_deref(),
            extras: Some(extras),
        };
        let (v, s) = if teste {
            regras::de_teste(&ctx)
        } else {
            regras::por_arquivo(&ctx)
        };
        a.violacoes = v;
        a.supressoes = s;
        if !teste {
            a.tokens = duplicacao::tokens(raiz_no, fonte);
        }
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
    teste: bool,
    extras: &Extras,
) -> (Analise, Option<Entrada>) {
    let (mut a, nova) = obter_sem_marca(parsers, raiz, rel, cfg, c, teste, extras);
    a.teste = teste;
    (a, nova)
}

fn obter_sem_marca(
    parsers: &mut Parsers,
    raiz: &Path,
    rel: &str,
    cfg: &Config,
    c: &Cache,
    teste: bool,
    extras: &Extras,
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
    let a = analisar_fonte(parsers, rel, &bruto, cfg, teste, extras);
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
    /// Linhas duplicadas e linhas de código de cada arquivo do escopo (com `duplicacao`).
    pub duplicadas: BTreeMap<String, BTreeSet<usize>>,
    pub linhas_codigo: BTreeMap<String, BTreeSet<usize>>,
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
    let (todos, testes) = descoberta::listar_com_testes(raiz, &cfg.fontes)?;
    let quero: Option<HashSet<&str>> = match escopo {
        Escopo::Tudo => None,
        Escopo::Arquivos(lista) => Some(lista.iter().map(String::as_str).collect()),
    };
    let no_escopo = |r: &&str| quero.as_ref().is_none_or(|q| q.contains(r));
    let alvo: HashSet<&str> = todos.iter().map(String::as_str).filter(no_escopo).collect();
    let testes_alvo: Vec<&str> = testes
        .iter()
        .map(String::as_str)
        .filter(no_escopo)
        .collect();

    let extras = Extras::carregar(raiz, cfg)?;
    // O projeto inteiro passa pelo cache: é o índice que mantém a arquitetura valendo
    // mesmo quando só um arquivo é verificado.
    let mut c = if usar_cache {
        Cache::carregar(
            raiz,
            &format!(
                "{}|{}",
                cfg.impressao_regras(),
                Extras::impressao(raiz, cfg)
            ),
        )
    } else {
        Cache::desligado()
    };
    let resultados: Vec<(Analise, Option<Entrada>)> = todos
        .par_iter()
        .map_init(Parsers::novo, |p, rel| {
            obter(p, raiz, rel, cfg, &c, false, &extras)
        })
        .collect();
    let de_teste: Vec<(Analise, Option<Entrada>)> = testes_alvo
        .par_iter()
        .map_init(Parsers::novo, |p, rel| {
            obter(p, raiz, rel, cfg, &c, true, &extras)
        })
        .collect();
    let mut todas = Vec::with_capacity(resultados.len());
    for (a, nova) in resultados {
        if let Some(e) = nova {
            c.guardar(a.arquivo.clone(), e);
        }
        todas.push(a);
    }
    let mut analises_teste = Vec::with_capacity(de_teste.len());
    for (a, nova) in de_teste {
        if let Some(e) = nova {
            c.guardar(a.arquivo.clone(), e);
        }
        analises_teste.push(a);
    }
    c.manter_so(&todos.iter().chain(&testes).map(String::as_str).collect());
    c.salvar();

    let indice: Vec<ArquivoIndice> = todas
        .iter()
        .map(|a| ArquivoIndice {
            arquivo: &a.arquivo,
            pacote: a.pacote.as_deref(),
            imports: &a.imports,
            anotacoes: &a.anotacoes,
            excecoes: &a.excecoes,
        })
        .collect();
    let pacotes: HashSet<String> = indice
        .iter()
        .filter_map(|a| a.pacote.map(str::to_string))
        .collect();
    let projeto = Projeto::novo(cfg.arquitetura.base.as_deref(), pacotes);

    let ranking = arquitetura::pontuar(&projeto, &indice);
    let (arq, origem) = match arquitetura::da_config(&cfg.arquitetura, &projeto)? {
        Some(a) => (Some(a), Origem::Config),
        None => match ranking.first() {
            Some(p) if p.nota >= arquitetura::CONFIANCA_MINIMA => (
                arquitetura::preset_do_projeto(&p.preset, &projeto),
                Origem::Detectada,
            ),
            _ => (None, Origem::Indefinida),
        },
    };

    let (sml002, duplicadas, linhas_codigo) = match &cfg.duplicacao {
        Some(d) => duplicacoes(&todas, &alvo, d.tokens_minimos),
        None => Default::default(),
    };
    // ARQ002 olha o grafo do projeto inteiro e reporta nos arquivos do escopo.
    let arq002 = if cfg.arquitetura.ciclos.as_deref() == Some("permitir") {
        Vec::new()
    } else {
        arquitetura::ciclos(&projeto, &indice, &alvo)
    };

    let spr004 = entidades_no_corpo(&todas, &alvo);
    let recursos = recursos::verificar(
        raiz,
        &descoberta::recursos(raiz, &cfg.fontes)?,
        quero.as_ref(),
    );

    let mut analises: Vec<Analise> = todas
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
                anotacoes: &a.anotacoes,
                excecoes: &a.excecoes,
            })
            .collect();
        violacoes.extend(arquitetura::verificar(arq, &projeto, &alvos));
        violacoes.extend(arquitetura::framework(
            arq,
            &projeto,
            &alvos,
            &cfg.arquitetura,
        ));
        violacoes.extend(arquitetura::controladores(arq, &projeto, &alvos));
        violacoes.extend(arquitetura::negocio(
            arq,
            &projeto,
            &alvos,
            &cfg.arquitetura,
        ));
    }
    violacoes.extend(sml002);
    violacoes.extend(spr004);
    violacoes.extend(recursos);
    violacoes.extend(arq002);
    // Arquivos de teste: só as regras TST, fora do índice e da duplicação.
    for a in analises_teste {
        violacoes.extend(a.violacoes.iter().cloned());
        analises.push(a);
    }

    // Supressões com motivo, regras desligadas e severidade do guarda.yml.
    let supressoes: BTreeMap<&str, &Vec<Supressao>> = analises
        .iter()
        .map(|a| (a.arquivo.as_str(), &a.supressoes))
        .collect();
    violacoes.retain(|v| {
        !cfg.desligada(v.codigo)
            && !supressoes
                .get(v.arquivo.as_str())
                .is_some_and(|ss| ss.iter().any(|s| s.cobre(v.codigo, v.linha)))
    });
    for v in &mut violacoes {
        v.severidade = cfg.severidade(v.codigo, v.severidade);
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
        duplicadas,
        linhas_codigo,
    })
}

type Duplicacoes = (
    Vec<Violacao>,
    BTreeMap<String, BTreeSet<usize>>,
    BTreeMap<String, BTreeSet<usize>>,
);

/// SML002 nos arquivos do escopo, procurando cópias no projeto inteiro.
fn duplicacoes(todas: &[Analise], alvo: &HashSet<&str>, minimo: usize) -> Duplicacoes {
    let validas: Vec<&Analise> = todas.iter().filter(|a| !a.erro_parse).collect();
    let tokens: Vec<&[Token]> = validas.iter().map(|a| a.tokens.as_slice()).collect();
    let blocos = duplicacao::detectar(&tokens, minimo);
    let mut violacoes = Vec::new();
    let mut duplicadas = BTreeMap::new();
    let mut linhas_codigo = BTreeMap::new();
    for (a, bs) in validas.iter().zip(&blocos) {
        if !alvo.contains(a.arquivo.as_str()) {
            continue;
        }
        linhas_codigo.insert(a.arquivo.clone(), duplicacao::linhas_de_codigo(&a.tokens));
        duplicadas.insert(a.arquivo.clone(), duplicacao::linhas_duplicadas(bs));
        for b in bs {
            let outro = &validas[b.par.0].arquivo;
            let nome = outro.rsplit('/').next().unwrap_or(outro);
            let local = if outro == &a.arquivo {
                format!("l.{}", b.par.1)
            } else {
                format!("{nome}:{}", b.par.1)
            };
            violacoes.push(Violacao::nova(
                "SML002",
                &a.arquivo,
                b.linha_ini,
                format!(
                    "{} linhas duplicadas (também em {local})",
                    b.linha_fim - b.linha_ini + 1
                ),
                "extraia para uma função ou classe compartilhada",
                nome_do_arquivo(&a.arquivo),
                &format!("{:016x}", b.digital),
                None,
            ));
        }
    }
    (violacoes, duplicadas, linhas_codigo)
}

fn nome_do_arquivo(rel: &str) -> &str {
    let nome = rel.rsplit('/').next().unwrap_or(rel);
    nome.split('.').next().unwrap_or(nome)
}

/// SPR004: entidade JPA (de qualquer arquivo do projeto) recebida direto no @RequestBody.
fn entidades_no_corpo(todas: &[Analise], alvo: &HashSet<&str>) -> Vec<Violacao> {
    let entidades: HashSet<&str> = todas
        .iter()
        .flat_map(|a| a.entidades.iter().map(String::as_str))
        .collect();
    let mut v = Vec::new();
    if entidades.is_empty() {
        return v;
    }
    for a in todas.iter().filter(|a| alvo.contains(a.arquivo.as_str())) {
        for c in &a.corpos {
            if let Some(e) = c.tipos.iter().find(|t| entidades.contains(t.as_str())) {
                v.push(Violacao::nova(
                    "SPR004",
                    &a.arquivo,
                    c.linha,
                    format!("entidade JPA {e} recebida direto no @RequestBody"),
                    "receba um DTO e copie só os campos permitidos (atribuição em massa)",
                    c.simbolo.clone(),
                    e,
                    None,
                ));
            }
        }
    }
    v
}
