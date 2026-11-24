//! Fase 1: schema, integração com o Claude Code, hooks e cache incremental.

mod comum;

use comum::{git, guarda, projeto, rodar};
use serde_json::{Value, json};
use std::path::Path;

const CALCULO: &str = "src/main/kotlin/com/app/application/Calculo.kt";

fn init(raiz: &Path, extra: &[&str]) {
    let mut args = vec!["init"];
    args.extend_from_slice(extra);
    let (c, s) = guarda(raiz, &args);
    assert_eq!(c, 0, "{s}");
}

/// Acrescenta um `!!` novo no Calculo.kt (violação nova, fora do baseline).
fn sujar(raiz: &Path) {
    let p = raiz.join(CALCULO);
    let t = std::fs::read_to_string(&p)
        .unwrap()
        .replace("val a = n!!", "val z = n!!.cru()\n        val a = n!!");
    std::fs::write(p, t).unwrap();
}

fn hook(raiz: &Path, evento: &str, entrada: Value) -> (i32, String, String) {
    rodar(raiz, &["hook", evento], Some(&entrada.to_string()))
}

fn caminho(raiz: &Path, rel: &str) -> String {
    raiz.join(rel).to_string_lossy().to_string()
}

#[test]
fn schema_tem_enums_e_descricoes() {
    let p = projeto();
    let (c, s) = guarda(p.path(), &["schema"]);
    assert_eq!(c, 0);
    let v: Value = serde_json::from_str(&s).unwrap();
    assert_eq!(v["title"], "guarda.yml");
    let props = &v["properties"];
    assert!(props["arquitetura"].is_object() || v["$defs"]["Arquitetura"].is_object());
    let arq = &v["$defs"]["Arquitetura"]["properties"]["preset"]["enum"];
    assert!(
        arq.as_array().unwrap().iter().any(|x| x == "hexagonal"),
        "{arq}"
    );
    assert_eq!(v["$defs"]["Fontes"]["additionalProperties"], false);
}

