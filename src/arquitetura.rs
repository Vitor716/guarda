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
    }
}

pub const PRESETS: &[&str] = &["hexagonal", "clean", "layered"];

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
        _ => return None,
    })
}

/// Arquitetura a partir do guarda.yml (preset nomeado, com camadas e exceções sobrescritas).
pub fn da_config(cfg: &config::Arquitetura) -> Result<Option<Arquitetura>> {
    let mut arq = match cfg.preset.as_str() {
        "auto" => return Ok(None),
        "nenhum" => Arquitetura {
            nome: "nenhum".into(),
            camadas: Vec::new(),
            permitidas: HashSet::new(),
        },
        nome => match preset(nome) {
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

#[derive(Debug, Clone)]
pub struct Pontuacao {
    pub preset: String,
    /// Confiança de 0 a 1.
    pub nota: f64,
    pub cobertos: usize,
    pub total: usize,
    pub com_violacao: usize,
    pub camadas_usadas: usize,
    /// Pacotes de primeiro nível abaixo da base que o preset classifica.
    pub topo_cobertos: usize,
    pub topo_total: usize,
}

/// Pontua cada preset pelos nomes de pacote e pelas violações que ele geraria.
///
/// nota = média(fração dos pacotes de primeiro nível classificados, fração dos arquivos
/// classificados) × (1 − metade da fração de arquivos com import proibido).
/// Os pacotes de primeiro nível pesam porque são a decisão de arquitetura do projeto; as
/// violações pesam pela metade porque um projeto real costuma ter desvios do próprio padrão,
/// e um preset que deixa camadas inteiras de fora não pode ganhar só por isso.
pub fn pontuar(projeto: &Projeto, arquivos: &[ArquivoIndice]) -> Vec<Pontuacao> {
    let topo: BTreeSet<&str> = arquivos
        .iter()
        .filter_map(|a| a.pacote)
        .map(|p| projeto.relativo(p))
        .filter(|r| !r.is_empty())
        .filter_map(|r| r.split('.').next())
        .collect();
    let mut res: Vec<Pontuacao> = PRESETS
        .iter()
        .map(|nome| {
            let arq = preset(nome).expect("preset");
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
            let topo_cobertos = topo
                .iter()
                .filter(|s| {
                    let pkg = match &projeto.base {
                        Some(b) if !b.is_empty() => format!("{b}.{s}"),
                        _ => s.to_string(),
                    };
                    arq.camada_de(&pkg, projeto).is_some()
                })
                .count();
            let nota = if total == 0 || usadas.len() < 2 {
                0.0
            } else {
                let t = topo_cobertos as f64 / topo.len().max(1) as f64;
                let c = cobertos as f64 / total as f64;
                let v = arquivos_com_violacao.len() as f64 / cobertos.max(1) as f64;
                (t + c) / 2.0 * (1.0 - v / 2.0)
            };
            Pontuacao {
                preset: nome.to_string(),
                nota,
                cobertos,
                total,
                com_violacao: arquivos_com_violacao.len(),
                camadas_usadas: usadas.len(),
                topo_cobertos,
                topo_total: topo.len(),
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
        }];
        let v = verificar(&arq, &p, &arquivos);
        assert_eq!(v.len(), 1);
        assert_eq!(
            v[0].mensagem,
            "dominio importa adaptadores-saida (HttpClient)"
        );
        assert_eq!(v[0].linha, 3);
    }
}
