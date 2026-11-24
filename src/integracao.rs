//! Integração com IA: `guarda contexto` (as regras do projeto em poucas linhas, geradas do
//! mesmo guarda.yml que a verificação usa) e `guarda integrar` (escreve esse contexto no
//! CLAUDE.md/AGENTS.md e os hooks no .claude/settings.json).

use crate::config::{self, Config};
use crate::motor::{self, Escopo, Relatorio};
use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::Path;

pub const INICIO: &str = "<!-- guarda:inicio (gerado por `guarda integrar`; não edite à mão) -->";
pub const FIM: &str = "<!-- guarda:fim -->";

/// As regras do projeto em ~15 linhas, para o prompt da IA.
pub fn contexto(raiz: &Path, cfg: &Config) -> Result<String> {
    let rel = motor::executar(raiz, cfg, &Escopo::Tudo)?;
    Ok(montar_contexto(&rel, cfg))
}

fn montar_contexto(rel: &Relatorio, cfg: &Config) -> String {
    let mut l = Vec::new();
    l.push("## guarda (verificação determinística)".to_string());
    l.push(
        "- Antes de terminar qualquer tarefa, rode `guarda check --alterados` e só termine com \
         `✓ guarda 0 erros`. Cada linha de violação traz a correção depois do `|`."
            .to_string(),
    );
    match &rel.arquitetura {
        Some(arq) if !arq.camadas.is_empty() => {
            let base = rel.projeto.base.as_deref().unwrap_or("");
            l.push(format!(
                "- Arquitetura {} (base `{base}`). Cada camada só importa as listadas; \
                 classe nova vai na pasta da camada certa:",
                arq.nome
            ));
            // Pastas reais do projeto em cada camada.
            let mut pastas: BTreeMap<&str, std::collections::BTreeSet<String>> = BTreeMap::new();
            for p in &rel.projeto.pacotes {
                if let Some((c, pasta)) = arq.camada_com_pasta(p, &rel.projeto) {
                    let curta = pasta
                        .strip_prefix(base)
                        .map(|s| s.trim_start_matches('.'))
                        .filter(|s| !s.is_empty())
                        .unwrap_or(&pasta)
                        .to_string();
                    pastas.entry(c).or_default().insert(curta);
                }
            }
            for c in &arq.camadas {
                let pode: Vec<&str> = arq
                    .camadas
                    .iter()
                    .map(|o| o.nome.as_str())
                    .filter(|o| *o != c.nome && arq.permitido(&c.nome, o))
                    .collect();
                let onde: Vec<String> = pastas
                    .get(c.nome.as_str())
                    .map(|s| s.iter().cloned().collect())
                    .unwrap_or_default();
                l.push(format!(
                    "  - {}{}: importa {}",
                    c.nome,
                    if onde.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", onde.join(", "))
                    },
                    if pode.is_empty() {
                        "nenhuma outra camada".to_string()
                    } else {
                        pode.join(", ")
                    }
                ));
            }
        }
        _ => l.push("- Arquitetura: nenhuma camada declarada (ARQ001 desligada).".to_string()),
    }
    l.push(format!(
        "- Por função: no máximo {} linhas de código e complexidade cognitiva {}.",
        cfg.limites.linhas_funcao, cfg.limites.complexidade_cognitiva
    ));
    l.push("- Kotlin: sem `!!` (use `?:`, `?.let` ou `requireNotNull` com mensagem).".to_string());
    l.push(
        "- Supressão só com motivo: `// guarda:ignorar CODIGO motivo: ...`; \
         `@Suppress`/`@SuppressWarnings` com comentário do motivo na linha."
            .to_string(),
    );
    l.push(
        "- Não edite `guarda.yml` nem `.guarda/` e não rode `guarda baseline`: o legado congelado \
         só encolhe, e congelar dívida nova é decisão humana."
            .to_string(),
    );
    if !cfg.regras.desligar.is_empty() {
        l.push(format!(
            "- Regras desligadas: {}.",
            cfg.regras.desligar.join(", ")
        ));
    }
    l.join("\n") + "\n"
}

/// Troca (ou acrescenta) o bloco da guarda num arquivo Markdown.
pub fn escrever_bloco(caminho: &Path, contexto: &str) -> Result<()> {
    let atual = std::fs::read_to_string(caminho).unwrap_or_default();
    let bloco = format!("{INICIO}\n{contexto}{FIM}\n");
    let novo = match (atual.find(INICIO), atual.find(FIM)) {
        (Some(i), Some(f)) if f > i => {
            let fim = f + FIM.len();
            let resto = atual[fim..].strip_prefix('\n').unwrap_or(&atual[fim..]);
            format!("{}{bloco}{resto}", &atual[..i])
        }
        (None, None) if atual.trim().is_empty() => bloco,
        (None, None) => format!("{}\n\n{bloco}", atual.trim_end()),
        _ => bail!(
            "{}: marcador da guarda incompleto; conserte à mão",
            caminho.display()
        ),
    };
    std::fs::write(caminho, novo)?;
    Ok(())
}

