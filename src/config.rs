//! `guarda.yml`: opcional, tudo que não for declarado herda o padrão.
//!
//! Seções da especificação que ainda não têm efeito (estende, perfil, cobertura,
//! duplicação, proibir, personalizadas) são aceitas para o arquivo já poder ser
//! escrito no formato final, mas ignoradas nesta fase.

use anyhow::{Result, bail};
use indexmap::IndexMap;
use serde::Deserialize;
use std::path::Path;

pub const ARQUIVO: &str = "guarda.yml";

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Config {
    pub versao: Option<u32>,
    pub estende: Option<serde_yaml::Value>,
    pub perfil: Option<String>,
    pub fontes: Fontes,
    pub arquitetura: Arquitetura,
    pub limites: Limites,
    pub regras: Regras,
    pub cobertura: Option<serde_yaml::Value>,
    pub duplicacao: Option<serde_yaml::Value>,
    pub gate: Gate,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Fontes {
    pub producao: Vec<String>,
    pub testes: Vec<String>,
    pub ignorar: Vec<String>,
}

impl Default for Fontes {
    fn default() -> Self {
        Self {
            producao: vec!["**/src/main/**".into()],
            testes: vec!["**/src/test/**".into(), "**/src/androidTest/**".into()],
            ignorar: vec![
                "**/build/**".into(),
                "**/target/**".into(),
                "**/generated/**".into(),
                "**/out/**".into(),
            ],
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Arquitetura {
    /// `auto` | `layered` | `clean` | `hexagonal` | `nenhum`
    pub preset: String,
    /// Pacote base do projeto. Sem ele, é o prefixo comum dos pacotes declarados.
    pub base: Option<String>,
    /// Substitui as camadas do preset (nome → padrões de pacote no estilo ArchUnit).
    pub camadas: Option<IndexMap<String, Vec<String>>>,
    /// Arestas extras liberadas, no formato `origem -> destino`.
    pub excecoes: Vec<String>,
    /// Arestas permitidas para camadas declaradas à mão (preset `nenhum`).
    pub permitidas: Vec<String>,
    pub ciclos: Option<String>,
}

impl Default for Arquitetura {
    fn default() -> Self {
        Self {
            preset: "auto".into(),
            base: None,
            camadas: None,
            excecoes: Vec::new(),
            permitidas: Vec::new(),
            ciclos: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Limites {
    pub linhas_funcao: u32,
    pub complexidade_cognitiva: u32,
    pub linhas_classe: Option<u32>,
    pub parametros: Option<u32>,
    pub aninhamento: Option<u32>,
}

impl Default for Limites {
    /// Perfil recomendado: detekt LongMethod (60) e Sonar S3776 (15).
    fn default() -> Self {
        Self {
            linhas_funcao: 60,
            complexidade_cognitiva: 15,
            linhas_classe: None,
            parametros: None,
            aninhamento: None,
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Regras {
    pub desligar: Vec<String>,
    pub severidade: IndexMap<String, String>,
    pub proibir: Option<serde_yaml::Value>,
    pub personalizadas: Option<serde_yaml::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Gate {
    /// `erro` | `aviso`
    pub falhar_em: String,
    pub baseline: String,
    /// `proibida` | `exigir-motivo` | `livre` (só `exigir-motivo` nesta fase)
    pub supressao: String,
}

impl Default for Gate {
    fn default() -> Self {
        Self {
            falhar_em: "erro".into(),
            baseline: ".guarda/baseline.json".into(),
            supressao: "exigir-motivo".into(),
        }
    }
}

impl Config {
    pub fn carregar(raiz: &Path) -> Result<Self> {
        let caminho = raiz.join(ARQUIVO);
        if !caminho.exists() {
            return Ok(Self::default());
        }
        let texto = std::fs::read_to_string(&caminho)?;
        Self::de_texto(&texto)
    }

    pub fn de_texto(texto: &str) -> Result<Self> {
        if texto.trim().is_empty() {
            return Ok(Self::default());
        }
        let cfg: Self = serde_yaml::from_str(texto).map_err(|e| {
            let linha = e.location().map(|l| l.line()).unwrap_or(0);
            anyhow::anyhow!("{ARQUIVO}:{linha} {e}")
        })?;
        cfg.validar()?;
        Ok(cfg)
    }

    fn validar(&self) -> Result<()> {
        for (codigo, sev) in &self.regras.severidade {
            if crate::modelo::Severidade::de_texto(sev).is_none() {
                bail!("{ARQUIVO} regras.severidade.{codigo}: esperado erro ou aviso, veio {sev:?}");
            }
        }
        if !matches!(self.gate.falhar_em.as_str(), "erro" | "aviso") {
            bail!("{ARQUIVO} gate.falhar-em: esperado erro ou aviso");
        }
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn vazio_usa_padrao() {
        let c = Config::de_texto("").unwrap();
        assert_eq!(c.limites.linhas_funcao, 60);
        assert_eq!(c.arquitetura.preset, "auto");
    }

    #[test]
    fn campo_desconhecido_aponta_linha() {
        let e = Config::de_texto("versao: 1\nlimites:\n  linhas-fucao: 3\n").unwrap_err();
        assert!(e.to_string().starts_with("guarda.yml:3"), "{e}");
    }

    #[test]
    fn exemplo_da_especificacao_e_aceito() {
        let yml = r#"
versao: 1
perfil: recomendado
fontes:
  producao: ["**/src/main/**"]
arquitetura:
  preset: hexagonal
  camadas:
    dominio: ["..domain.."]
    aplicacao: ["..application..", "..usecase.."]
    adaptadores: ["..adapter..", "..infra.."]
  excecoes:
    - adaptadores -> dominio
  ciclos: proibir
limites:
  linhas-funcao: 30
  linhas-classe: 400
  complexidade-cognitiva: 15
  parametros: 5
  aninhamento: 4
regras:
  desligar: [KT014]
  severidade: { CPX003: erro }
cobertura:
  relatorio: auto
duplicacao:
  tokens-minimos: 100
gate:
  falhar-em: erro
  baseline: .guarda/baseline.json
  supressao: exigir-motivo
"#;
        let c = Config::de_texto(yml).unwrap();
        assert_eq!(c.limites.linhas_funcao, 30);
        assert_eq!(c.arquitetura.camadas.unwrap().len(), 3);
    }
}
