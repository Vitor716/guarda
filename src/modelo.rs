//! Tipos centrais: violação, severidade e o catálogo de regras implementadas.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Severidade {
    Erro,
    Aviso,
}

pub struct Regra {
    pub codigo: &'static str,
    pub nome: &'static str,
    /// Severidade de hoje. Regra nova entra como aviso até ser medida sem falso positivo.
    pub severidade: Severidade,
    /// Severidade que a regra terá depois de validada em projeto real.
    pub alvo: Severidade,
    /// Regra de limite: o baseline guarda o valor e só aceita que ele diminua (catraca).
    pub catraca: bool,
}

use Severidade::{Aviso, Erro};

const fn r(
    codigo: &'static str,
    nome: &'static str,
    severidade: Severidade,
    alvo: Severidade,
    catraca: bool,
) -> Regra {
    Regra {
        codigo,
        nome,
        severidade,
        alvo,
        catraca,
    }
}

/// Catálogo. Códigos estáveis: nunca reaproveite um código removido.
pub const REGRAS: &[Regra] = &[
    r("ERR000", "arquivo que o parser não leu", Erro, Erro, false),
    r(
        "ARQ001",
        "dependência proibida entre camadas",
        Erro,
        Erro,
        false,
    ),
    r(
        "ARQ002",
        "ciclo entre fatias do projeto",
        Aviso,
        Erro,
        false,
    ),
    r("ARQ003", "framework dentro do domínio", Aviso, Erro, false),
    r(
        "ARQ004",
        "controlador fora da camada de entrada",
        Aviso,
        Erro,
        false,
    ),
    r("CPX001", "complexidade cognitiva", Erro, Erro, true),
    r("CPX002", "complexidade ciclomática", Aviso, Erro, true),
    r("CPX003", "aninhamento profundo", Aviso, Erro, true),
    r(
        "CPX004",
        "condição com muitos operadores",
        Aviso,
        Erro,
        false,
    ),
    r("CPX005", "função longa", Erro, Erro, true),
    r("CPX006", "classe longa", Aviso, Erro, true),
    r("CPX007", "muitos parâmetros", Aviso, Erro, true),
    r("CPX008", "muitas funções na classe", Aviso, Erro, true),
    r("CPX009", "muitos return", Aviso, Erro, true),
    r(
        "SML001",
        "God Class (aproximação sintática)",
        Aviso,
        Aviso,
        false,
    ),
    r("SML002", "código duplicado", Aviso, Aviso, false),
    r("SML003", "número mágico", Aviso, Aviso, false),
    r("SML004", "catch genérico", Aviso, Erro, false),
    r(
        "SML005",
        "exceção engolida ou catch vazio",
        Aviso,
        Erro,
        false,
    ),
    r("SML006", "lançar exceção genérica", Aviso, Erro, false),
    r("SML007", "código comentado", Aviso, Aviso, false),
    r("SML008", "membro privado não usado", Aviso, Aviso, false),
    r("SML009", "TODO/FIXME sem ticket", Aviso, Aviso, false),
    r("KT001", "uso de !!", Erro, Erro, false),
    r("KT002", "GlobalScope.launch/async", Aviso, Erro, false),
    r("KT003", "Thread.sleep em corrotina", Aviso, Erro, false),
    r("KT004", "suspend fun que retorna Flow", Aviso, Erro, false),
    r(
        "KT005",
        "Dispatchers fixo em vez de injetado",
        Aviso,
        Aviso,
        false,
    ),
    r("KT006", "data class com var", Aviso, Aviso, false),
    r(
        "KT007",
        "println/printStackTrace em produção",
        Aviso,
        Erro,
        false,
    ),
    r(
        "JV001",
        "System.out/printStackTrace em produção",
        Aviso,
        Erro,
        false,
    ),
    r(
        "JV002",
        "recurso sem try-with-resources",
        Aviso,
        Erro,
        false,
    ),
    r(
        "JV003",
        "equals sem hashCode (ou o contrário)",
        Aviso,
        Erro,
        false,
    ),
    r(
        "JV004",
        "Optional como campo ou parâmetro",
        Aviso,
        Aviso,
        false,
    ),
    r("JV005", "java.util.Date/Calendar", Aviso, Aviso, false),
    r(
        "CND001",
        "if envolve o caminho principal (guard clause)",
        Aviso,
        Aviso,
        false,
    ),
    r("CND002", "else depois de return/throw", Aviso, Aviso, false),
    r("CND003", "condição mistura && e ||", Aviso, Aviso, false),
    r("POL001", "cadeia de instanceof/is", Aviso, Aviso, false),
    r(
        "POL002",
        "switch/when repetido sobre o mesmo valor",
        Aviso,
        Aviso,
        false,
    ),
    r(
        "NEG001",
        "exceção HTTP na camada de negócio",
        Aviso,
        Erro,
        false,
    ),
    r(
        "NEG002",
        "regra de negócio com exceção genérica",
        Aviso,
        Aviso,
        false,
    ),
    r(
        "SEC001",
        "credencial fixa no código ou na configuração",
        Aviso,
        Erro,
        false,
    ),
    r("SEC002", "SQL montado com concatenação", Aviso, Erro, false),
    r(
        "SEC003",
        "comando do sistema com parte variável",
        Aviso,
        Erro,
        false,
    ),
    r(
        "SEC004",
        "hash fraco ou senha sem hash adaptativo",
        Aviso,
        Erro,
        false,
    ),
    r("SEC005", "cifra fraca ou modo inseguro", Aviso, Erro, false),
    r(
        "SEC006",
        "gerador aleatório previsível para valor secreto",
        Aviso,
        Aviso,
        false,
    ),
    r(
        "SEC007",
        "verificação de TLS desligada ou protocolo fraco",
        Aviso,
        Erro,
        false,
    ),
    r(
        "SEC008",
        "CORS liberado para qualquer origem",
        Aviso,
        Aviso,
        false,
    ),
    r("SEC009", "desserialização insegura", Aviso, Erro, false),
    r(
        "SEC010",
        "parser XML sem proteção contra XXE",
        Aviso,
        Erro,
        false,
    ),
    r(
        "SEC011",
        "proteção CSRF desligada numa aplicação com sessão",
        Aviso,
        Aviso,
        false,
    ),
    r(
        "SEC012",
        "rota liberada sem autenticação ampla ou sensível",
        Aviso,
        Aviso,
        false,
    ),
    r(
        "SEC013",
        "cookie sem Secure ou HttpOnly",
        Aviso,
        Aviso,
        false,
    ),
    r(
        "SEC014",
        "JWT lido sem verificar a assinatura",
        Aviso,
        Erro,
        false,
    ),
    r("SEC015", "log com dado sensível", Aviso, Aviso, false),
    r("SEC016", "TODO/FIXME de segurança", Aviso, Aviso, false),
    r(
        "SEC017",
        "configuração insegura (debug, actuator, console)",
        Aviso,
        Erro,
        false,
    ),
    r(
        "SEC018",
        "arquivo de segredo fora do .gitignore",
        Aviso,
        Erro,
        false,
    ),
    r("SEC019", "IP fixo no código", Aviso, Aviso, false),
    r(
        "SEC020",
        "entrada do request chega a um ponto perigoso",
        Aviso,
        Erro,
        false,
    ),
    r(
        "SPR001",
        "injeção de dependência por campo",
        Aviso,
        Aviso,
        false,
    ),
    r(
        "SPR002",
        "anotação de proxy em método private",
        Aviso,
        Erro,
        false,
    ),
    r(
        "SPR003",
        "chamada interna a método com anotação de proxy",
        Aviso,
        Erro,
        false,
    ),
    r(
        "SPR004",
        "entidade JPA recebida direto no @RequestBody",
        Aviso,
        Erro,
        false,
    ),
    r(
        "SPR005",
        "@RequestMapping sem método HTTP",
        Aviso,
        Aviso,
        false,
    ),
    r(
        "BUG001",
        "return ou throw dentro de finally",
        Aviso,
        Erro,
        false,
    ),
    r(
        "BUG002",
        "mesma expressão dos dois lados do operador",
        Aviso,
        Erro,
        false,
    ),
    r(
        "BUG003",
        "variável atribuída a ela mesma",
        Aviso,
        Erro,
        false,
    ),
    r(
        "BUG004",
        "condição repetida na cadeia if/when",
        Aviso,
        Erro,
        false,
    ),
    r("BUG005", "ramos com o mesmo código", Aviso, Erro, false),
    r(
        "BUG006",
        "String comparada com == no Java",
        Aviso,
        Erro,
        false,
    ),
    r(
        "BUG007",
        "BigDecimal criado a partir de double",
        Aviso,
        Erro,
        false,
    ),
    r(
        "BUG008",
        "InterruptedException ignorada",
        Aviso,
        Erro,
        false,
    ),
    r("SML010", "bloco vazio", Aviso, Aviso, false),
    r(
        "SML011",
        "exceção relançada sem a causa",
        Aviso,
        Aviso,
        false,
    ),
    r(
        "CND004",
        "if dentro de if que dá para juntar",
        Aviso,
        Aviso,
        false,
    ),
    r(
        "KT008",
        "MutableStateFlow/MutableLiveData exposto",
        Aviso,
        Aviso,
        false,
    ),
    r("JV006", "literal à direita no equals", Aviso, Aviso, false),
    r("TST001", "teste sem asserção", Aviso, Erro, false),
    r("TST002", "Thread.sleep em teste", Aviso, Aviso, false),
    r("TST003", "teste desativado sem motivo", Aviso, Aviso, false),
    r(
        "PRB001",
        "import proibido pelo guarda.yml",
        Erro,
        Erro,
        false,
    ),
    r(
        "PRB002",
        "tipo proibido no pacote pelo guarda.yml",
        Erro,
        Erro,
        false,
    ),
    r("SUP001", "supressão sem motivo", Erro, Erro, false),
    r(
        "COV000",
        "testes falharam ao gerar a cobertura",
        Erro,
        Erro,
        false,
    ),
    r("COV001", "cobertura do código novo", Erro, Erro, false),
    r("COV002", "cobertura por pacote", Erro, Erro, true),
    r("COV003", "cobertura geral", Erro, Erro, true),
    r(
        "GRD001",
        "guarda.yml ou baseline alterado (revisão humana)",
        Aviso,
        Aviso,
        false,
    ),
];

