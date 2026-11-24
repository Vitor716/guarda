//! Validação de arquitetura: todo preset vira a mesma estrutura interna
//! (camadas → padrões de pacote + arestas permitidas). ARQ001 usa só os imports.

use crate::config;
use crate::lang::Import;
use crate::modelo::Violacao;
use anyhow::{Result, bail};
use regex::Regex;
use std::collections::{BTreeSet, HashSet};

/// Padrão de pacote na sintaxe do ArchUnit: `..domain..`, `com.app..`, `..adapter.in..`, `*`.
#[derive(Debug, Clone)]
pub struct Padrao {
    re: Regex,
    /// Começa com `..`: casa só na parte do pacote abaixo do pacote base.
    relativo: bool,
}

impl Padrao {
    pub fn novo(p: &str) -> Result<Self> {
        let p = p.trim();
        if p.is_empty() {
            bail!("padrão de pacote vazio");
        }
        let relativo = p.starts_with("..");
        let mut s = p;
        let mut re = String::from("^");
        if let Some(resto) = s.strip_prefix("..") {
            // Preguiçoso: a primeira ocorrência (a mais rasa) é a que vale.
            re.push_str(r"(?:[^.]+\.)*?");
            s = resto;
        }
        let aberto_no_fim = s.ends_with("..");
        if aberto_no_fim {
            s = &s[..s.len() - 2];
        }
        re.push_str("(?P<n>");
        for (i, parte) in s.split("..").enumerate() {
            if i > 0 {
                re.push_str(r"\.(?:[^.]+\.)*?");
            }
            for ch in parte.chars() {
                match ch {
                    '*' => re.push_str("[^.]*"),
                    c => re.push_str(&regex::escape(&c.to_string())),
                }
            }
        }
        re.push(')');
        if aberto_no_fim {
            re.push_str(r"(?:\..*)?");
        }
        re.push('$');
        Ok(Self {
            re: Regex::new(&re)?,
            relativo,
        })
    }

    /// Profundidade (em segmentos) onde o padrão casou; `None` se não casou.
    #[cfg(test)]
    fn casar(&self, completo: &str, relativo: &str, profundidade_base: usize) -> Option<usize> {
        self.casar_com_tamanho(completo, relativo, profundidade_base)
            .map(|(d, _)| d)
    }

    /// O pacote casa com o padrão (relativo ao pacote base do projeto, se começar com `..`)?
    pub fn casa_pacote(&self, pacote: &str, projeto: &Projeto) -> bool {
        self.casar_com_tamanho(
            pacote,
            projeto.relativo(pacote),
            projeto.profundidade_base(),
        )
        .is_some()
    }

    /// Casa o pacote sem conhecer a base do projeto (regras por arquivo, em cache).
    pub fn casa_sem_projeto(&self, pacote: &str) -> bool {
        self.casar_com_tamanho(pacote, pacote, 0).is_some()
    }

