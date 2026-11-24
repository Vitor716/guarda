//! `guarda hook <evento>`: lê o JSON do evento do Claude Code no stdin e responde no contrato
//! dos hooks. Exit 2 bloqueia e o stderr volta para a IA; exit 0 libera; exit 1 é erro que
//! não bloqueia (a guarda nunca prende a IA por bug próprio).
//!
//! - `proteger` (PreToolUse): recusa mexer em guarda.yml, .guarda/**, no plugin de cobertura
//!   do build e nos hooks da guarda no .claude/settings*.json; recusa `guarda baseline`/`init`.
//! - `pos-edicao` (PostToolUse): verifica só o arquivo editado.
//! - `final` (Stop): verifica o que mudou; segura a IA até `gate.tentativas-final` vezes.

use crate::baseline;
use crate::config::{self, Config};
use crate::gate::{self, Opcoes, Pedido};
use crate::lang::Linguagem;
use crate::modelo::Violacao;
use crate::saida;
use anyhow::{Result, bail};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

pub enum Resposta {
    Liberar,
    /// stdout com um objeto JSON (ex.: `systemMessage` para você), exit 0.
    LiberarComMensagem(String),
    /// exit 2: o texto vai para a IA pelo stderr.
    Bloquear(String),
}

pub fn executar(evento: &str) -> Result<Resposta> {
    let mut texto = String::new();
    std::io::stdin().read_to_string(&mut texto)?;
    // Pipes do PowerShell 5.1 podem prefixar um BOM.
    let entrada: Value = serde_json::from_str(texto.trim_start_matches('\u{feff}').trim())?;
    match evento {
        "proteger" => proteger(&entrada),
        "pos-edicao" => pos_edicao(&entrada),
        "final" => final_(&entrada),
        outro => bail!("evento desconhecido: {outro} (use proteger, pos-edicao ou final)"),
    }
}

fn texto_de<'a>(v: &'a Value, campo: &str) -> Option<&'a str> {
    v.get(campo).and_then(Value::as_str)
}

fn caminho_da_ferramenta(entrada: &Value) -> Option<PathBuf> {
    let ti = entrada.get("tool_input")?;
    let p = texto_de(ti, "file_path").or_else(|| texto_de(ti, "notebook_path"))?;
    let p = PathBuf::from(p);
    if p.is_absolute() {
        Some(p)
    } else {
        Some(PathBuf::from(texto_de(entrada, "cwd").unwrap_or(".")).join(p))
    }
}

/// Raiz do projeto: o diretório mais próximo, subindo, que tem um guarda.yml.
fn achar_raiz(inicio: &Path) -> Option<PathBuf> {
    let mut d = Some(inicio);
    while let Some(x) = d {
        if x.join(config::ARQUIVO).is_file() {
            return Some(x.to_path_buf());
        }
        d = x.parent();
    }
    None
}

fn raiz_da_sessao(entrada: &Value) -> Option<PathBuf> {
    let cwd = texto_de(entrada, "cwd").map(PathBuf::from);
    let projeto = std::env::var_os("CLAUDE_PROJECT_DIR").map(PathBuf::from);
    cwd.iter().chain(projeto.iter()).find_map(|p| achar_raiz(p))
}

fn relativo(raiz: &Path, caminho: &Path) -> Option<String> {
    let raiz = std::path::absolute(raiz).ok()?;
    let caminho = std::path::absolute(caminho).ok()?;
    let r = raiz.to_string_lossy().replace('\\', "/").to_lowercase();
    let c = caminho.to_string_lossy().replace('\\', "/");
    // Windows não diferencia maiúsculas no caminho.
    if !c.to_lowercase().starts_with(&r) {
        return None;
    }
    Some(c[r.len()..].trim_start_matches('/').to_string())
}

fn protegido(rel: &str) -> bool {
    let r = rel.to_lowercase();
    r == config::ARQUIVO || r == ".guarda" || r.starts_with(".guarda/")
}

