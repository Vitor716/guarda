//! Fase 2: cobertura (JaCoCo), duplicação, e a integração agnóstica de agente
//! (hooks do Codex e do Antigravity, servidor MCP, pre-commit do git).

mod comum;

use comum::{git, guarda, projeto, rodar};
use serde_json::{Value, json};
use std::path::Path;

fn repo_com_base(r: &Path) {
    git(r, &["init", "-q"]);
    git(r, &["add", "-A"]);
    git(r, &["commit", "-q", "-m", "base"]);
}

const TAXA: &str = "package com.app.application

class Taxa {
    fun aliquota(isenta: Boolean): Double {
        if (isenta) {
            return 0.0
        }
        return 0.18
    }
}
";

// Linha 5 com um dos dois caminhos do if sem teste; linha 6 sem teste nenhum.
const JACOCO: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><!DOCTYPE report PUBLIC "-//JACOCO//DTD Report 1.1//EN" "report.dtd"><report name="t"><package name="com/app/application"><sourcefile name="Taxa.kt"><line nr="3" mi="0" ci="3" mb="0" cb="0"/><line nr="5" mi="0" ci="2" mb="1" cb="1"/><line nr="6" mi="2" ci="0" mb="0" cb="0"/><line nr="8" mi="0" ci="2" mb="0" cb="0"/></sourcefile></package></report>"#;

fn com_cobertura(r: &Path) {
    std::fs::write(
        r.join("guarda.yml"),
        "arquitetura:\n  preset: nenhum\ncobertura:\n  relatorio: [cov.xml]\n  codigo-novo: { referencia: HEAD, minimo: 80 }\n  geral-minimo: 90\n",
    )
    .unwrap();
    repo_com_base(r);
    std::fs::write(r.join("src/main/kotlin/com/app/application/Taxa.kt"), TAXA).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(30));
    std::fs::write(r.join("cov.xml"), JACOCO).unwrap();
}

#[test]
fn cobertura_do_codigo_novo_e_geral() {
    let p = projeto();
    let r = p.path();
    com_cobertura(r);
    let (c, s) = guarda(r, &["check", "--sem-baseline", "--max", "100"]);
    assert_eq!(c, 1, "{s}");
    assert!(
        s.contains(
            "  6 COV001 l.6 sem teste, l.5 com desvio sem teste | teste o outro caminho do if/&&/|| nas l.5"
        ),
        "{s}"
    );
    assert!(
        s.contains(
            "[cobertura]\n  1 COV001 código novo 66,7%<80,0% (1 arquivo(s) com linha sem teste)"
        ),
        "{s}"
    );
    assert!(s.contains("  1 COV003 projeto 66,7%<90,0%"), "{s}");
}

#[test]
fn cobertura_desatualizada_vira_nota_e_nao_reprova() {
    let p = projeto();
    let r = p.path();
    com_cobertura(r);
    std::thread::sleep(std::time::Duration::from_millis(30));
    let taxa = r.join("src/main/kotlin/com/app/application/Taxa.kt");
    std::fs::write(&taxa, TAXA.replace("0.18", "0.17")).unwrap();
    let (_, s) = guarda(
        r,
        &[
            "check",
            "--sem-baseline",
            "--arquivos",
            "src/main/kotlin/com/app/application/Taxa.kt",
        ],
    );
    assert!(!s.contains("COV001"), "{s}");
    assert!(s.contains("cobertura desatualizada em 1 arquivo(s)"), "{s}");
}

