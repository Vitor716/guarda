//! `guarda.yml`: opcional, tudo que não for declarado herda o padrão.
//!
//! Seções da especificação que ainda não têm efeito (estende, perfil, cobertura,
//! duplicação, proibir, personalizadas) são aceitas para o arquivo já poder ser
//! escrito no formato final, mas ignoradas nesta fase.

use crate::modelo::Severidade;
use anyhow::Result;
use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::Deserialize;
use std::path::Path;

pub const ARQUIVO: &str = "guarda.yml";
/// Onde o `init` grava o JSON Schema, referenciado na primeira linha do guarda.yml.
pub const ARQUIVO_SCHEMA: &str = ".guarda/guarda.schema.json";

/// Configuração da guarda. Tudo é opcional: o que não for declarado herda o perfil recomendado.
#[derive(Debug, Deserialize, Default, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
#[schemars(title = "guarda.yml")]
pub struct Config {
    /// Versão do formato do arquivo.
    pub versao: Option<u32>,
    /// Arquivo base a importar (ainda sem efeito: Fase 3).
    #[schemars(with = "Option<serde_json::Value>")]
    pub estende: Option<serde_yaml::Value>,
    /// Perfil de limites (ainda sem efeito: hoje vale sempre o recomendado).
    #[schemars(extend("enum" = ["minimo", "recomendado", "rigoroso"]))]
    pub perfil: Option<String>,
    pub fontes: Fontes,
    pub arquitetura: Arquitetura,
    pub limites: Limites,
    pub regras: Regras,
    /// Cobertura de testes lida do XML do JaCoCo/Kover. Sem esta seção, não é verificada.
    pub cobertura: Option<Cobertura>,
    /// Código duplicado por tokens (estilo PMD CPD). Sem esta seção, não é verificada.
    pub duplicacao: Option<Duplicacao>,
    pub gate: Gate,
}

/// Relatórios de cobertura: `auto` ou a lista de XMLs.
#[derive(Debug, Deserialize, JsonSchema, Clone)]
#[serde(untagged)]
pub enum Relatorios {
    /// `auto`: procura o XML do JaCoCo e do Kover em cada módulo.
    Um(String),
    /// Caminhos dos XMLs, relativos à raiz.
    Varios(Vec<String>),
}

impl Default for Relatorios {
    fn default() -> Self {
        Self::Um("auto".into())
    }
}

/// Cobertura no estilo do Quality Gate "Sonar way".
#[derive(Debug, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Cobertura {
    pub relatorio: Relatorios,
    pub codigo_novo: CodigoNovo,
    /// COV003: cobertura mínima do projeto inteiro (%).
    pub geral_minimo: Option<f64>,
    /// COV002: cobertura mínima por pacote (padrão ArchUnit → %).
    pub por_pacote: IndexMap<String, f64>,
    /// Arquivos fora da conta (globs relativos à raiz).
    pub ignorar: Vec<String>,
    /// Comando que roda os testes e gera o XML (`check --cobertura`). Padrão: pelo build.
    pub comando: Option<String>,
    /// O hook `final` roda os testes antes de verificar (lento: desligado por padrão).
    pub rodar_no_final: bool,
}

/// O que conta como código novo e quanto dele precisa de teste.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct CodigoNovo {
    /// Referência do git: código novo = diferença desde o ancestral comum com ela.
    pub referencia: String,
    /// COV001: cobertura mínima do código novo (%). Sonar way: 80.
    pub minimo: f64,
}

impl Default for CodigoNovo {
    fn default() -> Self {
        Self {
            referencia: "origin/main".into(),
            minimo: 80.0,
        }
    }
}

/// Duplicação por tokens (Karp-Rabin, como o PMD CPD).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Duplicacao {
    /// Tamanho mínimo de um bloco duplicado, em tokens.
    pub tokens_minimos: usize,
    /// Máximo de linhas duplicadas no código novo (%). Sonar way: 3.
    pub codigo_novo_maximo: f64,
    /// Referência do código novo, se não houver seção `cobertura`.
    pub referencia: Option<String>,
}

impl Default for Duplicacao {
    fn default() -> Self {
        Self {
            tokens_minimos: 100,
            codigo_novo_maximo: 3.0,
            referencia: None,
        }
    }
}

/// Quais arquivos entram na análise (globs relativos à raiz).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Fontes {
    /// Código de produção: é o que a guarda analisa.
    pub producao: Vec<String>,
    /// Código de teste (regras de teste chegam na Fase 3).
    pub testes: Vec<String>,
    /// Nunca analisados.
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

