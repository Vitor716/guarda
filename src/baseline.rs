//! Baseline: congela o legado para só o código novo ser cobrado.
//!
//! - Impressão digital estável: código + arquivo + símbolo dono + hash do trecho normalizado
//!   (nada de número de linha: mover código não descongela nada).
//! - Catraca: nas regras de limite o baseline guarda o valor; crescer vira violação nova.
//! - Encolhe sozinho (`check --atualizar-baseline`); crescer só com `baseline gerar`.
//! - JSON ordenado, uma violação por linha, para o diff do PR ser legível.

use crate::modelo::{Violacao, regra};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Entrada {
    pub codigo: String,
    pub arquivo: String,
    pub simbolo: String,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valor: Option<u32>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Baseline {
    pub versao: u32,
    pub violacoes: Vec<Entrada>,
}

fn catraca(codigo: &str) -> bool {
    regra(codigo).is_some_and(|r| r.catraca)
}

pub fn entrada(v: &Violacao) -> Entrada {
    // Nas regras de catraca a identidade é o símbolo; o trecho muda quando a função cresce.
    let trecho = if catraca(v.codigo) {
        ""
    } else {
        v.trecho.as_str()
    };
    let h =
        blake3::hash(format!("{}\0{}\0{}\0{}", v.codigo, v.arquivo, v.simbolo, trecho).as_bytes());
    Entrada {
        codigo: v.codigo.to_string(),
        arquivo: v.arquivo.clone(),
        simbolo: v.simbolo.clone(),
        hash: h.to_hex()[..16].to_string(),
        valor: v.valor.filter(|_| catraca(v.codigo)),
    }
}

impl Baseline {
    pub fn de_violacoes(vs: &[Violacao]) -> Self {
        let mut violacoes: Vec<Entrada> = vs.iter().map(entrada).collect();
        violacoes.sort();
        Self {
            versao: 1,
            violacoes,
        }
    }

    pub fn carregar(caminho: &Path) -> Result<Option<Self>> {
        if !caminho.exists() {
            return Ok(None);
        }
        let t = std::fs::read_to_string(caminho)?;
        let b = serde_json::from_str(&t)
            .with_context(|| format!("baseline inválido: {}", caminho.display()))?;
        Ok(Some(b))
    }

    pub fn salvar(&self, caminho: &Path) -> Result<()> {
        if let Some(dir) = caminho.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut s = format!("{{\n  \"versao\": {},\n  \"violacoes\": [\n", self.versao);
        let n = self.violacoes.len();
        for (i, e) in self.violacoes.iter().enumerate() {
            s.push_str("    ");
            s.push_str(&serde_json::to_string(e)?);
            s.push_str(if i + 1 < n { ",\n" } else { "\n" });
        }
        s.push_str("  ]\n}\n");
        std::fs::write(caminho, s)?;
        Ok(())
    }

    pub fn regras(&self) -> usize {
        self.violacoes
            .iter()
            .map(|e| e.codigo.as_str())
            .collect::<HashSet<_>>()
            .len()
    }
}

pub struct Comparacao {
    pub novas: Vec<Violacao>,
    pub congeladas: usize,
    /// Entradas que continuam valendo, com a catraca já apertada (para `--atualizar-baseline`).
    pub mantidas: Vec<Entrada>,
    /// Entradas do baseline dos arquivos analisados que não aparecem mais.
    pub corrigidas: usize,
}

type Chave = (String, String, String, String);

fn chave(e: &Entrada) -> Chave {
    (
        e.codigo.clone(),
        e.arquivo.clone(),
        e.simbolo.clone(),
        e.hash.clone(),
    )
}

/// Compara as violações atuais com o baseline. `analisados` limita a conta de corrigidas
/// aos arquivos que de fato foram verificados (modo `--arquivos`).
pub fn comparar(b: &Baseline, atuais: Vec<Violacao>, analisados: &HashSet<&str>) -> Comparacao {
    let mut disponiveis: BTreeMap<Chave, Vec<Option<u32>>> = BTreeMap::new();
    for e in &b.violacoes {
        disponiveis.entry(chave(e)).or_default().push(e.valor);
    }
    let mut novas = Vec::new();
    let mut mantidas = Vec::new();
    let mut congeladas = 0;
    for mut v in atuais {
        let e = entrada(&v);
        let fila = disponiveis.get_mut(&chave(&e));
        match fila.and_then(|f| f.pop()) {
            Some(valor_antigo) => match (valor_antigo, e.valor) {
                (Some(antigo), Some(atual)) if atual > antigo => {
                    v.mensagem = format!("{} (baseline {antigo})", v.mensagem);
                    novas.push(v);
                    // A entrada antiga continua valendo até a função voltar ao limite antigo.
                    mantidas.push(Entrada {
                        valor: Some(antigo),
                        ..e
                    });
                }
                _ => {
                    congeladas += 1;
                    mantidas.push(e);
                }
            },
            None => novas.push(v),
        }
    }
    let corrigidas = disponiveis
        .iter()
        .filter(|((_, arquivo, _, _), _)| analisados.contains(arquivo.as_str()))
        .map(|(_, sobra)| sobra.len())
        .sum();
    // Arquivos fora do escopo desta execução: as entradas ficam como estão.
    mantidas.extend(
        b.violacoes
            .iter()
            .filter(|e| !analisados.contains(e.arquivo.as_str()))
            .cloned(),
    );
    mantidas.sort();
    Comparacao {
        novas,
        congeladas,
        mantidas,
        corrigidas,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn v(codigo: &'static str, simbolo: &str, trecho: &str, valor: Option<u32>) -> Violacao {
        Violacao::nova(codigo, "A.kt", 10, "m", "d", simbolo, trecho, valor)
    }

    #[test]
    fn mover_de_linha_nao_descongela() {
        let b = Baseline::de_violacoes(&[v("KT001", "A.f", "x!!.y", None)]);
        let mut atual = v("KT001", "A.f", "x!!.y", None);
        atual.linha = 99;
        let analisados = HashSet::from(["A.kt"]);
        let c = comparar(&b, vec![atual], &analisados);
        assert!(c.novas.is_empty());
        assert_eq!(c.congeladas, 1);
    }

    #[test]
    fn catraca_so_aperta() {
        let b = Baseline::de_violacoes(&[v("CPX005", "A.f", "", Some(80))]);
        let analisados = HashSet::from(["A.kt"]);
        let cresceu = comparar(&b, vec![v("CPX005", "A.f", "", Some(81))], &analisados);
        assert_eq!(cresceu.novas.len(), 1);
        assert!(cresceu.novas[0].mensagem.contains("baseline 80"));
        let diminuiu = comparar(&b, vec![v("CPX005", "A.f", "", Some(70))], &analisados);
        assert!(diminuiu.novas.is_empty());
        assert_eq!(diminuiu.mantidas[0].valor, Some(70));
    }

    #[test]
    fn duplicadas_contam_como_multiconjunto() {
        let b = Baseline::de_violacoes(&[v("KT001", "A.f", "a!!", None)]);
        let analisados = HashSet::from(["A.kt"]);
        let c = comparar(
            &b,
            vec![
                v("KT001", "A.f", "a!!", None),
                v("KT001", "A.f", "a!!", None),
            ],
            &analisados,
        );
        assert_eq!(c.novas.len(), 1);
    }

    #[test]
    fn corrigida_sai_do_baseline() {
        let b = Baseline::de_violacoes(&[v("KT001", "A.f", "a!!", None)]);
        let analisados = HashSet::from(["A.kt"]);
        let c = comparar(&b, vec![], &analisados);
        assert_eq!(c.corrigidas, 1);
        assert!(c.mantidas.is_empty());
    }
}
