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

#[test]
fn baseline_status_regerar_parcial_e_recurso_corrigido() {
    let p = projeto();
    let r = p.path();
    std::fs::create_dir_all(r.join("src/main/resources")).unwrap();
    let props = r.join("src/main/resources/application.properties");
    std::fs::write(&props, "spring.datasource.password=root1234\n").unwrap();

    let (c, s) = guarda(r, &["init"]);
    assert_eq!(c, 0, "{s}");

    // Aceita KT001 em Calculo.kt só na linha 7 (teste de --linha e preservação de motivo).
    let calc = "src/main/kotlin/com/app/application/Calculo.kt";
    let (c, _) = guarda(
        r,
        &[
            "baseline",
            "aceitar",
            "KT001",
            calc,
            "--linha",
            "7",
            "--motivo",
            "decisão registrada para auditoria",
        ],
    );
    assert_eq!(c, 0);

    // Corrige o segredo em application.properties: deve aparecer em `baseline status` e sair no `--atualizar-baseline`.
    std::fs::write(&props, "spring.datasource.password=${DB_PASS}\n").unwrap();
    let (c, st) = guarda(r, &["baseline", "status"]);
    assert_eq!(c, 0, "{st}");
    assert!(st.contains("corrigidas (podem sair): 1"), "{st}");

    let (c, _) = guarda(r, &["check", "--atualizar-baseline"]);
    assert_eq!(c, 0);
    let bl = std::fs::read_to_string(r.join(".guarda/baseline.json")).unwrap();
    assert!(
        !bl.contains("SEC001"),
        "recurso corrigido sai do baseline: {bl}"
    );

    // Deleta um arquivo legado com violação e verifica se sai no `--atualizar-baseline`.
    std::fs::remove_file(r.join("src/main/java/com/app/domain/Quebrado.java")).unwrap();
    let (c, st) = guarda(r, &["baseline", "status"]);
    assert_eq!(c, 0, "{st}");
    assert!(st.contains("1 em arquivos removidos"), "{st}");
    let (c, _) = guarda(r, &["check", "--atualizar-baseline"]);
    assert_eq!(c, 0);
    let bl = std::fs::read_to_string(r.join(".guarda/baseline.json")).unwrap();
    assert!(!bl.contains("Quebrado.java"), "{bl}");

    // `baseline gerar --regras SEC001` não congela um KT001 novo adicionado no código.
    std::fs::write(&props, "spring.datasource.password=novaSenha99\n").unwrap();
    let calc_path = r.join(calc);
    let sujo = std::fs::read_to_string(&calc_path)
        .unwrap()
        .replace("val a = n!!", "val z = n!!.cru()\n        val a = n!!");
    std::fs::write(&calc_path, sujo).unwrap();
    let (c, out) = guarda(r, &["baseline", "gerar", "--regras", "SEC001"]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains("+1 novas"), "{out}");

    // O KT001 novo continua sendo cobrado, enquanto SEC001 foi congelado.
    let (c, out) = guarda(r, &["check", "--formato", "compacto"]);
    assert_eq!(c, 1, "{out}");
    assert!(out.contains("KT001"), "{out}");
    assert!(!out.contains("SEC001"), "{out}");
}

#[test]
fn novas_regras_spr006_bug009_e_tst004() {
    let p = projeto();
    let r = p.path();
    std::fs::create_dir_all(r.join("src/main/java/com/app/endpoint")).unwrap();
    std::fs::write(
        r.join("src/main/java/com/app/endpoint/PedidoController.java"),
        "package com.app.endpoint;\n@RestController\nclass PedidoController {\n    @Transactional\n    void criar(String s) { assert s != null; }\n}\n",
    )
    .unwrap();
    std::fs::write(
        r.join("src/test/java/com/app/TautologiaTest.java"),
        "package com.app;\nclass TautologiaTest {\n    @Test void ok() { assertTrue(true); }\n}\n",
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
        s.contains("SPR006 @Transactional em controlador HTTP"),
        "{s}"
    );
    assert!(s.contains("BUG009 assert em código de produção"), "{s}");
    assert!(
        s.contains("TST004 assertTrue(true) é sempre verdadeiro"),
        "{s}"
    );
}
