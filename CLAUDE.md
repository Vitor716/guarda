# CLAUDE.md

Linter determinístico para Kotlin e Java em Rust. Documentação para humanos no `README.md`;
medições de cada fase em `docs/`.

## Comandos

```bash
cargo test                              # unitários + ponta a ponta (tests/cli.rs)
cargo run -- arvore caminho/Arquivo.kt  # árvore sintática: use antes de escrever uma regra
cargo run --release -- medir <projeto>  # portão da fase em projeto real
```

## Mapa do código

- `src/lang.rs`: parser tree-sitter, utilitários de árvore, pacote, imports, linhas de código.
- `src/regras/`: regras por arquivo (`complexidade.rs` = CPX001/CPX005, `supressao.rs` = SUP001
  e `guarda:ignorar`, KT001 em `mod.rs`).
- `src/arquitetura.rs`: padrões ArchUnit, presets, detecção (`pontuar`) e ARQ001.
- `src/motor.rs`: varre, analisa em paralelo, monta o índice, aplica supressões e ordena.
- `src/baseline.rs`: impressão digital, catraca, comparação.
- `src/saida.rs`: formato compacto e JSON. `src/main.rs`: CLI (clap).

## Regras do projeto

- Determinismo acima de tudo: nada de data, rede ou ordem de HashMap na saída; ordene antes
  de imprimir.
- Regra nova só se for verificável pela sintaxe e tiver origem rastreável (detekt, PMD, Sonar,
  ArchUnit). Registre em `modelo::REGRAS`; código nunca é reaproveitado.
- Antes de escrever uma regra, confira os nomes dos nós com `guarda arvore` (as gramáticas
  Java e Kotlin nomeiam diferente: `if_statement` × `if_expression`, `catch_clause` ×
  `catch_block`).
- Toda regra entra com teste; mudança de saída quebra `tests/cli.rs` de propósito.
- Textos de mensagem e dica em português, curtos, com a correção na dica.