    /// Profundidade onde o padrão casou e quantos segmentos o trecho casado ocupa.
    fn casar_com_tamanho(
        &self,
        completo: &str,
        relativo: &str,
        profundidade_base: usize,
    ) -> Option<(usize, usize)> {
        let (alvo, desloc) = if self.relativo {
            (relativo, profundidade_base)
        } else {
            (completo, 0)
        };
        if alvo.is_empty() {
            return None;
        }
        let cap = self.re.captures(alvo)?;
        let n = cap.name("n")?;
        Some((
            desloc + alvo[..n.start()].matches('.').count(),
            n.as_str().matches('.').count() + 1,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct Camada {
    pub nome: String,
    pub padroes: Vec<Padrao>,
}

#[derive(Debug, Clone)]
pub struct Arquitetura {
    pub nome: String,
    pub camadas: Vec<Camada>,
    pub permitidas: HashSet<(String, String)>,
    /// Imports de fora do projeto proibidos por camada (ARQ003), além de `sem-framework`.
    pub externos: Vec<(String, Vec<String>)>,
}

/// O que se sabe do projeto inteiro: pacote base e pacotes declarados.
#[derive(Debug, Default, Clone)]
pub struct Projeto {
    pub base: Option<String>,
    pub pacotes: HashSet<String>,
}

impl Projeto {
    pub fn novo(base_config: Option<&str>, pacotes: HashSet<String>) -> Self {
        let base = base_config
            .map(str::to_string)
            .or_else(|| prefixo_comum(&pacotes));
        Self { base, pacotes }
    }

    /// O pacote é do próprio projeto (e não de biblioteca)?
    pub fn interno(&self, pacote: &str) -> bool {
        if self.pacotes.contains(pacote) {
            return true;
        }
        match &self.base {
            Some(b) if !b.is_empty() => {
                pacote == b
                    || (pacote.starts_with(b.as_str()) && pacote[b.len()..].starts_with('.'))
            }
            _ => false,
        }
    }

    fn relativo<'a>(&self, pacote: &'a str) -> &'a str {
        match &self.base {
            Some(b) if !b.is_empty() && pacote.starts_with(b.as_str()) => {
                pacote[b.len()..].trim_start_matches('.')
            }
            _ => pacote,
        }
    }

    fn profundidade_base(&self) -> usize {
        self.base
            .as_ref()
            .filter(|b| !b.is_empty())
            .map_or(0, |b| b.matches('.').count() + 1)
    }
}

pub fn prefixo_comum(pacotes: &HashSet<String>) -> Option<String> {
    let mut iter = pacotes.iter();
    let primeiro: Vec<&str> = iter.next()?.split('.').collect();
    let mut n = primeiro.len();
    for p in iter {
        let segs: Vec<&str> = p.split('.').collect();
        n = n.min(segs.len());
        n = (0..n).take_while(|&i| segs[i] == primeiro[i]).count();
    }
    if n == 0 {
        None
    } else {
        Some(primeiro[..n].join("."))
    }
}

impl Arquitetura {
    /// Camada do pacote: a do padrão que casa mais perto da raiz; empate → ordem de declaração.
    pub fn camada_de(&self, pacote: &str, projeto: &Projeto) -> Option<&str> {
        self.camada_com_pasta(pacote, projeto).map(|(c, _)| c)
    }

    /// Camada do pacote e o prefixo do pacote que decidiu a camada (`...domain`,
    /// `...adapter.in`): é a "pasta" da camada, usada no `guarda contexto`.
    pub fn camada_com_pasta(&self, pacote: &str, projeto: &Projeto) -> Option<(&str, String)> {
        let rel = projeto.relativo(pacote);
        let base = projeto.profundidade_base();
        let mut melhor: Option<(usize, usize, &str)> = None;
        for c in &self.camadas {
            for p in &c.padroes {
                if let Some((d, n)) = p.casar_com_tamanho(pacote, rel, base)
                    && melhor.is_none_or(|(md, _, _)| d < md)
                {
                    melhor = Some((d, n, &c.nome));
                }
            }
        }
        let (d, n, nome) = melhor?;
        let pasta: Vec<&str> = pacote.split('.').take(d + n).collect();
        Some((nome, pasta.join(".")))
    }

    pub fn permitido(&self, de: &str, para: &str) -> bool {
        de == para
            || self
                .permitidas
                .contains(&(de.to_string(), para.to_string()))
    }

    fn dica(&self, de: &str, para: &str) -> String {
        let meio =
            self.camadas.iter().map(|c| c.nome.as_str()).find(|m| {
                *m != de && *m != para && self.permitido(de, m) && self.permitido(m, para)
            });
        match meio {
            Some(m) => format!("passe pela camada {m}"),
            None => format!("dependa de uma interface (porta) em {de} e implemente em {para}"),
        }
    }
}

fn aresta(s: &str) -> Result<(String, String)> {
    match s.split_once("->") {
        Some((a, b)) if !a.trim().is_empty() && !b.trim().is_empty() => {
            Ok((a.trim().to_string(), b.trim().to_string()))
        }
        _ => bail!("aresta inválida {s:?}: use \"origem -> destino\""),
    }
}

fn montar(nome: &str, camadas: &[(&str, &[&str])], arestas: &[(&str, &str)]) -> Arquitetura {
    Arquitetura {
        nome: nome.to_string(),
        camadas: camadas
            .iter()
            .map(|(n, ps)| Camada {
                nome: n.to_string(),
                padroes: ps
                    .iter()
                    .map(|p| Padrao::novo(p).expect("padrão do preset"))
                    .collect(),
            })
            .collect(),
        permitidas: arestas
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect(),
        externos: Vec::new(),
    }
}

pub const PRESETS: &[&str] = &["hexagonal", "clean", "layered", "mvvm", "modular"];

const CORE: &[&str] = &["core", "common", "commons", "shared"];

/// Modular: uma camada por feature (`..feature.x..`/`..features.x..`) e o `core`. Cada feature
/// só importa o core; feature importando outra feature é ARQ001.
pub fn modular(projeto: &Projeto) -> Arquitetura {
    let mut features: BTreeSet<(String, String)> = BTreeSet::new();
    for p in &projeto.pacotes {
        let segs: Vec<&str> = p.split('.').collect();
        for w in segs.windows(2) {
            if matches!(w[0], "feature" | "features") {
                features.insert((w[0].to_string(), w[1].to_string()));
            }
        }
    }
    let mut camadas = vec![Camada {
        nome: "core".into(),
        padroes: CORE
            .iter()
            .map(|c| Padrao::novo(&format!("..{c}..")).expect("padrão do preset"))
            .collect(),
    }];
    let mut permitidas = HashSet::new();
    for (pai, f) in &features {
        let nome = format!("feature-{f}");
        camadas.push(Camada {
            nome: nome.clone(),
            padroes: vec![Padrao::novo(&format!("..{pai}.{f}..")).expect("padrão do preset")],
        });
        permitidas.insert((nome, "core".to_string()));
    }
    Arquitetura {
        nome: "modular".into(),
        camadas,
        permitidas,
        externos: Vec::new(),
    }
}

/// Preset pelo nome; o modular depende dos pacotes do projeto.
pub fn preset_do_projeto(nome: &str, projeto: &Projeto) -> Option<Arquitetura> {
    if nome == "modular" {
        return Some(modular(projeto));
    }
    preset(nome)
}

pub fn preset(nome: &str) -> Option<Arquitetura> {
    Some(match nome {
        "hexagonal" => montar(
            "hexagonal",
            &[
                ("dominio", &["..domain..", "..dominio.."]),
                (
                    "aplicacao",
                    &[
                        "..application..",
                        "..aplicacao..",
                        "..usecase..",
                        "..usecases..",
                        "..port..",
                        "..ports..",
                    ],
                ),
                (
                    "adaptadores-entrada",
                    &[
                        "..adapter.in..",
                        "..adapters.in..",
                        "..adapter.inbound..",
                        "..entrypoint..",
                        "..endpoint..",
                        "..controller..",
                        "..web..",
                        "..rest..",
                        "..api..",
                    ],
                ),
                (
                    "adaptadores-saida",
                    &[
                        "..adapter.out..",
                        "..adapters.out..",
                        "..adapter.outbound..",
                        "..infrastructure..",
                        "..infra..",
                        "..persistence..",
                        "..gateway..",
                    ],
                ),
            ],
            &[
                ("aplicacao", "dominio"),
                ("adaptadores-entrada", "aplicacao"),
                ("adaptadores-entrada", "dominio"),
                ("adaptadores-saida", "aplicacao"),
                ("adaptadores-saida", "dominio"),
            ],
        ),
        "clean" => montar(
            "clean",
            &[
                (
                    "entrada",
                    &[
                        "..presentation..",
                        "..entrypoint..",
                        "..endpoint..",
                        "..controller..",
                        "..web..",
                        "..rest..",
                        "..api..",
                    ],
                ),
                (
                    "casos-de-uso",
                    &[
                        "..usecase..",
                        "..usecases..",
                        "..application..",
                        "..interactor..",
                    ],
                ),
                (
                    "dominio",
                    &["..domain..", "..dominio..", "..entity..", "..entities.."],
                ),
                (
                    "infra",
                    &[
                        "..infrastructure..",
                        "..infra..",
                        "..data..",
                        "..persistence..",
                        "..gateway..",
                    ],
                ),
            ],
            &[
                ("entrada", "casos-de-uso"),
                ("entrada", "dominio"),
                ("casos-de-uso", "dominio"),
                ("infra", "casos-de-uso"),
                ("infra", "dominio"),
            ],
        ),
        "layered" => montar(
            "layered",
            &[
                (
                    "web",
                    &[
                        "..controller..",
                        "..controllers..",
                        "..web..",
                        "..rest..",
                        "..endpoint..",
                        "..api..",
                    ],
                ),
                ("servico", &["..service..", "..services..", "..servico.."]),
                (
                    "repositorio",
                    &[
                        "..repository..",
                        "..repositories..",
                        "..repositorio..",
                        "..dao..",
                        "..persistence..",
                    ],
                ),
                (
                    "modelo",
                    &[
                        "..model..",
                        "..models..",
                        "..entity..",
                        "..entities..",
                        "..domain..",
                        "..dto..",
                    ],
                ),
            ],
            &[
                ("web", "servico"),
                ("servico", "repositorio"),
                ("web", "modelo"),
                ("servico", "modelo"),
                ("repositorio", "modelo"),
            ],
        ),
        "mvvm" => {
            let mut a = montar(
                "mvvm",
                &[
                    (
                        "ui",
                        &[
                            "..ui..",
                            "..view..",
                            "..views..",
                            "..screen..",
                            "..screens..",
                            "..compose..",
                            "..activity..",
                            "..fragment..",
                        ],
                    ),
                    (
                        "viewmodel",
                        &["..viewmodel..", "..viewmodels..", "..presentation.."],
                    ),
                    ("dominio", &["..domain..", "..usecase..", "..usecases.."]),
                    (
                        "dados",
                        &[
                            "..data..",
                            "..repository..",
                            "..datasource..",
                            "..remote..",
                            "..local..",
                            "..network..",
                        ],
                    ),
                ],
                &[
                    ("ui", "viewmodel"),
                    ("ui", "dominio"),
                    ("viewmodel", "dominio"),
                    ("dados", "dominio"),
                ],
            );
            a.externos = vec![(
                "viewmodel".into(),
                vec!["android.view".into(), "android.widget".into()],
            )];
            a
        }
        _ => return None,
    })
}

/// Arquitetura a partir do guarda.yml (preset nomeado, com camadas e exceções sobrescritas).
pub fn da_config(cfg: &config::Arquitetura, projeto: &Projeto) -> Result<Option<Arquitetura>> {
    let mut arq = match cfg.preset.as_str() {
        "auto" => return Ok(None),
        "nenhum" => Arquitetura {
            nome: "nenhum".into(),
            camadas: Vec::new(),
            permitidas: HashSet::new(),
            externos: Vec::new(),
        },
        nome => match preset_do_projeto(nome, projeto) {
            Some(a) => a,
            None => bail!(
                "guarda.yml arquitetura.preset: {nome:?} não existe (use auto, nenhum, {})",
                PRESETS.join(", ")
            ),
        },
    };
    if let Some(camadas) = &cfg.camadas {
        arq.camadas = camadas
            .iter()
            .map(|(nome, ps)| {
                Ok(Camada {
                    nome: nome.clone(),
                    padroes: ps.iter().map(|p| Padrao::novo(p)).collect::<Result<_>>()?,
                })
            })
            .collect::<Result<_>>()?;
        let nomes: HashSet<&str> = arq.camadas.iter().map(|c| c.nome.as_str()).collect();
        arq.permitidas
            .retain(|(a, b)| nomes.contains(a.as_str()) && nomes.contains(b.as_str()));
    }
    for s in cfg.excecoes.iter().chain(&cfg.permitidas) {
        arq.permitidas.insert(aresta(s)?);
    }
    Ok(Some(arq))
}

/// Um arquivo do índice: pacote declarado e imports.
pub struct ArquivoIndice<'a> {
    pub arquivo: &'a str,
    pub pacote: Option<&'a str>,
    pub imports: &'a [Import],
    /// Anotações das classes do arquivo (nome, linha).
    pub anotacoes: &'a [(String, usize)],
    /// Exceções instanciadas no arquivo.
    pub excecoes: &'a [crate::lang::ExcecaoCriada],
}

/// ARQ001: import de um pacote do projeto que pertence a uma camada proibida.
pub fn verificar(
    arq: &Arquitetura,
    projeto: &Projeto,
    arquivos: &[ArquivoIndice],
) -> Vec<Violacao> {
    let mut v = Vec::new();
    for a in arquivos {
        let Some(pkg) = a.pacote else { continue };
        let Some(de) = arq.camada_de(pkg, projeto) else {
            continue;
        };
        let simbolo = nome_do_arquivo(a.arquivo);
        for imp in a.imports {
            if imp.pacote.is_empty() || !projeto.interno(&imp.pacote) {
                continue;
            }
            let Some(para) = arq.camada_de(&imp.pacote, projeto) else {
                continue;
            };
            if arq.permitido(de, para) {
                continue;
            }
            v.push(Violacao::nova(
                "ARQ001",
                a.arquivo,
                imp.linha,
                format!("{de} importa {para} ({})", imp.nome),
                arq.dica(de, para),
                simbolo,
                &imp.texto,
                None,
            ));
        }
    }
    v
}

fn nome_do_arquivo(rel: &str) -> &str {
    let nome = rel.rsplit('/').next().unwrap_or(rel);
    nome.split('.').next().unwrap_or(nome)
}

/// `pacote` começa pelo prefixo, respeitando os segmentos (`android` não casa `androidx`).
fn comeca_com(pacote: &str, prefixo: &str) -> bool {
    pacote == prefixo || (pacote.starts_with(prefixo) && pacote[prefixo.len()..].starts_with('.'))
}

/// ARQ003: camada sem framework (padrão: dominio) importando Spring, JPA, Jackson...
pub fn framework(
    arq: &Arquitetura,
    projeto: &Projeto,
    arquivos: &[ArquivoIndice],
    cfg: &config::Arquitetura,
) -> Vec<Violacao> {
    let mut v = Vec::new();
    for a in arquivos {
        let Some(camada) = a.pacote.and_then(|p| arq.camada_de(p, projeto)) else {
            continue;
        };
        let mut proibidos: Vec<&String> = Vec::new();
        if cfg.sem_framework.iter().any(|c| c == camada) {
            proibidos.extend(&cfg.frameworks);
        }
        for (c, ps) in &arq.externos {
            if c == camada {
                proibidos.extend(ps);
            }
        }
        if proibidos.is_empty() {
            continue;
        }
        for imp in a.imports {
            if projeto.interno(&imp.pacote) {
                continue;
            }
            let Some(fw) = proibidos.iter().find(|f| comeca_com(&imp.pacote, f)) else {
                continue;
            };
            v.push(Violacao::nova(
                "ARQ003",
                a.arquivo,
                imp.linha,
                format!("{camada} usa framework ({fw}: {})", imp.nome),
                "mantenha esta camada sem framework: anote e converta na camada de infraestrutura",
                nome_do_arquivo(a.arquivo),
                &imp.texto,
                None,
            ));
        }
    }
    v
}

const CONTROLADORES: &[&str] = &[
    "RestController",
    "Controller",
    "ControllerAdvice",
    "RestControllerAdvice",
];
const CAMADAS_DE_ENTRADA: &[&str] = &["adaptadores-entrada", "entrada", "web"];

/// ARQ004: @RestController/@Controller fora da camada de entrada.
pub fn controladores(
    arq: &Arquitetura,
    projeto: &Projeto,
    arquivos: &[ArquivoIndice],
) -> Vec<Violacao> {
    let Some(entrada) = arq
        .camadas
        .iter()
        .map(|c| c.nome.as_str())
        .find(|c| CAMADAS_DE_ENTRADA.contains(c))
    else {
        return Vec::new();
    };
    let mut v = Vec::new();
    for a in arquivos {
        let camada = a.pacote.and_then(|p| arq.camada_de(p, projeto));
        if camada == Some(entrada) {
            continue;
        }
        for (nome, linha) in a.anotacoes {
            if !CONTROLADORES.contains(&nome.as_str()) {
                continue;
            }
            let onde = camada.map_or("fora de qualquer camada".to_string(), |c| format!("em {c}"));
            v.push(Violacao::nova(
                "ARQ004",
                a.arquivo,
                *linha,
                format!("@{nome} {onde}"),
                format!("mova a classe para a camada {entrada}"),
                nome_do_arquivo(a.arquivo),
                nome,
                None,
            ));
        }
    }
    v
}

/// Pacotes de exceções HTTP: o tipo criado é resolvido pelo import explícito.
const HTTP_PACOTES: &[&str] = &[
    "org.springframework.web",
    "org.springframework.http",
    "jakarta.ws.rs",
    "javax.ws.rs",
    "io.ktor",
    "io.micronaut.http",
];
/// Nomes que só existem como exceção HTTP (Spring, JAX-RS): valem mesmo sem import.
const HTTP_EXCLUSIVAS: &[&str] = &[
    "ErrorResponseException",
    "HttpClientErrorException",
    "HttpServerErrorException",
    "HttpStatusCodeException",
    "ResponseStatusException",
    "WebApplicationException",
];
/// Exceções genéricas que escondem uma regra de negócio (as de SML006 já são cobradas lá).
const GENERICAS_DE_NEGOCIO: &[&str] = &["IllegalArgumentException", "IllegalStateException"];

fn excecao_http(tipo: &str, imports: &[Import]) -> bool {
    HTTP_EXCLUSIVAS.contains(&tipo)
        || imports
            .iter()
            .any(|i| i.nome == tipo && HTTP_PACOTES.iter().any(|p| comeca_com(&i.pacote, p)))
}

/// NEG001: exceção HTTP criada numa camada de negócio; NEG002: IllegalArgument/IllegalState
/// criada lá (regra de negócio sem nome).
pub fn negocio(
    arq: &Arquitetura,
    projeto: &Projeto,
    arquivos: &[ArquivoIndice],
    cfg: &config::Arquitetura,
) -> Vec<Violacao> {
    let mut v = Vec::new();
    for a in arquivos {
        let Some(camada) = a.pacote.and_then(|p| arq.camada_de(p, projeto)) else {
            continue;
        };
        if !cfg.negocio.iter().any(|c| c == camada) {
            continue;
        }
        for e in a.excecoes {
            let (codigo, mensagem, dica) = if excecao_http(&e.tipo, a.imports) {
                (
                    "NEG001",
                    format!("{} em {camada}", e.tipo),
                    "lance uma exceção de domínio e traduza para HTTP na camada de entrada",
                )
            } else if GENERICAS_DE_NEGOCIO.contains(&e.tipo.as_str()) {
                (
                    "NEG002",
                    format!("{} genérica em {camada}", e.tipo),
                    "crie uma exceção com o nome da regra (ex.: SaldoInsuficiente)",
                )
            } else {
                continue;
            };
            v.push(Violacao::nova(
                codigo, a.arquivo, e.linha, mensagem, dica, &e.simbolo, &e.trecho, None,
            ));
        }
    }
    v
}

/// Fatia do pacote: o primeiro segmento abaixo do pacote base.
fn fatia<'a>(pacote: &'a str, projeto: &Projeto) -> Option<&'a str> {
    let base = projeto.base.as_deref().filter(|b| !b.is_empty())?;
    if !comeca_com(pacote, base) || pacote == base {
        return None;
    }
    pacote[base.len() + 1..].split('.').next()
}

