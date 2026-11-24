//! Testes de ponta a ponta: roda o binário sobre uma cópia de `tests/fixtures/hex`.

mod comum;

use comum::{guarda, projeto};

/// Tira o tempo do rodapé, a única parte não determinística da saída.
fn sem_tempo(s: &str) -> String {
    s.lines()
        .map(|l| match l.rfind(" · ") {
            Some(i) if l.starts_with('✓') || l.starts_with('✗') => l[..i].to_string(),
            _ => l.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn check_sem_config_detecta_e_reporta() {
    let p = projeto();
    let (codigo, saida) = guarda(p.path(), &["check", "--sem-baseline", "--max", "100"]);
    assert_eq!(codigo, 1, "{saida}");
    let esperado = "\
src/main/java/com/app/domain/Nota.java
  3 ARQ001 dominio importa adaptadores-saida (HttpClient) | dependa de uma interface (porta) em dominio e implemente em adaptadores-saida
  7 SUP001 @SuppressWarnings sem motivo | comente o motivo na mesma linha ou na linha de cima
src/main/java/com/app/domain/Quebrado.java
  4 ERR000 o parser não leu o arquivo inteiro (1 trecho(s) com erro) | se o código compila, é limite da gramática: registre o caso
src/main/kotlin/com/app/application/Calculo.kt
  7 KT001 uso de !! | troque por ?:, ?.let ou requireNotNull com mensagem
 10 SUP001 guarda:ignorar sem código ou sem motivo | escreva // guarda:ignorar CODIGO motivo: <por quê>
 11 KT001 uso de !! | troque por ?:, ?.let ou requireNotNull com mensagem
✗ 6 erros em 3 arquivos";
    assert_eq!(sem_tempo(&saida).trim_end(), esperado);
}

#[test]
fn init_congela_e_check_cobra_so_o_novo() {
    let p = projeto();
    let (codigo, saida) = guarda(p.path(), &["init"]);
    assert_eq!(codigo, 0, "{saida}");
    assert!(saida.contains("arquitetura: hexagonal"), "{saida}");
    assert!(
        saida.contains("baseline: 6 violações congeladas em 4 regras"),
        "{saida}"
    );
    assert!(p.path().join("guarda.yml").exists());

    let (codigo, saida) = guarda(p.path(), &["check"]);
    assert_eq!(codigo, 0, "{saida}");
    assert!(
        saida.starts_with("✓ guarda 0 erros · baseline 6"),
        "{saida}"
    );

    // Código novo com !! é cobrado; o legado (que mudou de linha) continua congelado.
    let rel = "src/main/kotlin/com/app/application/Calculo.kt";
    let calc = p.path().join(rel);
    let novo = std::fs::read_to_string(&calc)
        .unwrap()
        .replace("val a = n!!", "val z = n!!.cru()\n        val a = n!!");
    std::fs::write(&calc, novo).unwrap();
    let (codigo, saida) = guarda(p.path(), &["check", "--arquivos", rel]);
    assert_eq!(codigo, 1, "{saida}");
    assert!(saida.contains("  7 KT001 uso de !!"), "{saida}");
    assert!(saida.contains("✗ 1 erro em 1 arquivo"), "{saida}");
}

#[test]
fn saida_e_deterministica() {
    let p = projeto();
    let (_, a) = guarda(p.path(), &["check", "--sem-baseline", "--formato", "json"]);
    let (_, b) = guarda(p.path(), &["check", "--sem-baseline", "--formato", "json"]);
    assert_eq!(a, b);
    assert_eq!(a.lines().count(), 6);
}

#[test]
fn config_invalida_aponta_linha() {
    let p = projeto();
    std::fs::write(
        p.path().join("guarda.yml"),
        "versao: 1\nlimites:\n  linhas-fucao: 3\n",
    )
    .unwrap();
    let (codigo, saida) = guarda(p.path(), &["check"]);
    assert_eq!(codigo, 2);
    assert!(saida.contains("guarda.yml:3"), "{saida}");
}