#[test]
fn cobertura_geral_entra_no_baseline_como_catraca() {
    let p = projeto();
    let r = p.path();
    com_cobertura(r);
    let (c, s) = guarda(r, &["baseline", "gerar"]);
    assert_eq!(c, 0, "{s}");
    let b = std::fs::read_to_string(r.join(".guarda/baseline.json")).unwrap();
    assert!(
        b.contains(r#""codigo":"COV003""#) && b.contains(r#""valor":333"#),
        "{b}"
    );
    assert!(
        !b.contains("COV001"),
        "COV001 é da mudança, não do legado: {b}"
    );
    let (_, s) = guarda(r, &["check"]);
    assert!(!s.contains("COV003"), "{s}");
    assert!(s.contains("COV001"), "{s}");
}

fn metodo_grande(nome: &str) -> String {
    let mut corpo = String::new();
    for i in 0..15 {
        corpo.push_str(&format!(
            "        total += valores[{i}] * {} + ajuste;\n",
            i + 2
        ));
    }
    format!(
        "    int {nome}(int[] valores, int ajuste) {{\n        int total = 0;\n{corpo}        return total;\n    }}\n"
    )
}

#[test]
fn duplicacao_entre_arquivos_e_densidade_no_codigo_novo() {
    let p = projeto();
    let r = p.path();
    std::fs::write(
        r.join("guarda.yml"),
        "arquitetura:\n  preset: nenhum\nduplicacao:\n  tokens-minimos: 100\n  codigo-novo-maximo: 3\n  referencia: HEAD\n",
    )
    .unwrap();
    repo_com_base(r);
    let dir = r.join("src/main/java/com/app/domain");
    for classe in ["Soma", "Total"] {
        std::fs::write(
            dir.join(format!("{classe}.java")),
            format!(
                "package com.app.domain;\n\nclass {classe} {{\n{}}}\n",
                metodo_grande("calcular")
            ),
        )
        .unwrap();
    }
    let (c, s) = guarda(r, &["check", "--sem-baseline", "--max", "100"]);
    assert_eq!(c, 1, "{s}");
    // Só o nome da classe difere: o bloco começa no `{` da linha 3.
    assert!(
        s.contains("  3 SML002 21 linhas duplicadas (também em Total.java:3)"),
        "{s}"
    );
    assert!(
        s.contains("  3 SML002 21 linhas duplicadas (também em Soma.java:3)"),
        "{s}"
    );
    assert!(
        s.contains("[duplicação]\n  1 SML002 código novo com"),
        "{s}"
    );
}

#[test]
fn hooks_do_codex() {
    let p = projeto();
    let r = p.path();
    guarda(r, &["init"]);
    let cwd = r.to_string_lossy().to_string();
    let patch = json!({"tool_name": "apply_patch", "cwd": cwd, "session_id": "c1",
        "tool_input": {"command": "*** Begin Patch\n*** Update File: guarda.yml\n@@\n-a\n+b\n*** End Patch"}});
    let (c, _, e) = rodar(
        r,
        &["hook", "proteger", "--agente", "codex"],
        Some(&patch.to_string()),
    );
    assert_eq!(c, 2, "{e}");
    assert!(e.contains("guarda.yml é protegido"), "{e}");

    // Patch que acrescenta !! no Kotlin: o pos-edicao devolve a violação.
    let calc = "src/main/kotlin/com/app/application/Calculo.kt";
    let t = std::fs::read_to_string(r.join(calc))
        .unwrap()
        .replace("val a = n!!", "val z = n!!.cru()\n        val a = n!!");
    std::fs::write(r.join(calc), t).unwrap();
    let pos = json!({"tool_name": "apply_patch", "cwd": cwd,
        "tool_input": {"command": format!("*** Begin Patch\n*** Update File: {calc}\n@@\n+val z = n!!.cru()\n*** End Patch")}});
    let (c, _, e) = rodar(
        r,
        &["hook", "pos-edicao", "--agente", "codex"],
        Some(&pos.to_string()),
    );
    assert_eq!(c, 2, "{e}");
    assert!(e.contains("KT001"), "{e}");

    let fim = json!({"session_id": "c1", "cwd": cwd, "hook_event_name": "Stop"});
    let (c, o, _) = rodar(
        r,
        &["hook", "final", "--agente", "codex"],
        Some(&fim.to_string()),
    );
    assert_eq!(c, 0);
    let v: Value = serde_json::from_str(o.trim()).unwrap();
    assert_eq!(v["decision"], "block", "{o}");
    assert!(v["reason"].as_str().unwrap().contains("KT001"));
}

#[test]
fn hooks_do_antigravity_respondem_em_json() {
    let p = projeto();
    let r = p.path();
    guarda(r, &["init"]);
    let ws = r.to_string_lossy().to_string();
    let edita = |alvo: &str| {
        json!({"conversationId": "a1", "workspacePaths": [ws],
            "toolCall": {"name": "replace_file_content", "args": {
                "TargetFile": r.join(alvo).to_string_lossy(), "TargetContent": "a", "ReplacementContent": "b"}}})
    };
    let (c, o, _) = rodar(
        r,
        &["hook", "proteger", "--agente", "antigravity"],
        Some(&edita("guarda.yml").to_string()),
    );
    assert_eq!(c, 0);
    let v: Value = serde_json::from_str(o.trim()).unwrap();
    assert_eq!(v["decision"], "deny");
    let (_, o, _) = rodar(
        r,
        &["hook", "proteger", "--agente", "antigravity"],
        Some(&edita("src/main/kotlin/com/app/application/Calculo.kt").to_string()),
    );
    assert_eq!(o.trim(), r#"{"decision":"allow"}"#);

    // Sem git, o final verifica o projeto todo: o legado está no baseline, então libera.
    let fim = json!({"conversationId": "a1", "workspacePaths": [ws]});
    let (c, o, _) = rodar(
        r,
        &["hook", "final", "--agente", "antigravity"],
        Some(&fim.to_string()),
    );
    assert_eq!(c, 0);
    let v: Value = serde_json::from_str(o.trim()).unwrap();
    assert_eq!(v["decision"], "stop", "{o}");
}

#[test]
fn servidor_mcp_pelo_binario() {
    let p = projeto();
    let r = p.path();
    guarda(r, &["init"]);
    let entrada = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "verificar", "arguments": {"projeto_todo": true}}}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "arquitetura", "arguments": {}}}),
    ]
    .iter()
    .map(|v| v.to_string())
    .collect::<Vec<_>>()
    .join("\n")
        + "\n";
    let (c, o, e) = rodar(r, &["mcp"], Some(&entrada));
    assert_eq!(c, 0, "{e}");
    let respostas: Vec<Value> = o
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(respostas.len(), 3, "notificação não tem resposta: {o}");
    assert_eq!(respostas[0]["result"]["serverInfo"]["name"], "guarda");
    let verificar = respostas[1]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(verificar.starts_with("✓ guarda 0 erros"), "{verificar}");
    let arq = respostas[2]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(arq.contains("dominio (domain)"), "{arq}");
}