/// ARQ002: ciclo entre fatias (pacotes de primeiro nível abaixo da base), pelos imports.
/// Cada arquivo do escopo recebe uma violação por fatia de destino que fecha um ciclo.
pub fn ciclos(projeto: &Projeto, todos: &[ArquivoIndice], escopo: &HashSet<&str>) -> Vec<Violacao> {
    use std::collections::BTreeMap;
    let mut arestas: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for a in todos {
        let Some(de) = a.pacote.and_then(|p| fatia(p, projeto)) else {
            continue;
        };
        for imp in a.imports {
            if let Some(para) = fatia(&imp.pacote, projeto).filter(|p| *p != de) {
                arestas.entry(de).or_default().insert(para);
            }
        }
    }
    // Componentes fortemente conexos (Kosaraju): o grafo de fatias é pequeno.
    let nos: BTreeSet<&str> = arestas
        .iter()
        .flat_map(|(a, bs)| std::iter::once(*a).chain(bs.iter().copied()))
        .collect();
    let mut inverso: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (a, bs) in &arestas {
        for b in bs {
            inverso.entry(b).or_default().insert(a);
        }
    }
    fn visitar<'a>(
        n: &'a str,
        g: &BTreeMap<&'a str, BTreeSet<&'a str>>,
        vistos: &mut BTreeSet<&'a str>,
        saida: &mut Vec<&'a str>,
    ) {
        if !vistos.insert(n) {
            return;
        }
        for m in g.get(n).into_iter().flatten() {
            visitar(m, g, vistos, saida);
        }
        saida.push(n);
    }
    let mut ordem = Vec::new();
    let mut vistos = BTreeSet::new();
    for n in &nos {
        visitar(n, &arestas, &mut vistos, &mut ordem);
    }
    let mut componente: BTreeMap<&str, usize> = BTreeMap::new();
    let mut vistos = BTreeSet::new();
    let mut membros: Vec<Vec<&str>> = Vec::new();
    for n in ordem.iter().rev() {
        if vistos.contains(n) {
            continue;
        }
        let mut grupo = Vec::new();
        visitar(n, &inverso, &mut vistos, &mut grupo);
        grupo.sort();
        for m in &grupo {
            componente.insert(m, membros.len());
        }
        membros.push(grupo);
    }

    let mut v = Vec::new();
    for a in todos.iter().filter(|a| escopo.contains(a.arquivo)) {
        let Some(de) = a.pacote.and_then(|p| fatia(p, projeto)) else {
            continue;
        };
        let Some(&c) = componente.get(de) else {
            continue;
        };
        if membros[c].len() < 2 {
            continue;
        }
        let mut ja = BTreeSet::new();
        for imp in a.imports {
            let Some(para) = fatia(&imp.pacote, projeto) else {
                continue;
            };
            if para == de || componente.get(para) != Some(&c) || !ja.insert(para) {
                continue;
            }
            v.push(Violacao::nova(
                "ARQ002",
                a.arquivo,
                imp.linha,
                format!(
                    "{de} → {para} fecha um ciclo entre fatias ({})",
                    membros[c].join(", ")
                ),
                "quebre o ciclo: o lado de dentro declara uma interface e o de fora implementa",
                nome_do_arquivo(a.arquivo),
                &format!("{de}->{para}"),
                None,
            ));
        }
    }
    v
}