/// Trecho sensível de um arquivo que a IA pode editar, mas não nessa parte.
fn agulha(rel: &str) -> Option<(&'static Regex, &'static str)> {
    static COBERTURA: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)jacoco|kover").expect("regex"));
    static HOOKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"guarda hook").expect("regex"));
    let nome = rel.rsplit('/').next().unwrap_or(rel).to_lowercase();
    match nome.as_str() {
        "pom.xml" | "build.gradle" | "build.gradle.kts" => {
            Some((&COBERTURA, "o plugin de cobertura do build é protegido"))
        }
        "settings.json" | "settings.local.json" if rel.to_lowercase().starts_with(".claude/") => {
            Some((&HOOKS, "os hooks da guarda são protegidos"))
        }
        _ => None,
    }
}

fn linhas_com(re: &Regex, t: &str) -> Vec<String> {
    t.lines()
        .filter(|l| re.is_match(l))
        .map(|l| l.trim().to_string())
        .collect()
}

/// A edição mexe em linhas com a agulha?
fn mexe_na_agulha(re: &Regex, ferramenta: &str, ti: &Value, caminho: &Path) -> bool {
    let toca = |s: Option<&str>| s.is_some_and(|s| re.is_match(s));
    match ferramenta {
        "Edit" => toca(texto_de(ti, "old_string")) || toca(texto_de(ti, "new_string")),
        "MultiEdit" => ti.get("edits").and_then(Value::as_array).is_some_and(|es| {
            es.iter()
                .any(|e| toca(texto_de(e, "old_string")) || toca(texto_de(e, "new_string")))
        }),
        "Write" => {
            let atual = std::fs::read_to_string(caminho).unwrap_or_default();
            let antes = linhas_com(re, &atual);
            !antes.is_empty() && antes != linhas_com(re, texto_de(ti, "content").unwrap_or(""))
        }
        _ => false,
    }
}

static GUARDA_PROIBIDO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bguarda(?:\.exe)?\s+(baseline|init)\b").expect("regex"));
static CITA_PROTEGIDO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)guarda\.ya?ml|\.guarda\b|\.claude[\\/]settings").expect("regex")
});
static MUTA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
        \b(rm|del|erase|rmdir|rd|mv|move|cp|copy|ren|rename|sed|perl|tee|truncate|touch|chmod|
           remove-item|move-item|copy-item|rename-item|set-content|add-content|out-file|
           clear-content|new-item|ni|sc|ac)\b
        | \bgit\s+(checkout|restore|reset|rm|mv|stash)\b
        | >",
    )
    .expect("regex")
});

/// Motivo para recusar o comando de shell, se houver.
pub fn comando_proibido(cmd: &str) -> Option<&'static str> {
    if let Some(c) = GUARDA_PROIBIDO.captures(cmd) {
        return Some(if c[1].eq_ignore_ascii_case("baseline") {
            "congelar dívida no baseline é decisão humana"
        } else {
            "guarda init reescreve a configuração"
        });
    }
    if CITA_PROTEGIDO.is_match(cmd) && MUTA.is_match(cmd) {
        return Some("guarda.yml, .guarda/ e os hooks são protegidos");
    }
    None
}

fn recusa(motivo: &str) -> Resposta {
    Resposta::Bloquear(format!(
        "guarda: {motivo}. Não contorne: siga com a tarefa sem mexer nisso ou peça a um humano."
    ))
}

