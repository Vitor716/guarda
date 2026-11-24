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

fn eh_comando_da_guarda(h: &Value) -> bool {
    h["command"]
        .as_str()
        .is_some_and(|c| c.starts_with("guarda hook"))
}

fn eh_da_guarda(grupo: &Value) -> bool {
    grupo["hooks"]
        .as_array()
        .is_some_and(|hs| hs.iter().any(eh_comando_da_guarda))
}

/// Um hook da guarda: evento, matcher (regex ou lista) e fase.
struct Gancho {
    evento: &'static str,
    matcher: Option<&'static str>,
    fase: &'static str,
    timeout: u32,
}

fn ganchos(agente: &Agente) -> Vec<Gancho> {
    let g = |evento, matcher, fase, timeout| Gancho {
        evento,
        matcher,
        fase,
        timeout,
    };
    match agente {
        Agente::Claude => vec![
            g(
                "PreToolUse",
                Some("Edit|Write|MultiEdit|NotebookEdit|Bash|PowerShell"),
                "proteger",
                10,
            ),
            g(
                "PostToolUse",
                Some("Edit|Write|MultiEdit"),
                "pos-edicao",
                30,
            ),
            g("Stop", None, "final", 120),
        ],
        Agente::Codex => vec![
            g("PreToolUse", Some("^(Bash|apply_patch)$"), "proteger", 10),
            g("PostToolUse", Some("^apply_patch$"), "pos-edicao", 30),
            g("Stop", None, "final", 120),
        ],
        // O PostToolUse do Antigravity não devolve nada à IA: a verificação fica no Stop.
        Agente::Antigravity => vec![
            g(
                "PreToolUse",
                Some(
                    "^(write_to_file|replace_file_content|multi_replace_file_content|run_command)$",
                ),
                "proteger",
                10,
            ),
            g("Stop", None, "final", 120),
        ],
        Agente::Agents | Agente::Git => Vec::new(),
    }
}

fn comando(agente: &Agente, fase: &str) -> String {
    match agente {
        // Sem --agente: compatível com o que a Fase 1 escreveu.
        Agente::Claude => format!("guarda hook {fase}"),
        Agente::Codex => format!("guarda hook {fase} --agente codex"),
        _ => format!("guarda hook {fase} --agente antigravity"),
    }
}

/// Formato do Claude Code e do Codex: `hooks.<evento>: [{matcher, hooks: [...]}]`.
/// Junta os hooks da guarda preservando o resto e sem duplicar.
pub fn mesclar_hooks(doc: &mut Value, agente: &Agente) -> Result<()> {
    if !doc.is_object() {
        bail!("o arquivo de configuração não é um objeto JSON");
    }
    let hooks = doc
        .as_object_mut()
        .expect("objeto")
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()));
    let hooks = hooks.as_object_mut().context("\"hooks\" não é um objeto")?;
    for g in ganchos(agente) {
        let lista = hooks
            .entry(g.evento)
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .with_context(|| format!("hooks.{} não é uma lista", g.evento))?;
        lista.retain(|x| !eh_da_guarda(x));
        let mut grupo = Map::new();
        if let Some(m) = g.matcher {
            grupo.insert("matcher".into(), json!(m));
        }
        grupo.insert(
            "hooks".into(),
            json!([{ "type": "command", "command": comando(agente, g.fase), "timeout": g.timeout }]),
        );
        lista.push(Value::Object(grupo));
    }
    Ok(())
}

/// Formato do Antigravity: `{"<nome do hook>": {"<evento>": ...}}`; eventos com matcher levam
/// `[{matcher, hooks}]` e o Stop leva a lista de handlers direto.
pub fn mesclar_hooks_antigravity(doc: &mut Value) -> Result<()> {
    let obj = doc
        .as_object_mut()
        .context("hooks.json não é um objeto JSON")?;
    let mut nosso = Map::new();
    for g in ganchos(&Agente::Antigravity) {
        let handler = json!({ "type": "command", "command": comando(&Agente::Antigravity, g.fase), "timeout": g.timeout });
        let valor = match g.matcher {
            Some(m) => json!([{ "matcher": m, "hooks": [handler] }]),
            None => json!([handler]),
        };
        nosso.insert(g.evento.to_string(), valor);
    }
    obj.insert("guarda".into(), Value::Object(nosso));
    Ok(())
}

/// `mcpServers.guarda` (formato comum do Claude Code e do Antigravity).
pub fn mesclar_mcp(doc: &mut Value) -> Result<()> {
    let obj = doc
        .as_object_mut()
        .context("configuração de MCP não é um objeto JSON")?;
    let servidores = obj
        .entry("mcpServers")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .context("\"mcpServers\" não é um objeto")?;
    servidores.insert(
        "guarda".into(),
        json!({ "command": "guarda", "args": ["mcp"] }),
    );
    Ok(())
}

fn ler_json(caminho: &Path) -> Result<Value> {
    Ok(match std::fs::read_to_string(caminho) {
        Ok(t) if !t.trim().is_empty() => serde_json::from_str(&t)
            .with_context(|| format!("{} não é JSON válido", caminho.display()))?,
        _ => json!({}),
    })
}

fn gravar_json(caminho: &Path, v: &Value) -> Result<()> {
    std::fs::create_dir_all(caminho.parent().expect("pai"))?;
    std::fs::write(caminho, serde_json::to_string_pretty(v)? + "\n")?;
    Ok(())
}

fn editar_json(raiz: &Path, rel: &str, f: impl FnOnce(&mut Value) -> Result<()>) -> Result<String> {
    let caminho = raiz.join(rel);
    let mut v = ler_json(&caminho)?;
    f(&mut v)?;
    gravar_json(&caminho, &v)?;
    Ok(rel.to_string())
}

