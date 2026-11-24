//! `guarda.yml`: opcional, tudo que não for declarado herda o padrão.
//!
//! Ordem de montagem: o arquivo de `estende` (recursivo) é a base; o guarda.yml do projeto
//! sobrescreve chave a chave (listas são trocadas inteiras); os limites que ninguém declarou
//! vêm do `perfil`.

use crate::modelo::{self, Severidade};
use anyhow::{Context, Result, bail};
use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
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
    /// guarda.yml base (caminho relativo a este arquivo), ex.: o padrão do time. Este arquivo
    /// sobrescreve a base chave a chave.
    pub estende: Option<String>,
    /// `minimo`: só as regras que já são erro, limites folgados. `recomendado`: padrões do
    /// detekt e do Sonar. `rigoroso`: limites apertados e toda regra na severidade alvo.
    pub perfil: Option<Perfil>,
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

#[derive(Debug, Deserialize, JsonSchema, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Perfil {
    Minimo,
    #[default]
    Recomendado,
    Rigoroso,
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
    #[schemars(extend("enum" = ["auto", "hexagonal", "clean", "layered", "mvvm", "modular", "nenhum"]))]
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
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
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

impl Limites {
    /// Limites de cada perfil. Recomendado = padrões do detekt e do Sonar; os outros dois
    /// são escolha da guarda (folga e aperto em torno deles).
    pub fn do_perfil(p: Perfil) -> Self {
        match p {
            Perfil::Recomendado => Self::default(),
            Perfil::Minimo => Self {
                linhas_funcao: 100,
                complexidade_cognitiva: 25,
                complexidade_ciclomatica: 25,
                aninhamento: 6,
                condicao_operadores: 6,
                linhas_classe: 1000,
                parametros: 8,
                funcoes_por_classe: 20,
                retornos: 4,
            },
            Perfil::Rigoroso => Self {
                linhas_funcao: 30,
                complexidade_cognitiva: 10,
                complexidade_ciclomatica: 10,
                aninhamento: 3,
                condicao_operadores: 3,
                linhas_classe: 400,
                parametros: 5,
                funcoes_por_classe: 11,
                retornos: 2,
            },
        }
    }
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
    /// Imports e tipos proibidos (PRB001, PRB002).
    pub proibir: Proibir,
    /// Pasta (relativa à raiz) com regras extras em YAML, uma por arquivo, escritas como
    /// consulta do tree-sitter.
    pub personalizadas: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Proibir {
    /// PRB001: imports proibidos. `a.b.C` (exato) ou `a.b.**` (o pacote e os de dentro).
    pub imports: Vec<String>,
    /// PRB002: tipos proibidos em pacotes (padrão ArchUnit).
    pub tipos_em_pacote: Vec<TipoEmPacote>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct TipoEmPacote {
    /// Nome simples do tipo (ex.: `Double`, `double`).
    pub tipo: String,
    /// Padrão de pacote (ex.: `..calculo..`).
    pub pacote: String,
    /// O que usar no lugar.
    pub dica: Option<String>,
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

/// Lê um YAML validando-o sozinho primeiro, para o erro apontar arquivo e linha.
fn ler_valor(texto: &str, nome: &str) -> Result<serde_yaml::Value> {
    if texto.trim().is_empty() {
        return Ok(serde_yaml::Value::Mapping(Default::default()));
    }
    let erro = |e: serde_yaml::Error| {
        let linha = e.location().map(|l| l.line()).unwrap_or(0);
        anyhow::anyhow!("{nome}:{linha} {e}")
    };
    serde_yaml::from_str::<Config>(texto).map_err(erro)?;
    serde_yaml::from_str(texto).map_err(erro)
}

/// Lê o guarda.yml e a cadeia de `estende`, cada camada sobrescrevendo a de baixo.
fn montar(caminho: &Path, profundidade: usize) -> Result<serde_yaml::Value> {
    if profundidade > 5 {
        bail!("estende encadeado demais (ciclo?) em {}", caminho.display());
    }
    let texto = std::fs::read_to_string(caminho)
        .with_context(|| format!("não consegui ler {}", caminho.display()))?;
    let nome = caminho
        .file_name()
        .map_or(ARQUIVO.into(), |n| n.to_string_lossy().to_string());
    let v = ler_valor(&texto, &nome)?;
    match v.get("estende").and_then(|e| e.as_str()) {
        Some(base) => {
            let dir = caminho.parent().unwrap_or(Path::new("."));
            Ok(mesclar(montar(&dir.join(base), profundidade + 1)?, v))
        }
        None => Ok(v),
    }
}

/// `cima` sobrescreve `base`; mapas são mesclados chave a chave, o resto é trocado.
fn mesclar(base: serde_yaml::Value, cima: serde_yaml::Value) -> serde_yaml::Value {
    use serde_yaml::Value;
    match (base, cima) {
        (Value::Mapping(mut b), Value::Mapping(c)) => {
            for (k, vc) in c {
                let novo = match b.remove(&k) {
                    Some(vb) => mesclar(vb, vc),
                    None => vc,
                };
                b.insert(k, novo);
            }
            Value::Mapping(b)
        }
        (b, Value::Null) => b,
        (_, c) => c,
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
        Self::de_valor(montar(&caminho, 0)?)
    }

    /// Só o texto, sem `estende` (usado pelo `init` e nos testes).
    pub fn de_texto(texto: &str) -> Result<Self> {
        Self::de_valor(ler_valor(texto, ARQUIVO)?)
    }

    /// Aplica o perfil aos limites não declarados e monta a configuração.
    fn de_valor(mut v: serde_yaml::Value) -> Result<Self> {
        let perfil: Perfil = match v.get("perfil") {
            Some(p) => serde_yaml::from_value(p.clone()).context("guarda.yml perfil")?,
            None => Perfil::default(),
        };
        let do_perfil = serde_yaml::to_value(Limites::do_perfil(perfil))?;
        let declarados = v.get("limites").cloned().unwrap_or(serde_yaml::Value::Null);
        if let serde_yaml::Value::Mapping(m) = &mut v {
            m.insert("limites".into(), mesclar(do_perfil, declarados));
        }
        serde_yaml::from_value(v).context("guarda.yml")
    }

    pub fn perfil(&self) -> Perfil {
        self.perfil.unwrap_or_default()
    }

    /// Severidade efetiva: a do guarda.yml; senão, no perfil rigoroso, a severidade alvo.
    pub fn severidade(&self, codigo: &str, padrao: Severidade) -> Severidade {
        if let Some(s) = self.regras.severidade.get(codigo) {
            return *s;
        }
        match (self.perfil(), modelo::regra(codigo)) {
            // A mais severa das duas (Erro < Aviso na ordem do enum).
            (Perfil::Rigoroso, Some(r)) => padrao.min(r.alvo),
            _ => padrao,
        }
    }

    /// Regra desligada: pelo guarda.yml ou, no perfil mínimo, toda regra que hoje é aviso
    /// (menos a GRD001, que protege a própria configuração).
    pub fn desligada(&self, codigo: &str) -> bool {
        if self.regras.desligar.iter().any(|d| d == codigo) {
            return true;
        }
        self.perfil() == Perfil::Minimo
            && codigo != "GRD001"
            && !self.regras.severidade.contains_key(codigo)
            && modelo::regra(codigo).is_some_and(|r| r.severidade == Severidade::Aviso)
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
        // A identidade do executável muda a cada build: regra alterada nunca lê cache velho.
        let binario = std::env::current_exe()
            .and_then(std::fs::metadata)
            .map(|m| {
                let t = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_nanos());
                format!("{}:{t}", m.len())
            })
            .unwrap_or_default();
        format!(
            "{}|{binario}|{:?}|{:?}|{:?}|{:?}|{:?}",
            env!("CARGO_PKG_VERSION"),
            self.limites,
            self.arquitetura.frameworks,
            self.regras.proibir,
            self.regras.personalizadas,
            self.fontes
        )
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn estende_e_perfil() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("base.yml"),
            "perfil: rigoroso\nregras:\n  desligar: [SML003]\nlimites:\n  parametros: 9\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("guarda.yml"),
            "estende: base.yml\nlimites:\n  retornos: 5\n",
        )
        .unwrap();
        let c = Config::carregar(dir.path()).unwrap();
        assert_eq!(c.perfil(), Perfil::Rigoroso);
        assert_eq!(c.limites.parametros, 9, "da base");
        assert_eq!(c.limites.retornos, 5, "do projeto");
        assert_eq!(c.limites.linhas_funcao, 30, "do perfil");
        assert!(c.desligada("SML003"));
        assert_eq!(c.severidade("SML004", Severidade::Aviso), Severidade::Erro);
        assert_eq!(c.severidade("SML002", Severidade::Erro), Severidade::Erro);
    }

    #[test]
    fn perfil_minimo_desliga_os_avisos() {
        let c = Config::de_texto("perfil: minimo").unwrap();
        assert!(c.desligada("SML003"));
        assert!(!c.desligada("KT001"));
        assert!(!c.desligada("GRD001"));
        assert_eq!(c.limites.linhas_funcao, 100);
    }

    #[test]
    fn erro_na_base_aponta_o_arquivo_dela() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("time.yml"), "versao: 1\nlimite: 3\n").unwrap();
        std::fs::write(dir.path().join("guarda.yml"), "estende: time.yml\n").unwrap();
        let e = Config::carregar(dir.path()).unwrap_err().to_string();
        assert!(e.starts_with("time.yml:2"), "{e}");
    }

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