/// Camadas e dependências permitidas entre elas (regra ARQ001).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Arquitetura {
    /// `auto` detecta pelo nome dos pacotes; `nenhum` exige declarar camadas e `permitidas`.
    #[schemars(extend("enum" = ["auto", "hexagonal", "clean", "layered", "nenhum"]))]
    pub preset: String,
    /// Pacote base do projeto. Sem ele, é o prefixo comum dos pacotes declarados.
    pub base: Option<String>,
    /// Substitui as camadas do preset (nome → padrões de pacote no estilo ArchUnit).
    pub camadas: Option<IndexMap<String, Vec<String>>>,
    /// Arestas extras liberadas, no formato `origem -> destino`.
    pub excecoes: Vec<String>,
    /// Arestas permitidas para camadas declaradas à mão (preset `nenhum`).
    pub permitidas: Vec<String>,
    /// ARQ002: ciclos entre fatias (pacotes de primeiro nível abaixo da base).
    #[schemars(extend("enum" = ["proibir", "permitir"]))]
    pub ciclos: Option<String>,
    /// ARQ003: camadas que não podem importar framework.
    pub sem_framework: Vec<String>,
    /// ARQ003: prefixos de pacote considerados framework.
    pub frameworks: Vec<String>,
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
            sem_framework: vec!["dominio".into()],
            frameworks: [
                "org.springframework",
                "jakarta.persistence",
                "javax.persistence",
                "org.hibernate",
                "com.fasterxml.jackson",
                "io.micronaut",
                "io.quarkus",
                "io.ktor",
                "android",
                "androidx",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        }
    }
}

/// Limites das regras de tamanho e complexidade. A violação é acima do limite.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Limites {
    /// CPX005: linhas de código por função (sem brancos e comentários). detekt: 60.
    pub linhas_funcao: u32,
    /// CPX001: complexidade cognitiva por função. Sonar S3776: 15.
    pub complexidade_cognitiva: u32,
    /// CPX002: complexidade ciclomática por função. detekt e PMD: 15.
    pub complexidade_ciclomatica: u32,
    /// CPX003: profundidade de blocos aninhados. detekt NestedBlockDepth: 4.
    pub aninhamento: u32,
    /// CPX004: operandos de && e || numa condição. detekt ComplexCondition: 4.
    pub condicao_operadores: u32,
    /// CPX006: linhas de código por classe. detekt LargeClass: 600.
    pub linhas_classe: u32,
    /// CPX007: parâmetros por função. detekt LongParameterList: 6.
    pub parametros: u32,
    /// CPX008: funções por classe. detekt TooManyFunctions: 11.
    pub funcoes_por_classe: u32,
    /// CPX009: `return` por função. detekt ReturnCount: 2.
    pub retornos: u32,
}

impl Default for Limites {
    /// Perfil recomendado: os padrões do detekt e do Sonar.
    fn default() -> Self {
        Self {
            linhas_funcao: 60,
            complexidade_cognitiva: 15,
            complexidade_ciclomatica: 15,
            aninhamento: 4,
            condicao_operadores: 4,
            linhas_classe: 600,
            parametros: 6,
            funcoes_por_classe: 11,
            retornos: 2,
        }
    }
}

/// Liga, desliga e ajusta regras pelo código (ARQ001, KT001...).
#[derive(Debug, Deserialize, Default, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Regras {
    /// Códigos de regra desligados.
    pub desligar: Vec<String>,
    /// Severidade por código de regra.
    pub severidade: IndexMap<String, Severidade>,
    /// Imports e tipos proibidos (ainda sem efeito: Fase 3).
    #[schemars(with = "Option<serde_json::Value>")]
    pub proibir: Option<serde_yaml::Value>,
    /// Pasta de regras extras em YAML (ainda sem efeito: Fase 3).
    #[schemars(with = "Option<serde_json::Value>")]
    pub personalizadas: Option<serde_yaml::Value>,
}

/// O que reprova a verificação.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Gate {
    /// Severidade mínima que reprova.
    pub falhar_em: Severidade,
    /// Caminho do baseline, relativo à raiz.
    pub baseline: String,
    /// Política de supressão (só `exigir-motivo` tem efeito nesta fase).
    #[schemars(extend("enum" = ["proibida", "exigir-motivo", "livre"]))]
    pub supressao: String,
    /// Quantas vezes o hook `final` segura a IA antes de liberar e deixar o relatório.
    pub tentativas_final: u32,
}

impl Default for Gate {
    fn default() -> Self {
        Self {
            falhar_em: Severidade::Erro,
            baseline: ".guarda/baseline.json".into(),
            supressao: "exigir-motivo".into(),
            tentativas_final: 3,
        }
    }
}

/// JSON Schema do guarda.yml (autocomplete e validação no editor e na IA).
pub fn schema() -> String {
    let s = schemars::schema_for!(Config);
    serde_json::to_string_pretty(&s).expect("schema serializável") + "\n"
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
        serde_yaml::from_str(texto).map_err(|e| {
            let linha = e.location().map(|l| l.line()).unwrap_or(0);
            anyhow::anyhow!("{ARQUIVO}:{linha} {e}")
        })
    }

    /// Referência do git que define o código novo.
    pub fn referencia(&self) -> &str {
        self.cobertura
            .as_ref()
            .map(|c| c.codigo_novo.referencia.as_str())
            .or_else(|| {
                self.duplicacao
                    .as_ref()
                    .and_then(|d| d.referencia.as_deref())
            })
            .unwrap_or("origin/main")
    }

    pub fn cobertura_no_final(&self) -> bool {
        self.cobertura.as_ref().is_some_and(|c| c.rodar_no_final)
    }

    /// Impressão digital do que muda o resultado das regras por arquivo (para o cache).
    pub fn impressao_regras(&self) -> String {
        format!("{}|{:?}|regras3", env!("CARGO_PKG_VERSION"), self.limites)
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