fn proteger(entrada: &Value) -> Result<Resposta> {
    let ferramenta = texto_de(entrada, "tool_name").unwrap_or("");
    let ti = entrada.get("tool_input").cloned().unwrap_or(Value::Null);
    match ferramenta {
        "Bash" | "PowerShell" => {
            if raiz_da_sessao(entrada).is_none() {
                return Ok(Resposta::Liberar);
            }
            Ok(match texto_de(&ti, "command").and_then(comando_proibido) {
                Some(m) => recusa(m),
                None => Resposta::Liberar,
            })
        }
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => {
            let Some(caminho) = caminho_da_ferramenta(entrada) else {
                return Ok(Resposta::Liberar);
            };
            let inicio = caminho.parent().unwrap_or(&caminho);
            let Some(raiz) = achar_raiz(inicio).or_else(|| raiz_da_sessao(entrada)) else {
                return Ok(Resposta::Liberar);
            };
            let Some(rel) = relativo(&raiz, &caminho) else {
                return Ok(Resposta::Liberar);
            };
            if protegido(&rel) {
                return Ok(recusa(&format!("{rel} é protegido")));
            }
            if let Some((re, motivo)) = agulha(&rel)
                && mexe_na_agulha(re, ferramenta, &ti, &caminho)
            {
                return Ok(recusa(motivo));
            }
            Ok(Resposta::Liberar)
        }
        _ => Ok(Resposta::Liberar),
    }
}

fn pos_edicao(entrada: &Value) -> Result<Resposta> {
    let Some(caminho) = caminho_da_ferramenta(entrada) else {
        return Ok(Resposta::Liberar);
    };
    if Linguagem::do_caminho(&caminho.to_string_lossy()).is_none() {
        return Ok(Resposta::Liberar);
    }
    let Some(raiz) = achar_raiz(caminho.parent().unwrap_or(&caminho)) else {
        return Ok(Resposta::Liberar);
    };
    let Some(rel) = relativo(&raiz, &caminho) else {
        return Ok(Resposta::Liberar);
    };
    let cfg = Config::carregar(&raiz)?;
    let r = gate::verificar(&raiz, &cfg, Pedido::Arquivos(vec![rel]), &Opcoes::default())?;
    if !r.falhou {
        return Ok(Resposta::Liberar);
    }
    let resumo = saida::Resumo {
        congeladas: r.congeladas,
        tempo: r.tempo,
        falhou: true,
    };
    Ok(Resposta::Bloquear(format!(
        "{}Corrija agora, enquanto o contexto está fresco.",
        saida::compacta(&r.novas, 30, &resumo)
    )))
}

const ESTADO_FINAL: &str = ".guarda/cache/final.json";
const RELATORIO_FINAL: &str = ".guarda/cache/final.txt";

#[derive(Default, Serialize, Deserialize)]
struct EstadoFinal {
    sessao: String,
    tentativas: u32,
    /// Violações que já foram liberadas nesta sessão: não seguram a IA de novo.
    liberadas: Vec<String>,
}

fn digital(v: &Violacao) -> String {
    let e = baseline::entrada(v);
    format!("{}:{}:{}", e.codigo, e.arquivo, e.hash)
}