/// Código com vida estática: os do catálogo direto; os das regras personalizadas, guardados
/// uma vez só (o servidor MCP roda por muito tempo e não pode vazar a cada verificação).
pub fn interno(codigo: &str) -> &'static str {
    use std::collections::HashSet;
    use std::sync::{LazyLock, Mutex};
    static CODIGOS: LazyLock<Mutex<HashSet<&'static str>>> = LazyLock::new(Default::default);
    if let Some(r) = regra(codigo) {
        return r.codigo;
    }
    let mut c = CODIGOS.lock().expect("códigos");
    if let Some(x) = c.get(codigo) {
        return x;
    }
    let x: &'static str = Box::leak(codigo.to_string().into_boxed_str());
    c.insert(x);
    x
}

pub fn regra(codigo: &str) -> Option<&'static Regra> {
    REGRAS.iter().find(|r| r.codigo == codigo)
}

#[derive(Clone, Debug, Serialize)]
pub struct Violacao {
    pub arquivo: String,
    pub linha: usize,
    pub codigo: &'static str,
    pub severidade: Severidade,
    pub mensagem: String,
    pub dica: String,
    /// Símbolo dono (`Classe.metodo`): parte da impressão digital do baseline.
    pub simbolo: String,
    /// Trecho normalizado: parte da impressão digital do baseline.
    #[serde(skip)]
    pub trecho: String,
    /// Valor medido, nas regras de limite (catraca).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valor: Option<u32>,
}

impl Violacao {
    #[allow(clippy::too_many_arguments)]
    pub fn nova(
        codigo: &'static str,
        arquivo: &str,
        linha: usize,
        mensagem: impl Into<String>,
        dica: impl Into<String>,
        simbolo: impl Into<String>,
        trecho: &str,
        valor: Option<u32>,
    ) -> Self {
        Self {
            arquivo: arquivo.to_string(),
            linha,
            codigo,
            severidade: regra(codigo).map_or(Severidade::Erro, |r| r.severidade),
            mensagem: mensagem.into(),
            dica: dica.into(),
            simbolo: simbolo.into(),
            trecho: trecho.split_whitespace().collect::<Vec<_>>().join(" "),
            valor,
        }
    }

    /// Ordem determinística da saída: arquivo, severidade, linha, código, mensagem.
    pub fn chave_ordem(&self) -> (&str, Severidade, usize, &str, &str) {
        (
            &self.arquivo,
            self.severidade,
            self.linha,
            self.codigo,
            &self.mensagem,
        )
    }
}
