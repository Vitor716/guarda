//! `guarda mcp`: servidor MCP por stdio (JSON-RPC 2.0, uma mensagem por linha), para qualquer
//! agente que fale MCP (Claude Code, Codex, Antigravity, Cursor, Copilot...).
//!
//! Ferramentas: `verificar` (arquivos ou o que mudou), `explicar` (uma regra) e `arquitetura`
//! (camadas e o que cada uma importa, para decidir onde criar uma classe antes de escrever).
//! O stdout carrega só JSON-RPC; qualquer log vai para o stderr.

use crate::config::{self, Config};
use crate::gate::{self, Opcoes, Pedido};
use crate::{explicar, integracao, saida};
use anyhow::Result;
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

const VERSOES: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// Raiz: o diretório mais próximo, subindo a partir de `inicio`, que tem um guarda.yml.
pub fn achar_raiz(inicio: &Path) -> PathBuf {
    let mut d = Some(inicio);
    while let Some(x) = d {
        if x.join(config::ARQUIVO).is_file() {
            return x.to_path_buf();
        }
        d = x.parent();
    }
    inicio.to_path_buf()
}

pub fn servir(raiz: &Path) -> Result<()> {
    let entrada = std::io::stdin().lock();
    let mut saida_ = std::io::stdout().lock();
    for linha in entrada.lines() {
        let linha = linha?;
        let linha = linha.trim_start_matches('\u{feff}').trim();
        if linha.is_empty() {
            continue;
        }
        if let Some(resposta) = tratar(raiz, linha) {
            writeln!(saida_, "{resposta}")?;
            saida_.flush()?;
        }
    }
    Ok(())
}

fn erro(id: Value, codigo: i64, mensagem: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": codigo, "message": mensagem } })
}

/// Trata uma mensagem; `None` para notificações (sem `id`).
pub fn tratar(raiz: &Path, linha: &str) -> Option<Value> {
    let msg: Value = match serde_json::from_str(linha) {
        Ok(v) => v,
        Err(e) => return Some(erro(Value::Null, -32700, &format!("JSON inválido: {e}"))),
    };
    let id = msg.get("id").cloned()?;
    let metodo = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let resultado = match metodo {
        "initialize" => {
            let pedida = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or("");
            let versao = VERSOES
                .iter()
                .find(|v| **v == pedida)
                .copied()
                .unwrap_or(VERSOES[0]);
            json!({
                "protocolVersion": versao,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "guarda", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "Linter determinístico do projeto. Use `arquitetura` antes de criar classe, `verificar` depois de editar e `explicar` para entender uma regra."
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({ "tools": ferramentas() }),
        "tools/call" => {
            let nome = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            let (texto, falhou) = match chamar(raiz, nome, &args) {
                Ok(t) => t,
                Err(e) => (format!("guarda: {e:#}"), true),
            };
            json!({ "content": [{ "type": "text", "text": texto }], "isError": falhou })
        }
        outro => return Some(erro(id, -32601, &format!("método desconhecido: {outro}"))),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": resultado }))
}

fn ferramentas() -> Value {
    json!([
        {
            "name": "verificar",
            "description": "Verifica arquivos Kotlin/Java contra as regras do projeto (arquitetura, complexidade, boas práticas, cobertura). Sem argumentos, verifica o que mudou contra HEAD. Devolve uma linha por violação nova, com a correção depois do `|`; só '✓ guarda 0 erros' quando está tudo certo.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "arquivos": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Caminhos relativos à raiz do projeto. Vazio: o que mudou."
                    },
                    "projeto_todo": { "type": "boolean", "description": "Verifica o projeto inteiro." }
                }
            }
        },
        {
            "name": "explicar",
            "description": "Explica uma regra da guarda pelo código (ex.: ARQ001, CPX001, KT001): por quê, exemplo errado e certo.",
            "inputSchema": {
                "type": "object",
                "properties": { "codigo": { "type": "string" } },
                "required": ["codigo"]
            }
        },
        {
            "name": "arquitetura",
            "description": "Camadas do projeto, a pasta real de cada uma e o que cada camada pode importar, mais os limites e proibições. Use antes de criar uma classe para decidir onde ela vai.",
            "inputSchema": { "type": "object", "properties": {} }
        }
    ])
}

fn chamar(raiz: &Path, nome: &str, args: &Value) -> Result<(String, bool)> {
    match nome {
        "verificar" => {
            let cfg = Config::carregar(raiz)?;
            let arquivos: Vec<String> = args
                .get("arquivos")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(|s| s.replace('\\', "/").trim_start_matches("./").to_string())
                        .collect()
                })
                .unwrap_or_default();
            let pedido = if args.get("projeto_todo").and_then(Value::as_bool) == Some(true) {
                Pedido::Tudo
            } else if arquivos.is_empty() {
                Pedido::Alterados
            } else {
                Pedido::Arquivos(arquivos)
            };
            let r = gate::verificar(raiz, &cfg, pedido, &Opcoes::default())?;
            let resumo = saida::Resumo {
                congeladas: r.congeladas,
                tempo: r.tempo,
                falhou: r.falhou,
                nota: r.nota.clone(),
            };
            // Violação é resultado da ferramenta, não erro dela.
            Ok((saida::compacta(&r.novas, 30, &resumo), false))
        }
        "explicar" => {
            let codigo = args.get("codigo").and_then(Value::as_str).unwrap_or("");
            Ok(match explicar::explicar(codigo) {
                Some(t) => (t, false),
                None => (
                    format!("regra desconhecida: {codigo}\n\n{}", explicar::lista()),
                    true,
                ),
            })
        }
        "arquitetura" => {
            let cfg = Config::carregar(raiz)?;
            Ok((integracao::contexto(raiz, &cfg)?, false))
        }
        outro => Ok((format!("ferramenta desconhecida: {outro}"), true)),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn protocolo_basico() {
        let raiz = Path::new(".");
        let r = tratar(raiz, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#).unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(r["result"]["serverInfo"]["name"], "guarda");
        assert!(
            tratar(
                raiz,
                r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#
            )
            .is_none()
        );
        let r = tratar(raiz, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap();
        let nomes: Vec<&str> = r["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(nomes, ["verificar", "explicar", "arquitetura"]);
        let r = tratar(raiz, r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"explicar","arguments":{"codigo":"KT001"}}}"#).unwrap();
        assert_eq!(r["result"]["isError"], false);
        assert!(
            r["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("requireNotNull")
        );
        let r = tratar(raiz, r#"{"jsonrpc":"2.0","id":4,"method":"x"}"#).unwrap();
        assert_eq!(r["error"]["code"], -32601);
        let r = tratar(raiz, "{nao json").unwrap();
        assert_eq!(r["error"]["code"], -32700);
    }
}
