//! Tipos centrais: violação, severidade e o catálogo de regras implementadas.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severidade {
    Erro,
    Aviso,
}

impl Severidade {
    pub fn de_texto(s: &str) -> Option<Self> {
        match s {
            "erro" => Some(Self::Erro),
            "aviso" => Some(Self::Aviso),
            _ => None,
        }
    }
}

pub struct Regra {
    pub codigo: &'static str,
    pub nome: &'static str,
    pub severidade: Severidade,
    /// Regra de limite: o baseline guarda o valor e só aceita que ele diminua (catraca).
    pub catraca: bool,
}

/// Regras da Fase 0. Códigos estáveis: nunca reaproveite um código removido.
pub const REGRAS: &[Regra] = &[
    Regra {
        codigo: "ERR000",
        nome: "arquivo que o parser não leu",
        severidade: Severidade::Erro,
        catraca: false,
    },
    Regra {
        codigo: "ARQ001",
        nome: "dependência proibida entre camadas",
        severidade: Severidade::Erro,
        catraca: false,
    },
    Regra {
        codigo: "CPX001",
        nome: "complexidade cognitiva",
        severidade: Severidade::Erro,
        catraca: true,
    },
    Regra {
        codigo: "CPX005",
        nome: "função longa",
        severidade: Severidade::Erro,
        catraca: true,
    },
    Regra {
        codigo: "KT001",
        nome: "uso de !!",
        severidade: Severidade::Erro,
        catraca: false,
    },
    Regra {
        codigo: "SUP001",
        nome: "supressão sem motivo",
        severidade: Severidade::Erro,
        catraca: false,
    },
];

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
