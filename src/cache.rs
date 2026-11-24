//! Cache incremental em `.guarda/cache/`: o resultado das regras por arquivo, indexado pelo
//! conteúdo. Arquivo com mesmo tamanho e data não é nem lido; com data nova, é lido e
//! comparado pelo hash; só o que mudou passa pelo parser de novo.
//!
//! O cache nunca muda o resultado: a chave inclui a versão da guarda e os limites que
//! afetam as regras por arquivo. As regras de projeto (ARQ001) sempre rodam de novo.

use crate::lang::Import;
use crate::modelo::{Severidade, Violacao, regra};
use crate::regras::Supressao;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub const DIRETORIO: &str = ".guarda/cache";
const ARQUIVO: &str = "arquivos.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ViolacaoSalva {
    codigo: String,
    linha: usize,
    severidade: Severidade,
    mensagem: String,
    dica: String,
    simbolo: String,
    trecho: String,
    valor: Option<u32>,
}

impl ViolacaoSalva {
    pub fn de(v: &Violacao) -> Self {
        Self {
            codigo: v.codigo.to_string(),
            linha: v.linha,
            severidade: v.severidade,
            mensagem: v.mensagem.clone(),
            dica: v.dica.clone(),
            simbolo: v.simbolo.clone(),
            trecho: v.trecho.clone(),
            valor: v.valor,
        }
    }

    /// `None` se o código não existe mais nesta versão (o cache é descartado antes disso).
    pub fn para(&self, arquivo: &str) -> Option<Violacao> {
        Some(Violacao {
            arquivo: arquivo.to_string(),
            linha: self.linha,
            codigo: regra(&self.codigo)?.codigo,
            severidade: self.severidade,
            mensagem: self.mensagem.clone(),
            dica: self.dica.clone(),
            simbolo: self.simbolo.clone(),
            trecho: self.trecho.clone(),
            valor: self.valor,
        })
    }
}

/// O que as regras por arquivo produziram.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dados {
    pub pacote: Option<String>,
    pub imports: Vec<Import>,
    pub violacoes: Vec<ViolacaoSalva>,
    pub supressoes: Vec<Supressao>,
    pub erro_parse: bool,
    pub linhas: usize,
    pub tokens: Vec<crate::duplicacao::Token>,
    pub anotacoes: Vec<(String, usize)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entrada {
    pub tamanho: u64,
    pub modificado_ns: u128,
    pub hash: String,
    pub dados: Dados,
}

#[derive(Default, Serialize, Deserialize)]
struct Arquivo {
    impressao: String,
    entradas: BTreeMap<String, Entrada>,
}

pub struct Cache {
    caminho: Option<PathBuf>,
    impressao: String,
    entradas: BTreeMap<String, Entrada>,
    mudou: bool,
}

/// Tamanho e data de modificação, o atalho para nem abrir o arquivo.
pub fn assinatura(caminho: &Path) -> Option<(u64, u128)> {
    let m = std::fs::metadata(caminho).ok()?;
    let ns = m
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some((m.len(), ns))
}

pub fn hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex()[..32].to_string()
}

impl Cache {
    /// Cache desligado: nada é lido nem gravado.
    pub fn desligado() -> Self {
        Self {
            caminho: None,
            impressao: String::new(),
            entradas: BTreeMap::new(),
            mudou: false,
        }
    }

    pub fn carregar(raiz: &Path, impressao: &str) -> Self {
        let caminho = raiz.join(DIRETORIO).join(ARQUIVO);
        let entradas = std::fs::read_to_string(&caminho)
            .ok()
            .and_then(|t| serde_json::from_str::<Arquivo>(&t).ok())
            .filter(|a| a.impressao == impressao)
            .map(|a| a.entradas)
            .unwrap_or_default();
        Self {
            caminho: Some(caminho),
            impressao: impressao.to_string(),
            entradas,
            mudou: false,
        }
    }

    pub fn ligado(&self) -> bool {
        self.caminho.is_some()
    }

    pub fn get(&self, rel: &str) -> Option<&Entrada> {
        self.entradas.get(rel)
    }

    pub fn guardar(&mut self, rel: String, e: Entrada) {
        if self.caminho.is_some() {
            self.entradas.insert(rel, e);
            self.mudou = true;
        }
    }

    /// Tira do cache os arquivos que não existem mais.
    pub fn manter_so(&mut self, existentes: &std::collections::HashSet<&str>) {
        let antes = self.entradas.len();
        self.entradas.retain(|k, _| existentes.contains(k.as_str()));
        self.mudou |= self.entradas.len() != antes;
    }

    /// Grava se algo mudou. Falha de escrita não derruba a verificação.
    pub fn salvar(self) {
        let Some(caminho) = self.caminho else { return };
        if !self.mudou {
            return;
        }
        let Some(dir) = caminho.parent() else { return };
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
        // O cache nunca vai para o git, mesmo sem .gitignore no projeto.
        let _ = std::fs::write(dir.join(".gitignore"), "*\n");
        let a = Arquivo {
            impressao: self.impressao,
            entradas: self.entradas,
        };
        if let Ok(t) = serde_json::to_string(&a) {
            let tmp = caminho.with_extension("tmp");
            if std::fs::write(&tmp, t).is_ok() {
                let _ = std::fs::rename(&tmp, &caminho);
            }
        }
    }
}
