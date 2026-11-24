//! Fase 4: regras de fluxo (guard clause, condição, polimorfismo) e exceções de negócio.

mod comum;

use comum::{guarda, projeto};

#[test]
fn excecoes_na_camada_de_negocio() {
    let p = projeto();
    let r = p.path();
    std::fs::write(
        r.join("src/main/java/com/app/domain/Conta.java"),
        "package com.app.domain;

import org.springframework.web.server.ResponseStatusException;

class Conta {
    long saldo;

    void debitar(long v) {
        if (v > saldo) throw new IllegalStateException(\"saldo insuficiente\");
        if (v < 0) throw new ResponseStatusException(BAD_REQUEST);
        saldo -= v;
    }
}
",
    )
    .unwrap();
    std::fs::write(
        r.join("src/main/java/com/app/infra/Cliente.java"),
        "package com.app.infra;

class Cliente {
    void chamar(int v) {
        if (v < 0) throw new IllegalArgumentException(\"v\");
    }
}
",
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
    assert!(
        s.contains("  9 NEG002 IllegalStateException genérica em dominio"),
        "{s}"
    );
    assert!(
        s.contains(" 10 NEG001 ResponseStatusException em dominio"),
        "{s}"
    );
    // Fora da camada de negócio (infra), IllegalArgumentException não é regra de negócio.
    assert!(!s.contains("Cliente.java\n  5 NEG"), "{s}");
}

#[test]
fn regras_de_fluxo_pela_cli() {
    let p = projeto();
    let r = p.path();
    std::fs::write(
        r.join("src/main/java/com/app/domain/Frete.java"),
        "package com.app.domain;

class Frete {
    int prazo(Tipo t, boolean urgente, boolean vip) {
        if (urgente && vip || t == null) {
            return 1;
        } else {
            return 5;
        }
    }
}
",
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
    assert!(s.contains("  5 CND003 condição mistura && e ||"), "{s}");
    assert!(s.contains("  7 CND002 else depois de return"), "{s}");
    let (c, e) = guarda(r, &["explicar", "CND001"]);
    assert_eq!(c, 0);
    assert!(e.contains("guard clause"), "{e}");
}

#[test]
fn baseline_aceitar_exige_motivo_e_o_guarda() {
    let p = projeto();
    let r = p.path();
    let alvo = "src/main/kotlin/com/app/application/Calculo.kt";
    let (c, s) = guarda(
        r,
        &["baseline", "aceitar", "KT001", alvo, "--motivo", "curto"],
    );
    assert_ne!(c, 0, "{s}");
    assert!(s.contains("--motivo precisa explicar"), "{s}");

    let motivo = "legado do cálculo; refatorar no épico de tarifas";
    let (c, s) = guarda(
        r,
        &["baseline", "aceitar", "kt001", alvo, "--motivo", motivo],
    );
    assert_eq!(c, 0, "{s}");
    assert!(s.contains("2 violação(ões) de KT001 em src/main/kotlin/com/app/application/Calculo.kt aceita(s); 0 já"), "{s}");
    let (_, s) = guarda(
        r,
        &["baseline", "aceitar", "KT001", alvo, "--motivo", motivo],
    );
    assert!(s.contains("0 violação(ões) de KT001"), "{s}");

    let (_, s) = guarda(r, &["check", "--max", "100", "--formato", "compacto"]);
    assert!(!s.contains("KT001"), "{s}");
    let (_, _) = guarda(r, &["check", "--atualizar-baseline"]);
    let b = std::fs::read_to_string(r.join(".guarda/baseline.json")).unwrap();
    assert!(b.contains(&format!("\"motivo\":\"{motivo}\"")), "{b}");
    assert_eq!(b.matches("KT001").count(), 2, "{b}");
}

#[test]
fn integrar_maven_imprime_o_plugin() {
    let p = projeto();
    let (c, s) = guarda(p.path(), &["integrar", "maven"]);
    assert_eq!(c, 0, "{s}");
    assert!(
        s.contains("<artifactId>exec-maven-plugin</artifactId>"),
        "{s}"
    );
    assert!(s.contains("<phase>verify</phase>"), "{s}");
    assert!(!p.path().join("pom.xml").exists());
}