fn final_(entrada: &Value) -> Result<Resposta> {
    let Some(raiz) = raiz_da_sessao(entrada) else {
        return Ok(Resposta::Liberar);
    };
    let cfg = Config::carregar(&raiz)?;
    let r = match gate::verificar(&raiz, &cfg, Pedido::Alterados, &Opcoes::default()) {
        Ok(r) => r,
        // Sem git (ou sem commit): verifica o projeto todo.
        Err(_) => gate::verificar(&raiz, &cfg, Pedido::Tudo, &Opcoes::default())?,
    };
    let sessao = texto_de(entrada, "session_id").unwrap_or("").to_string();
    let caminho_estado = raiz.join(ESTADO_FINAL);
    let mut estado: EstadoFinal = std::fs::read_to_string(&caminho_estado)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .filter(|e: &EstadoFinal| e.sessao == sessao)
        .unwrap_or_else(|| EstadoFinal {
            sessao: sessao.clone(),
            ..Default::default()
        });

    let pendentes: Vec<Violacao> = r
        .novas
        .iter()
        .filter(|v| !estado.liberadas.contains(&digital(v)))
        .cloned()
        .collect();
    use crate::modelo::Severidade;
    let bloqueia = match cfg.gate.falhar_em {
        Severidade::Aviso => !pendentes.is_empty(),
        Severidade::Erro => pendentes.iter().any(|v| v.severidade == Severidade::Erro),
    };
    let salvar = |e: &EstadoFinal| -> Result<()> {
        std::fs::create_dir_all(caminho_estado.parent().expect("pai"))?;
        std::fs::write(&caminho_estado, serde_json::to_string(e)?)?;
        Ok(())
    };

    if !bloqueia {
        estado.tentativas = 0;
        // Avisos (ex.: GRD001) não seguram a IA; você fica sabendo uma vez por sessão.
        estado.liberadas.extend(pendentes.iter().map(digital));
        estado.liberadas.sort();
        estado.liberadas.dedup();
        salvar(&estado)?;
        let avisos: Vec<String> = pendentes
            .iter()
            .map(|v| format!("{} {}:{} {}", v.codigo, v.arquivo, v.linha, v.mensagem))
            .collect();
        return Ok(if avisos.is_empty() {
            Resposta::Liberar
        } else {
            mensagem(&format!("guarda: {}", avisos.join("; ")))
        });
    }

    estado.tentativas += 1;
    let limite = cfg.gate.tentativas_final;
    let resumo = saida::Resumo {
        congeladas: r.congeladas,
        tempo: r.tempo,
        falhou: true,
    };
    let relatorio = saida::compacta(&pendentes, 30, &resumo);
    if estado.tentativas > limite {
        std::fs::write(raiz.join(RELATORIO_FINAL), &relatorio)?;
        estado.liberadas.extend(pendentes.iter().map(digital));
        estado.liberadas.sort();
        estado.liberadas.dedup();
        estado.tentativas = 0;
        salvar(&estado)?;
        let erros = pendentes.len();
        return Ok(mensagem(&format!(
            "guarda: liberei o encerramento depois de {limite} tentativas; {erros} violação(ões) \
             pendente(s). Relatório em {RELATORIO_FINAL}."
        )));
    }
    salvar(&estado)?;
    Ok(Resposta::Bloquear(format!(
        "{relatorio}A tarefa não termina com violação nova (tentativa {} de {limite}). \
         Corrija e confira com `guarda check --alterados`.",
        estado.tentativas
    )))
}

fn mensagem(texto: &str) -> Resposta {
    Resposta::LiberarComMensagem(json!({ "systemMessage": texto }).to_string())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn comandos() {
        assert!(comando_proibido("guarda baseline gerar").is_some());
        assert!(comando_proibido("cd x && guarda.exe init --forcar").is_some());
        assert!(comando_proibido("rm -rf .guarda").is_some());
        assert!(comando_proibido("echo 'x' > guarda.yml").is_some());
        assert!(comando_proibido("Remove-Item .guarda\\baseline.json").is_some());
        assert!(comando_proibido("git checkout -- guarda.yml").is_some());
        assert!(comando_proibido("cat guarda.yml").is_none());
        assert!(comando_proibido("guarda check --alterados").is_none());
        assert!(comando_proibido("cargo test").is_none());
    }

    #[test]
    fn caminhos_protegidos() {
        assert!(protegido("guarda.yml"));
        assert!(protegido(".guarda/baseline.json"));
        assert!(protegido(".GUARDA/x"));
        assert!(!protegido("src/main/guarda.kt"));
    }

    #[test]
    fn cobertura_no_build() {
        let (re, _) = agulha("pom.xml").unwrap();
        let ti =
            json!({"old_string": "<artifactId>jacoco-maven-plugin</artifactId>", "new_string": ""});
        assert!(mexe_na_agulha(re, "Edit", &ti, Path::new("x")));
        let ti =
            json!({"old_string": "<version>1</version>", "new_string": "<version>2</version>"});
        assert!(!mexe_na_agulha(re, "Edit", &ti, Path::new("x")));
        assert!(agulha(".claude/settings.json").is_some());
        assert!(agulha("src/settings.json").is_none());
    }
}