#[test]
fn init_com_ia_claude_escreve_contexto_hooks_e_schema() {
    let p = projeto();
    std::fs::write(p.path().join("CLAUDE.md"), "# Meu projeto\n").unwrap();
    init(p.path(), &["--ia", "claude"]);

    let yml = std::fs::read_to_string(p.path().join("guarda.yml")).unwrap();
    assert!(yml.starts_with("# yaml-language-server: $schema=./.guarda/guarda.schema.json\n"));
    assert!(p.path().join(".guarda/guarda.schema.json").is_file());

    let md = std::fs::read_to_string(p.path().join("CLAUDE.md")).unwrap();
    assert!(md.starts_with("# Meu projeto\n"), "{md}");
    assert!(md.contains("Arquitetura hexagonal"), "{md}");
    assert!(
        md.contains("  - dominio (domain): importa nenhuma outra camada"),
        "{md}"
    );
    assert!(
        md.contains("  - adaptadores-saida (infra): importa dominio, aplicacao"),
        "{md}"
    );

    let s: Value = serde_json::from_str(
        &std::fs::read_to_string(p.path().join(".claude/settings.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        s["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
        "guarda hook proteger"
    );
    assert_eq!(
        s["hooks"]["PostToolUse"][0]["hooks"][0]["command"],
        "guarda hook pos-edicao"
    );
    assert_eq!(
        s["hooks"]["Stop"][0]["hooks"][0]["command"],
        "guarda hook final"
    );

    // Rodar de novo não duplica nada.
    let (c, _) = guarda(p.path(), &["integrar", "claude"]);
    assert_eq!(c, 0);
    let md2 = std::fs::read_to_string(p.path().join("CLAUDE.md")).unwrap();
    assert_eq!(md, md2);
}

#[test]
fn proteger_recusa_config_baseline_e_comandos() {
    let p = projeto();
    init(p.path(), &[]);
    let r = p.path();
    let edit = |rel: &str| {
        json!({"tool_name": "Edit", "cwd": caminho(r, ""),
        "tool_input": {"file_path": caminho(r, rel), "old_string": "a", "new_string": "b"}})
    };

    let (c, _, e) = hook(r, "proteger", edit("guarda.yml"));
    assert_eq!(c, 2, "{e}");
    assert!(e.contains("guarda.yml é protegido"), "{e}");
    let (c, _, _) = hook(r, "proteger", edit(".guarda/baseline.json"));
    assert_eq!(c, 2);
    let (c, _, e) = hook(r, "proteger", edit(CALCULO));
    assert_eq!(c, 0, "{e}");

    let bash = |cmd: &str| json!({"tool_name": "Bash", "cwd": caminho(r, ""), "tool_input": {"command": cmd}});
    let (c, _, e) = hook(r, "proteger", bash("guarda baseline gerar"));
    assert_eq!(c, 2);
    assert!(e.contains("decisão humana"), "{e}");
    assert_eq!(hook(r, "proteger", bash("rm -rf .guarda")).0, 2);
    assert_eq!(hook(r, "proteger", bash("guarda check --alterados")).0, 0);
}

#[test]
fn proteger_ignora_projeto_sem_guarda() {
    let p = projeto();
    let r = p.path();
    let e = json!({"tool_name": "Edit", "cwd": caminho(r, ""),
        "tool_input": {"file_path": caminho(r, "guarda.yml"), "old_string": "a", "new_string": "b"}});
    assert_eq!(hook(r, "proteger", e).0, 0);
}

#[test]
fn pos_edicao_cobra_so_o_arquivo_editado() {
    let p = projeto();
    init(p.path(), &[]);
    let r = p.path();
    let evento = json!({"tool_name": "Edit", "cwd": caminho(r, ""),
        "tool_input": {"file_path": caminho(r, CALCULO)}, "tool_response": {}});

    let (c, _, e) = hook(r, "pos-edicao", evento.clone());
    assert_eq!(c, 0, "legado congelado não cobra: {e}");

    sujar(r);
    let (c, _, e) = hook(r, "pos-edicao", evento);
    assert_eq!(c, 2);
    assert!(e.contains("  7 KT001 uso de !!"), "{e}");
    assert!(e.contains("Corrija agora"), "{e}");
}

#[test]
fn final_segura_ate_o_limite_e_depois_libera_com_relatorio() {
    let p = projeto();
    let r = p.path();
    init(r, &[]);
    git(r, &["init", "-q"]);
    git(r, &["add", "-A"]);
    git(r, &["commit", "-q", "-m", "base"]);

    let stop = json!({"session_id": "s1", "cwd": caminho(r, ""), "hook_event_name": "Stop",
        "stop_hook_active": false});
    assert_eq!(
        hook(r, "final", stop.clone()).0,
        0,
        "sem mudança, sem bloqueio"
    );

    sujar(r);
    for n in 1..=3 {
        let (c, _, e) = hook(r, "final", stop.clone());
        assert_eq!(c, 2, "tentativa {n}: {e}");
        assert!(e.contains(&format!("tentativa {n} de 3")), "{e}");
    }
    let (c, o, _) = hook(r, "final", stop.clone());
    assert_eq!(c, 0);
    let msg: Value = serde_json::from_str(o.trim()).unwrap();
    assert!(
        msg["systemMessage"].as_str().unwrap().contains("liberei"),
        "{o}"
    );
    assert!(r.join(".guarda/cache/final.txt").is_file());

    // A mesma violação já liberada não prende a IA de novo nesta sessão.
    assert_eq!(hook(r, "final", stop.clone()).0, 0);
    // Outra sessão começa do zero.
    let outra = json!({"session_id": "s2", "cwd": caminho(r, "")});
    assert_eq!(hook(r, "final", outra).0, 2);
}

#[test]
fn grd001_avisa_quando_a_config_muda() {
    let p = projeto();
    let r = p.path();
    init(r, &[]);
    git(r, &["init", "-q"]);
    git(r, &["add", "-A"]);
    git(r, &["commit", "-q", "-m", "base"]);
    let yml = r.join("guarda.yml");
    let t = std::fs::read_to_string(&yml).unwrap() + "# afrouxei\n";
    std::fs::write(&yml, t).unwrap();

    let (c, s) = guarda(r, &["check", "--alterados"]);
    assert_eq!(c, 0, "aviso não reprova: {s}");
    assert!(s.contains("GRD001 guarda.yml mudou"), "{s}");

    let (c, o, _) = hook(
        r,
        "final",
        json!({"session_id": "s", "cwd": caminho(r, "")}),
    );
    assert_eq!(c, 0);
    assert!(o.contains("GRD001"), "{o}");
}

#[test]
fn cache_acompanha_conteudo_e_limites() {
    let p = projeto();
    let r = p.path();
    init(r, &[]);
    assert!(r.join(".guarda/cache/arquivos.json").is_file());
    assert_eq!(
        std::fs::read_to_string(r.join(".guarda/cache/.gitignore")).unwrap(),
        "*\n"
    );

    let (c, _) = guarda(r, &["check"]);
    assert_eq!(c, 0);
    sujar(r);
    let (c, s) = guarda(r, &["check"]);
    assert_eq!(c, 1, "arquivo alterado sai do cache: {s}");

    // Limite novo invalida o cache: a função de 6 linhas passa a ser longa.
    let yml = r.join("guarda.yml");
    let t = std::fs::read_to_string(&yml)
        .unwrap()
        .replace("linhas-funcao: 60", "linhas-funcao: 3");
    std::fs::write(&yml, t).unwrap();
    let (_, s) = guarda(r, &["check", "--sem-baseline"]);
    assert!(s.contains("CPX005"), "{s}");
}