pub enum Agente {
    Claude,
    Codex,
    Antigravity,
    /// Só o AGENTS.md (Cursor, Copilot e outros que leem o padrão).
    Agents,
    /// Hook de pre-commit do git: barra o commit de qualquer IA ou pessoa.
    Git,
}

pub const PRE_COMMIT: &str = "#!/bin/sh\n# guarda: verificação antes do commit (gerado por `guarda integrar git`)\nexec guarda check --alterados\n";

/// Instala o pre-commit, sem pisar num pre-commit que não seja da guarda.
fn instalar_pre_commit(raiz: &Path) -> Result<String> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--git-path", "hooks/pre-commit"])
        .current_dir(raiz)
        .output()
        .context("não consegui rodar o git")?;
    if !out.status.success() {
        bail!("{} não é um repositório git", raiz.display());
    }
    let rel = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let caminho = raiz.join(&rel);
    if let Ok(atual) = std::fs::read_to_string(&caminho)
        && !atual.contains("guarda")
    {
        bail!(
            "{rel} já existe e não é da guarda; acrescente `guarda check --alterados` nele à mão"
        );
    }
    std::fs::create_dir_all(caminho.parent().expect("pai"))?;
    std::fs::write(&caminho, PRE_COMMIT)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&caminho, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(rel)
}

pub struct Integracao {
    pub escritos: Vec<String>,
    /// O que a pessoa ainda precisa fazer no agente.
    pub notas: Vec<String>,
}

/// Escreve o contexto no arquivo de instruções do agente, os hooks e o servidor MCP.
pub fn integrar(raiz: &Path, cfg: &Config, agente: Agente) -> Result<Integracao> {
    let mut escritos = Vec::new();
    let mut notas = Vec::new();
    let ctx = || contexto(raiz, cfg);
    match agente {
        Agente::Claude => {
            escrever_bloco(&raiz.join("CLAUDE.md"), &ctx()?)?;
            escritos.push("CLAUDE.md".to_string());
            escritos.push(editar_json(raiz, ".claude/settings.json", |v| {
                mesclar_hooks(v, &agente)
            })?);
            escritos.push(editar_json(raiz, ".mcp.json", mesclar_mcp)?);
            notas.push("Claude Code: aprove o servidor MCP `guarda` quando ele pedir".into());
        }
        Agente::Codex => {
            escrever_bloco(&raiz.join("AGENTS.md"), &ctx()?)?;
            escritos.push("AGENTS.md".to_string());
            escritos.push(editar_json(raiz, ".codex/hooks.json", |v| {
                mesclar_hooks(v, &agente)
            })?);
            notas.push("Codex: confie no projeto e revise os hooks em /hooks (o Codex só roda hook revisado)".into());
            notas.push(
                "Codex: para o MCP, acrescente ao ~/.codex/config.toml: [mcp_servers.guarda] command = \"guarda\" args = [\"mcp\"]"
                    .into(),
            );
        }
        Agente::Antigravity => {
            escrever_bloco(&raiz.join("AGENTS.md"), &ctx()?)?;
            escritos.push("AGENTS.md".to_string());
            escritos.push(editar_json(
                raiz,
                ".agents/hooks.json",
                mesclar_hooks_antigravity,
            )?);
            escritos.push(editar_json(raiz, ".agents/mcp_config.json", mesclar_mcp)?);
            notas
                .push("Antigravity: confira com /hooks e /mcp que os dois foram carregados".into());
        }
        Agente::Agents => {
            escrever_bloco(&raiz.join("AGENTS.md"), &ctx()?)?;
            escritos.push("AGENTS.md".to_string());
        }
        Agente::Git => {
            escritos.push(instalar_pre_commit(raiz)?);
            notas.push(
                "git: todo commit passa por `guarda check --alterados`, venha de qual IA vier"
                    .into(),
            );
        }
    }
    Ok(Integracao { escritos, notas })
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
        mesclar_hooks(&mut s, &Agente::Claude).unwrap();
        mesclar_hooks(&mut s, &Agente::Claude).unwrap();
        assert_eq!(s["permissions"]["allow"][0], "Bash(ls)");
        let pre = s["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2, "{pre:?}");
        assert_eq!(pre[0]["hooks"][0]["command"], "meu-hook");
        assert_eq!(s["hooks"]["Stop"].as_array().unwrap().len(), 1);
        assert!(s["hooks"]["Stop"][0].get("matcher").is_none());
    }

    #[test]
    fn codex_e_antigravity() {
        let mut c = json!({});
        mesclar_hooks(&mut c, &Agente::Codex).unwrap();
        assert_eq!(
            c["hooks"]["PreToolUse"][0]["matcher"],
            "^(Bash|apply_patch)$"
        );
        assert_eq!(
            c["hooks"]["Stop"][0]["hooks"][0]["command"],
            "guarda hook final --agente codex"
        );

        let mut a = json!({ "outro-hook": { "Stop": [] } });
        mesclar_hooks_antigravity(&mut a).unwrap();
        mesclar_hooks_antigravity(&mut a).unwrap();
        assert!(a["outro-hook"].is_object());
        assert_eq!(
            a["guarda"]["Stop"][0]["command"],
            "guarda hook final --agente antigravity"
        );
        assert!(
            a["guarda"]["PreToolUse"][0]["matcher"]
                .as_str()
                .unwrap()
                .contains("run_command")
        );
        assert!(a["guarda"].get("PostToolUse").is_none());

        let mut m = json!({ "mcpServers": { "outro": { "command": "x" } } });
        mesclar_mcp(&mut m).unwrap();
        assert_eq!(m["mcpServers"]["guarda"]["args"][0], "mcp");
        assert_eq!(m["mcpServers"]["outro"]["command"], "x");
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
