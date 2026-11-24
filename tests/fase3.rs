//! Fase 3 (lote 2): código de teste, proibições do projeto, perfil e SARIF.

mod comum;

use comum::{guarda, projeto};
use serde_json::Value;

#[test]
fn regras_de_teste_rodam_em_src_test() {
    let p = projeto();
    let r = p.path();
    std::fs::write(
        r.join("src/test/java/com/app/CalculoTest.java"),
        "package com.app;\n\nclass CalculoTest {\n    @Test void soma() { new Calculo().total(null); }\n    @Disabled @Test void lento() { Thread.sleep(10); assertTrue(true); }\n}\n",
    )
    .unwrap();
    let (_, s) = guarda(
        r,
        &[
            "check",
            "--sem-baseline",
            "--max",
            "100",
            "--formato",
            "compacto",
        ],
    );
    assert!(s.contains("src/test/java/com/app/CalculoTest.java"), "{s}");
    assert!(s.contains("  4 TST001 teste sem asserção"), "{s}");
    assert!(s.contains("  5 TST003 @Disabled sem motivo"), "{s}");
    assert!(s.contains("  5 TST002 Thread.sleep em teste"), "{s}");
    // Regra de produção não roda no teste (o KT001/SUP001 do fixture são de src/main).
    assert!(!s.contains("CalculoTest.java\n  4 CPX"), "{s}");
}

#[test]
fn proibir_pelo_guarda_yml() {
    let p = projeto();
    let r = p.path();
    std::fs::write(
        r.join("guarda.yml"),
        "arquitetura:\n  preset: nenhum\nregras:\n  proibir:\n    imports: [\"java.util.**\"]\n    tipos-em-pacote:\n      - tipo: Nota\n        pacote: \"..application..\"\n        dica: use o DTO\n",
    )
    .unwrap();
    let (c, s) = guarda(r, &["check", "--sem-baseline", "--max", "100"]);
    assert_eq!(c, 1, "{s}");
    assert!(
        s.contains("  4 PRB001 import proibido (java.util.**)"),
        "{s}"
    );
    assert!(
        s.contains("PRB002 Nota proibido em ..application.. | use o DTO"),
        "{s}"
    );
}

#[test]
fn perfil_minimo_desliga_avisos_e_rigoroso_promove() {
    let p = projeto();
    let r = p.path();
    std::fs::write(r.join("guarda.yml"), "perfil: minimo\n").unwrap();
    let (_, s) = guarda(r, &["check", "--sem-baseline", "--max", "100"]);
    assert!(!s.contains("ARQ002"), "aviso desligado no mínimo: {s}");
    assert!(s.contains("KT001"), "{s}");

    std::fs::write(r.join("guarda.yml"), "perfil: rigoroso\n").unwrap();
    let (_, s) = guarda(
        r,
        &[
            "check",
            "--sem-baseline",
            "--max",
            "100",
            "--formato",
            "json",
        ],
    );
    let arq002: Vec<Value> = s
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["codigo"] == "ARQ002")
        .collect();
    assert!(!arq002.is_empty(), "{s}");
    assert!(arq002.iter().all(|v| v["severidade"] == "erro"), "{s}");
}

#[test]
fn sarif_pela_cli() {
    let p = projeto();
    let (_, s) = guarda(p.path(), &["check", "--sem-baseline", "--formato", "sarif"]);
    let v: Value = serde_json::from_str(&s).unwrap();
    assert_eq!(v["version"], "2.1.0");
    let ids: Vec<&str> = v["runs"][0]["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["ruleId"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"KT001") && ids.contains(&"ARQ001"), "{ids:?}");
}