#[test]
fn integrar_git_instala_pre_commit_sem_pisar_no_alheio() {
    let p = projeto();
    let r = p.path();
    guarda(r, &["init"]);
    repo_com_base(r);
    let (c, s) = guarda(r, &["integrar", "git"]);
    assert_eq!(c, 0, "{s}");
    let hook = std::fs::read_to_string(r.join(".git/hooks/pre-commit")).unwrap();
    assert!(hook.contains("exec guarda check --alterados"), "{hook}");

    std::fs::write(r.join(".git/hooks/pre-commit"), "#!/bin/sh\necho meu\n").unwrap();
    let (c, s) = guarda(r, &["integrar", "git"]);
    assert_eq!(c, 2);
    assert!(s.contains("não é da guarda"), "{s}");
}

#[test]
fn integrar_codex_e_antigravity_escreve_os_arquivos_certos() {
    let p = projeto();
    let r = p.path();
    guarda(r, &["init", "--ia", "codex", "--ia", "antigravity"]);
    let md = std::fs::read_to_string(r.join("AGENTS.md")).unwrap();
    assert_eq!(
        md.matches("guarda:inicio").count(),
        1,
        "um bloco só para os dois: {md}"
    );
    let codex: Value =
        serde_json::from_str(&std::fs::read_to_string(r.join(".codex/hooks.json")).unwrap())
            .unwrap();
    assert_eq!(
        codex["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
        "guarda hook proteger --agente codex"
    );
    let ag: Value =
        serde_json::from_str(&std::fs::read_to_string(r.join(".agents/hooks.json")).unwrap())
            .unwrap();
    assert_eq!(
        ag["guarda"]["Stop"][0]["command"],
        "guarda hook final --agente antigravity"
    );
    let mcp: Value =
        serde_json::from_str(&std::fs::read_to_string(r.join(".agents/mcp_config.json")).unwrap())
            .unwrap();
    assert_eq!(mcp["mcpServers"]["guarda"]["command"], "guarda");
}

#[test]
fn explicar_pela_cli() {
    let p = projeto();
    let (c, s) = guarda(p.path(), &["explicar", "arq001"]);
    assert_eq!(c, 0);
    assert!(
        s.starts_with("ARQ001 Dependência proibida entre camadas"),
        "{s}"
    );
    let (c, s) = guarda(p.path(), &["explicar"]);
    assert_eq!(c, 0);
    assert!(s.contains("COV001") && s.contains("SML002"), "{s}");
}