fn eh_da_guarda(grupo: &Value) -> bool {
    grupo["hooks"].as_array().is_some_and(|hs| {
        hs.iter().any(|h| {
            h["command"]
                .as_str()
                .is_some_and(|c| c.starts_with("guarda hook"))
        })
    })
}

/// Junta os hooks da guarda ao settings.json, preservando o resto e sem duplicar.
pub fn mesclar_hooks(settings: &mut Value) -> Result<()> {
    if !settings.is_object() {
        bail!("settings.json não é um objeto JSON");
    }
    let hooks = settings
        .as_object_mut()
        .expect("objeto")
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()));
    let hooks = hooks.as_object_mut().context("\"hooks\" não é um objeto")?;
    let nossos = [
        (
            "PreToolUse",
            Some("Edit|Write|MultiEdit|NotebookEdit|Bash|PowerShell"),
            "guarda hook proteger",
            10,
        ),
        (
            "PostToolUse",
            Some("Edit|Write|MultiEdit"),
            "guarda hook pos-edicao",
            30,
        ),
        ("Stop", None, "guarda hook final", 120),
    ];
    for (evento, matcher, comando, timeout) in nossos {
        let lista = hooks
            .entry(evento)
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .with_context(|| format!("hooks.{evento} não é uma lista"))?;
        lista.retain(|g| !eh_da_guarda(g));
        let mut grupo = Map::new();
        if let Some(m) = matcher {
            grupo.insert("matcher".into(), json!(m));
        }
        grupo.insert(
            "hooks".into(),
            json!([{ "type": "command", "command": comando, "timeout": timeout }]),
        );
        lista.push(Value::Object(grupo));
    }
    Ok(())
}

pub enum Agente {
    Claude,
    Agents,
}

/// Escreve o contexto no arquivo de instruções do agente e, no Claude Code, os hooks.
pub fn integrar(raiz: &Path, cfg: &Config, agente: Agente) -> Result<Vec<String>> {
    let ctx = contexto(raiz, cfg)?;
    let mut escritos = Vec::new();
    match agente {
        Agente::Claude => {
            escrever_bloco(&raiz.join("CLAUDE.md"), &ctx)?;
            escritos.push("CLAUDE.md".to_string());
            let caminho = raiz.join(".claude").join("settings.json");
            let mut settings: Value = match std::fs::read_to_string(&caminho) {
                Ok(t) if !t.trim().is_empty() => serde_json::from_str(&t)
                    .with_context(|| format!("{} não é JSON válido", caminho.display()))?,
                _ => json!({}),
            };
            mesclar_hooks(&mut settings)?;
            std::fs::create_dir_all(caminho.parent().expect("pai"))?;
            std::fs::write(&caminho, serde_json::to_string_pretty(&settings)? + "\n")?;
            escritos.push(".claude/settings.json".to_string());
        }
        Agente::Agents => {
            escrever_bloco(&raiz.join("AGENTS.md"), &ctx)?;
            escritos.push("AGENTS.md".to_string());
        }
    }
    Ok(escritos)
}

/// Grava o JSON Schema em `.guarda/` (o guarda.yml aponta para ele na primeira linha).
pub fn gravar_schema(raiz: &Path) -> Result<()> {
    let caminho = raiz.join(config::ARQUIVO_SCHEMA);
    std::fs::create_dir_all(caminho.parent().expect("pai"))?;
    std::fs::write(caminho, config::schema())?;
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn mescla_sem_duplicar_e_preserva_o_resto() {
        let mut s = json!({
            "permissions": { "allow": ["Bash(ls)"] },
            "hooks": { "PreToolUse": [
                { "matcher": "Bash", "hooks": [{ "type": "command", "command": "meu-hook" }] }
            ]}
        });
        mesclar_hooks(&mut s).unwrap();
        mesclar_hooks(&mut s).unwrap();
        assert_eq!(s["permissions"]["allow"][0], "Bash(ls)");
        let pre = s["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2, "{pre:?}");
        assert_eq!(pre[0]["hooks"][0]["command"], "meu-hook");
        assert_eq!(s["hooks"]["Stop"].as_array().unwrap().len(), 1);
        assert!(s["hooks"]["Stop"][0].get("matcher").is_none());
    }

    #[test]
    fn bloco_e_trocado_no_lugar() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("CLAUDE.md");
        std::fs::write(&md, "# Projeto\n\nTexto meu.\n").unwrap();
        escrever_bloco(&md, "v1\n").unwrap();
        escrever_bloco(&md, "v2\n").unwrap();
        let t = std::fs::read_to_string(&md).unwrap();
        assert!(t.starts_with("# Projeto\n\nTexto meu.\n\n"), "{t}");
        assert_eq!(t.matches(INICIO).count(), 1);
        assert!(t.contains("v2\n") && !t.contains("v1\n"));
    }
}