#[derive(Debug, Clone)]
pub struct Pontuacao {
    pub preset: String,
    /// Confiança de 0 a 1.
    pub nota: f64,
    pub cobertos: usize,
    pub total: usize,
    pub com_violacao: usize,
    pub camadas_usadas: usize,
    /// Pacotes distintos (abaixo da base) que o preset classifica.
    pub pacotes_cobertos: usize,
    pub pacotes_total: usize,
    /// Nomes exclusivos do estilo achados nos pacotes (port, usecase, viewmodel...).
    pub marcas: usize,
}

/// Nomes de pacote que só um estilo usa: desempatam presets que classificam igual.
fn marcas_do_estilo(preset: &str) -> &'static [&'static str] {
    match preset {
        "hexagonal" => &[
            "port", "ports", "adapter", "adapters", "inbound", "outbound",
        ],
        "clean" => &[
            "usecase",
            "usecases",
            "interactor",
            "interactors",
            "presentation",
        ],
        "layered" => &["dao", "daos"],
        "mvvm" => &["viewmodel", "viewmodels"],
        "modular" => &["feature", "features"],
        _ => &[],
    }
}

/// Pontua cada preset pelos nomes de pacote e pelas violações que ele geraria.
///
/// nota = média(fração dos pacotes distintos classificados, fração dos arquivos
/// classificados) × (1 − metade da fração de arquivos com import proibido)
/// + 0,02 por nome exclusivo do estilo (até 3).
///
/// Pacotes distintos, em qualquer nível, cobrem tanto o projeto organizado por camada
/// (`controller`, `service` no 1º nível) quanto o organizado por feature
/// (`pedido.controller`, `pedido.service`). As violações pesam pela metade porque projeto
/// real tem desvios do próprio padrão, e um preset que deixa camadas inteiras de fora não
/// pode ganhar só por isso.
pub fn pontuar(projeto: &Projeto, arquivos: &[ArquivoIndice]) -> Vec<Pontuacao> {
    let pacotes: BTreeSet<&str> = arquivos
        .iter()
        .filter_map(|a| a.pacote)
        .filter(|p| !projeto.relativo(p).is_empty())
        .collect();
    let segmentos: BTreeSet<&str> = pacotes
        .iter()
        .flat_map(|p| projeto.relativo(p).split('.'))
        .collect();
    let mut res: Vec<Pontuacao> = PRESETS
        .iter()
        .map(|nome| {
            let arq = preset_do_projeto(nome, projeto).expect("preset");
            let mut total = 0;
            let mut cobertos = 0;
            let mut usadas = BTreeSet::new();
            let mut arquivos_com_violacao = BTreeSet::new();
            for a in arquivos {
                let Some(pkg) = a.pacote else { continue };
                total += 1;
                if let Some(c) = arq.camada_de(pkg, projeto) {
                    cobertos += 1;
                    usadas.insert(c.to_string());
                }
            }
            for viol in verificar(&arq, projeto, arquivos) {
                arquivos_com_violacao.insert(viol.arquivo);
            }
            let pacotes_cobertos = pacotes
                .iter()
                .filter(|p| arq.camada_de(p, projeto).is_some())
                .count();
            let marcas = marcas_do_estilo(nome)
                .iter()
                .filter(|m| segmentos.contains(*m))
                .count();
            let nota = if total == 0 || usadas.len() < 2 {
                0.0
            } else {
                let p = pacotes_cobertos as f64 / pacotes.len().max(1) as f64;
                let c = cobertos as f64 / total as f64;
                let v = arquivos_com_violacao.len() as f64 / cobertos.max(1) as f64;
                (p + c) / 2.0 * (1.0 - v / 2.0) + 0.02 * marcas.min(3) as f64
            };
            Pontuacao {
                preset: nome.to_string(),
                nota: nota.min(1.0),
                cobertos,
                total,
                com_violacao: arquivos_com_violacao.len(),
                camadas_usadas: usadas.len(),
                pacotes_cobertos,
                pacotes_total: pacotes.len(),
                marcas,
            }
        })
        .collect();
    // Ordem estável: nota desc, depois a ordem de PRESETS.
    res.sort_by(|a, b| {
        b.nota
            .partial_cmp(&a.nota)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    res
}

pub const CONFIANCA_MINIMA: f64 = 0.6;

#[cfg(test)]
mod testes {
    use super::*;

    fn projeto(base: &str) -> Projeto {
        Projeto::novo(Some(base), HashSet::new())
    }

    fn imp(pacote: &str, nome: &str, linha: usize) -> Import {
        Import {
            pacote: pacote.into(),
            nome: nome.into(),
            linha,
            texto: format!("import {pacote}.{nome};"),
        }
    }

    #[test]
    fn arq002_003_004() {
        let p = projeto("com.app");
        let arq = preset("hexagonal").unwrap();
        let dom = vec![
            imp("jakarta.persistence", "Entity", 3),
            imp("com.app.infra.db", "Repo", 4),
            imp("androidx.room", "Dao", 5),
        ];
        let infra = vec![imp("com.app.domain", "Usuario", 3)];
        let app = vec![imp("com.app.domain", "Usuario", 3)];
        let anot_ctrl = vec![("RestController".to_string(), 7)];
        let arquivos = [
            ArquivoIndice {
                arquivo: "D.java",
                pacote: Some("com.app.domain"),
                imports: &dom,
                anotacoes: &[],
                excecoes: &[],
            },
            ArquivoIndice {
                arquivo: "I.java",
                pacote: Some("com.app.infra.db"),
                imports: &infra,
                anotacoes: &[],
                excecoes: &[],
            },
            ArquivoIndice {
                arquivo: "C.java",
                pacote: Some("com.app.application"),
                imports: &app,
                anotacoes: &anot_ctrl,
                excecoes: &[],
            },
        ];
        let cfg = config::Arquitetura::default();
        let f: Vec<String> = framework(&arq, &p, &arquivos, &cfg)
            .into_iter()
            .map(|v| v.mensagem)
            .collect();
        assert_eq!(
            f,
            [
                "dominio usa framework (jakarta.persistence: Entity)",
                "dominio usa framework (androidx: Dao)"
            ]
        );
        let c = controladores(&arq, &p, &arquivos);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].mensagem, "@RestController em aplicacao");
        assert_eq!(c[0].linha, 7);

        let escopo: HashSet<&str> = ["D.java", "I.java", "C.java"].into_iter().collect();
        let ciclo: Vec<(&str, usize)> = ciclos(&p, &arquivos, &escopo)
            .iter()
            .map(|v| (if v.arquivo == "D.java" { "D" } else { "I" }, v.linha))
            .collect();
        // domain ↔ infra é ciclo; application → domain não é.
        assert_eq!(ciclo, [("D", 4), ("I", 3)]);
    }

    #[test]
    fn modular_feature_so_importa_core() {
        let pacotes: HashSet<String> = [
            "com.app.core.rede",
            "com.app.feature.login",
            "com.app.feature.login.ui",
            "com.app.feature.pagamento",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let p = Projeto::novo(None, pacotes);
        let arq = modular(&p);
        assert_eq!(
            arq.camada_de("com.app.feature.login.ui", &p),
            Some("feature-login")
        );
        assert_eq!(arq.camada_de("com.app.core.rede", &p), Some("core"));
        let imports = vec![
            imp("com.app.core.rede", "Http", 3),
            imp("com.app.feature.pagamento", "Cartao", 4),
        ];
        let arquivos = [ArquivoIndice {
            arquivo: "Login.kt",
            pacote: Some("com.app.feature.login"),
            imports: &imports,
            anotacoes: &[],
            excecoes: &[],
        }];
        let v = verificar(&arq, &p, &arquivos);
        assert_eq!(v.len(), 1);
        assert_eq!(
            v[0].mensagem,
            "feature-login importa feature-pagamento (Cartao)"
        );
    }

    #[test]
    fn mvvm_viewmodel_sem_android_view() {
        let p = projeto("com.app");
        let arq = preset("mvvm").unwrap();
        let imports = vec![
            imp("android.widget", "TextView", 2),
            imp("androidx.lifecycle", "ViewModel", 3),
        ];
        let arquivos = [ArquivoIndice {
            arquivo: "LoginViewModel.kt",
            pacote: Some("com.app.viewmodel"),
            imports: &imports,
            anotacoes: &[],
            excecoes: &[],
        }];
        let cfg = config::Arquitetura::default();
        let v = framework(&arq, &p, &arquivos, &cfg);
        assert_eq!(v.len(), 1, "{v:?}");
        assert_eq!(
            v[0].mensagem,
            "viewmodel usa framework (android.widget: TextView)"
        );
    }

    #[test]
    fn padrao_archunit() {
        let p = projeto("com.app");
        let arq = preset("hexagonal").unwrap();
        assert_eq!(arq.camada_de("com.app.domain.model", &p), Some("dominio"));
        assert_eq!(
            arq.camada_de("com.app.adapter.in.rest", &p),
            Some("adaptadores-entrada")
        );
        assert_eq!(
            arq.camada_de("com.app.infrastructure.entity", &p),
            Some("adaptadores-saida")
        );
        // A camada mais rasa vence: usecase dentro de domain é domínio.
        assert_eq!(arq.camada_de("com.app.domain.usecase", &p), Some("dominio"));
        assert_eq!(arq.camada_de("com.app.config", &p), None);
    }

    #[test]
    fn base_com_nome_de_camada_nao_confunde() {
        // "api" no pacote base não pode jogar tudo na camada de entrada.
        let p = projeto("com.empresa.api");
        let arq = preset("hexagonal").unwrap();
        assert_eq!(arq.camada_de("com.empresa.api.domain", &p), Some("dominio"));
        assert_eq!(arq.camada_de("com.empresa.api", &p), None);
    }

    #[test]
    fn padrao_absoluto_e_curinga() {
        let p = Padrao::novo("com.app..").unwrap();
        assert_eq!(p.casar("com.app.x", "", 0), Some(0));
        assert_eq!(p.casar("org.app.x", "", 0), None);
        let p = Padrao::novo("..*Service..").unwrap();
        assert!(p.casar("a.UserService", "a.UserService", 0).is_some());
    }

    #[test]
    fn prefixo() {
        let s: HashSet<String> = ["a.b.c", "a.b.d.e", "a.b"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(prefixo_comum(&s).as_deref(), Some("a.b"));
    }

    #[test]
    fn arq001_dominio_importando_infra() {
        let p = projeto("com.app");
        let arq = preset("hexagonal").unwrap();
        let imports = vec![Import {
            pacote: "com.app.infra.http".into(),
            nome: "HttpClient".into(),
            linha: 3,
            texto: "import com.app.infra.http.HttpClient;".into(),
        }];
        let arquivos = [ArquivoIndice {
            arquivo: "src/main/java/com/app/domain/Nota.java",
            pacote: Some("com.app.domain"),
            imports: &imports,
            anotacoes: &[],
            excecoes: &[],
        }];
        let v = verificar(&arq, &p, &arquivos);
        assert_eq!(v.len(), 1);
        // O mesmo import também é framework? Não: HttpClient é do projeto.
        let cfg = config::Arquitetura::default();
        assert!(framework(&arq, &p, &arquivos, &cfg).is_empty());
        assert_eq!(
            v[0].mensagem,
            "dominio importa adaptadores-saida (HttpClient)"
        );
        assert_eq!(v[0].linha, 3);
    }
}
