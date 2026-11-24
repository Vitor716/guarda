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
    /// Por que a dívida foi aceita (`guarda baseline aceitar --motivo`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motivo: Option<String>,
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
        motivo: None,
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

    /// Regrava o baseline a partir das violações atuais, preservando os `motivo`s já aceitos.
    /// Se `so_regras` for informado, atualiza só essas regras e mantém as demais intactas
    /// (útil ao ativar regras novas sem congelar dívida nova das regras antigas).
    pub fn regerar(&self, vs: &[Violacao], so_regras: Option<&[String]>) -> (Self, Regeracao) {
        let filtro: Option<HashSet<&str>> =
            so_regras.map(|rs| rs.iter().map(String::as_str).collect());
        let no_alvo = |codigo: &str| filtro.as_ref().is_none_or(|f| f.contains(codigo));

        let mut anteriores: BTreeMap<Chave, Vec<Option<String>>> = BTreeMap::new();
        let mut mantidas_fora = Vec::new();
        let mut alvo_antes = 0usize;
        for e in &self.violacoes {
            if no_alvo(&e.codigo) {
                alvo_antes += 1;
                anteriores
                    .entry(chave(e))
                    .or_default()
                    .push(e.motivo.clone());
            } else {
                mantidas_fora.push(e.clone());
            }
        }
        // Prioriza reaproveitar entradas com motivo antes das sem motivo.
        for lista in anteriores.values_mut() {
            lista.sort_by_key(|m| m.is_some());
        }

        let mut novas_entradas = Vec::new();
        let mut novas = 0usize;
        let mut motivos_preservados = 0usize;
        for v in vs {
            if !no_alvo(v.codigo) {
                continue;
            }
            let mut e = entrada(v);
            match anteriores.get_mut(&chave(&e)).and_then(|l| l.pop()) {
                Some(motivo) => {
                    if motivo.is_some() {
                        motivos_preservados += 1;
                    }
                    e.motivo = motivo;
                }
                None => novas += 1,
            }
            novas_entradas.push(e);
        }
        let removidas: usize = anteriores.values().map(Vec::len).sum();
        debug_assert_eq!(
            alvo_antes + novas - removidas,
            novas_entradas.len(),
            "contabilidade da regeração"
        );
        novas_entradas.extend(mantidas_fora);
        novas_entradas.sort();
        (
            Self {
                versao: 1,
                violacoes: novas_entradas,
            },
            Regeracao {
                novas,
                removidas,
                motivos_preservados,
            },
        )
    }

    /// Acrescenta as violações que ainda não estão congeladas (ou sobe a catraca quando ela
    /// cresceu), com o motivo.
    pub fn aceitar(&mut self, vs: &[Violacao], motivo: &str) -> Aceite {
        if self.versao == 0 {
            self.versao = 1;
        }
        let mut por_chave: BTreeMap<Chave, Vec<usize>> = BTreeMap::new();
        for (idx, e) in self.violacoes.iter().enumerate() {
            por_chave.entry(chave(e)).or_default().push(idx);
        }
        let mut res = Aceite::default();
        let mut novas_entradas = Vec::new();
        for v in vs {
            let e = entrada(v);
            let k = chave(&e);
            if catraca(v.codigo) {
                if let Some(indices) = por_chave.get_mut(&k)
                    && !indices.is_empty()
                {
                    let atual = e.valor.unwrap_or(0);
                    // Se já há uma entrada cobrindo o valor atual, reaproveita-a.
                    if let Some(pos) = indices
                        .iter()
                        .position(|&i| self.violacoes[i].valor.is_none_or(|a| atual <= a))
                    {
                        indices.remove(pos);
                        res.ja_congeladas += 1;
                    } else {
                        // A catraca cresceu: atualiza o maior valor existente com o novo limite e motivo.
                        let idx = indices.pop().expect("não vazio");
                        self.violacoes[idx].valor = e.valor;
                        self.violacoes[idx].motivo = Some(motivo.to_string());
                        res.atualizadas += 1;
                    }
                    continue;
                }
            } else if let Some(indices) = por_chave.get_mut(&k)
                && !indices.is_empty()
            {
                indices.pop();
                res.ja_congeladas += 1;
                continue;
            }
            novas_entradas.push(Entrada {
                motivo: Some(motivo.to_string()),
                ..e
            });
            res.novas += 1;
        }
        self.violacoes.extend(novas_entradas);
        self.violacoes.sort();
        res
    }

    pub fn regras(&self) -> usize {
        self.violacoes
            .iter()
            .map(|e| e.codigo.as_str())
            .collect::<HashSet<_>>()
            .len()
    }

    pub fn com_motivo(&self) -> usize {
        self.violacoes.iter().filter(|e| e.motivo.is_some()).count()
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Regeracao {
    pub novas: usize,
    pub removidas: usize,
    pub motivos_preservados: usize,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Aceite {
    pub novas: usize,
    pub atualizadas: usize,
    pub ja_congeladas: usize,
}

pub struct Comparacao {
    pub novas: Vec<Violacao>,
    pub congeladas: usize,
    /// Entradas que continuam valendo, com a catraca já apertada (para `--atualizar-baseline`).
    pub mantidas: Vec<Entrada>,
    /// Entradas do baseline que não aparecem mais (no escopo analisado ou em arquivos removidos).
    pub corrigidas: usize,
    /// Entradas de catraca cujo valor atual ficou menor que o congelado.
    pub catracas_apertadas: usize,
    /// Entradas removidas porque o arquivo deixou de existir (só em `escopo_total`).
    pub arquivos_removidos: usize,
}

type Chave = (String, String, String, String);
/// O que a comparação reaproveita de uma entrada congelada: valor da catraca e motivo.
type Congelada = (Option<u32>, Option<String>);

fn chave(e: &Entrada) -> Chave {
    (
        e.codigo.clone(),
        e.arquivo.clone(),
        e.simbolo.clone(),
        e.hash.clone(),
    )
}

/// Compara as violações atuais com o baseline. `analisados` limita a conta de corrigidas
/// aos arquivos que de fato foram verificados (modo `--arquivos`/`--alterados`).
/// Em `escopo_total` (projeto inteiro), entradas de arquivos que não existem mais também
/// são contadas como corrigidas e removidas em `mantidas`.
pub fn comparar(
    b: &Baseline,
    atuais: Vec<Violacao>,
    analisados: &HashSet<&str>,
    escopo_total: bool,
) -> Comparacao {
    let mut disponiveis: BTreeMap<Chave, Vec<Congelada>> = BTreeMap::new();
    for e in &b.violacoes {
        disponiveis
            .entry(chave(e))
            .or_default()
            .push((e.valor, e.motivo.clone()));
    }
    for lista in disponiveis.values_mut() {
        lista.sort_by_key(|(v, _)| *v);
    }

    let mut por_chave_atual: BTreeMap<Chave, Vec<(Violacao, Entrada)>> = BTreeMap::new();
    for v in atuais {
        let e = entrada(&v);
        por_chave_atual.entry(chave(&e)).or_default().push((v, e));
    }

    let mut novas = Vec::new();
    let mut mantidas = Vec::new();
    let mut congeladas = 0;
    let mut catracas_apertadas = 0;

    for (k, mut itens) in por_chave_atual {
        let eh_catraca = catraca(&k.0);
        if eh_catraca {
            // Pareamento determinístico do maior valor atual para o menor:
            // evita falso positivo quando há sobrecarga com mesma chave (ex.: dois métodos `buscar`).
            itens.sort_by(|a, b| {
                b.1.valor
                    .cmp(&a.1.valor)
                    .then_with(|| a.0.chave_ordem().cmp(&b.0.chave_ordem()))
            });
            let mut excedentes: Vec<(Violacao, Entrada)> = Vec::new();
            let mut fila = disponiveis.get_mut(&k);
            for (v, e) in itens {
                let atual = e.valor;
                let casou = fila.as_mut().and_then(|f| {
                    // `f` está ordenado crescente: se o maior disponível cobre `atual`, consome-o.
                    let ultimo = f.last()?;
                    let ok = match (ultimo.0, atual) {
                        (Some(antigo), Some(cur)) => cur <= antigo,
                        _ => true,
                    };
                    if ok { f.pop() } else { None }
                });
                match casou {
                    Some((valor_antigo, motivo)) => {
                        if let (Some(antigo), Some(cur)) = (valor_antigo, atual)
                            && cur < antigo
                        {
                            catracas_apertadas += 1;
                        }
                        congeladas += 1;
                        mantidas.push(Entrada { motivo, ..e });
                    }
                    None => excedentes.push((v, e)),
                }
            }
            for (mut v, e) in excedentes {
                let sobra = disponiveis.get_mut(&k).and_then(|f| f.pop());
                match sobra {
                    Some((Some(antigo), motivo)) => {
                        v.mensagem = format!("{} (baseline {antigo})", v.mensagem);
                        novas.push(v);
                        mantidas.push(Entrada {
                            valor: Some(antigo),
                            motivo,
                            ..e
                        });
                    }
                    Some((None, motivo)) => {
                        congeladas += 1;
                        mantidas.push(Entrada { motivo, ..e });
                    }
                    None => novas.push(v),
                }
            }
        } else {
            for (v, e) in itens {
                let fila = disponiveis.get_mut(&k);
                match fila.and_then(|f| f.pop()) {
                    Some((_, motivo)) => {
                        congeladas += 1;
                        mantidas.push(Entrada { motivo, ..e });
                    }
                    None => novas.push(v),
                }
            }
        }
    }

    let mut corrigidas = 0;
    let mut arquivos_removidos = 0;
    for ((_, arquivo, _, _), sobra) in &disponiveis {
        if sobra.is_empty() {
            continue;
        }
        if analisados.contains(arquivo.as_str()) {
            corrigidas += sobra.len();
        } else if escopo_total {
            corrigidas += sobra.len();
            if !arquivo.starts_with('[') {
                arquivos_removidos += sobra.len();
            }
        }
    }
    if !escopo_total {
        // Arquivos fora do escopo desta execução: as entradas ficam como estão.
        mantidas.extend(
            b.violacoes
                .iter()
                .filter(|e| !analisados.contains(e.arquivo.as_str()))
                .cloned(),
        );
    }
    mantidas.sort();
    novas.sort_by(|a, b| a.chave_ordem().cmp(&b.chave_ordem()));
    Comparacao {
        novas,
        congeladas,
        mantidas,
        corrigidas,
        catracas_apertadas,
        arquivos_removidos,
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
        let c = comparar(&b, vec![atual], &analisados, false);
        assert!(c.novas.is_empty());
        assert_eq!(c.congeladas, 1);
    }

    #[test]
    fn catraca_so_aperta() {
        let b = Baseline::de_violacoes(&[v("CPX005", "A.f", "", Some(80))]);
        let analisados = HashSet::from(["A.kt"]);
        let cresceu = comparar(
            &b,
            vec![v("CPX005", "A.f", "", Some(81))],
            &analisados,
            false,
        );
        assert_eq!(cresceu.novas.len(), 1);
        assert!(cresceu.novas[0].mensagem.contains("baseline 80"));
        let diminuiu = comparar(
            &b,
            vec![v("CPX005", "A.f", "", Some(70))],
            &analisados,
            false,
        );
        assert!(diminuiu.novas.is_empty());
        assert_eq!(diminuiu.mantidas[0].valor, Some(70));
        assert_eq!(diminuiu.catracas_apertadas, 1);
    }

    #[test]
    fn catraca_com_sobrecarga_nao_gera_falso_positivo() {
        // Dois métodos sobrecarregados `A.buscar`: um com CPX 18 e outro com CPX 25.
        let b = Baseline::de_violacoes(&[
            v("CPX001", "A.buscar", "", Some(18)),
            v("CPX001", "A.buscar", "", Some(25)),
        ]);
        let analisados = HashSet::from(["A.kt"]);
        // Ordem invertida nas linhas: 18 aparece antes de 25.
        let igual = comparar(
            &b,
            vec![
                v("CPX001", "A.buscar", "", Some(18)),
                v("CPX001", "A.buscar", "", Some(25)),
            ],
            &analisados,
            true,
        );
        assert!(igual.novas.is_empty(), "{:?}", igual.novas);
        assert_eq!(igual.congeladas, 2);

        // O menor cresceu (18 -> 20) e o maior ficou em 25: cobra só o 20 contra baseline 18.
        let cresceu = comparar(
            &b,
            vec![
                v("CPX001", "A.buscar", "", Some(20)),
                v("CPX001", "A.buscar", "", Some(25)),
            ],
            &analisados,
            true,
        );
        assert_eq!(cresceu.novas.len(), 1);
        assert!(cresceu.novas[0].mensagem.contains("baseline 18"));
    }

    #[test]
    fn aceitar_atualiza_catraca_que_cresceu() {
        let mut b = Baseline::de_violacoes(&[v("CPX001", "A.f", "", Some(18))]);
        let r = b.aceitar(
            &[v("CPX001", "A.f", "", Some(22))],
            "complexidade aceita temporariamente",
        );
        assert_eq!(
            r,
            Aceite {
                novas: 0,
                atualizadas: 1,
                ja_congeladas: 0
            }
        );
        assert_eq!(b.violacoes.len(), 1);
        assert_eq!(b.violacoes[0].valor, Some(22));
        assert_eq!(
            b.violacoes[0].motivo.as_deref(),
            Some("complexidade aceita temporariamente")
        );
    }

    #[test]
    fn regerar_preserva_motivos_e_filtra_por_regras() {
        let mut b = Baseline::de_violacoes(&[v("KT001", "A.f", "a!!", None)]);
        b.aceitar(&[v("CPX001", "A.g", "", Some(20))], "motivo registrado");
        // Regerar só SEC001 não congela um KT001 novo!
        let (parcial, info) = b.regerar(
            &[
                v("KT001", "A.f", "a!!", None),
                v("KT001", "A.f", "b!!", None),
                v("SEC001", "A.h", "senha", None),
            ],
            Some(&["SEC001".to_string()]),
        );
        assert_eq!(info.novas, 1);
        assert_eq!(
            parcial
                .violacoes
                .iter()
                .filter(|e| e.codigo == "KT001")
                .count(),
            1
        );
        assert_eq!(
            parcial
                .violacoes
                .iter()
                .filter(|e| e.codigo == "SEC001")
                .count(),
            1
        );
        assert_eq!(parcial.com_motivo(), 1);

        // Regerar tudo preserva o motivo de CPX001.
        let (completo, info) = b.regerar(
            &[
                v("CPX001", "A.g", "", Some(20)),
                v("SEC001", "A.h", "senha", None),
            ],
            None,
        );
        assert_eq!(info.motivos_preservados, 1);
        assert_eq!(info.removidas, 1);
        assert_eq!(info.novas, 1);
        assert_eq!(completo.com_motivo(), 1);
    }

    #[test]
    fn arquivo_deletado_sai_no_escopo_total() {
        let b = Baseline::de_violacoes(&[v("KT001", "A.f", "a!!", None)]);
        let analisados = HashSet::from(["Outro.kt"]);
        let parcial = comparar(&b, vec![], &analisados, false);
        assert_eq!(parcial.corrigidas, 0);
        assert_eq!(parcial.mantidas.len(), 1);

        let total = comparar(&b, vec![], &analisados, true);
        assert_eq!(total.corrigidas, 1);
        assert_eq!(total.arquivos_removidos, 1);
        assert!(total.mantidas.is_empty());
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
            false,
        );
        assert_eq!(c.novas.len(), 1);
    }

    #[test]
    fn corrigida_sai_do_baseline() {
        let b = Baseline::de_violacoes(&[v("KT001", "A.f", "a!!", None)]);
        let analisados = HashSet::from(["A.kt"]);
        let c = comparar(&b, vec![], &analisados, false);
        assert_eq!(c.corrigidas, 1);
        assert!(c.mantidas.is_empty());
    }
}
