# CLAUDE.md

Linter determinístico para Kotlin e Java em Rust. Documentação para humanos no `README.md`;
medições de cada fase em `docs/`.

## Comandos

```bash
cargo test                              # unitários + ponta a ponta (tests/cli.rs, tests/fase1.rs)
cargo run -- arvore caminho/Arquivo.kt  # árvore sintática: use antes de escrever uma regra
cargo run --release -- medir <projeto>  # portão da fase em projeto real
cargo install --path . --locked         # atualiza o `guarda` do PATH (os hooks usam esse)
```

## Mapa do código

- `src/lang.rs`: parser tree-sitter, utilitários de árvore, pacote, imports, linhas de código.
- `src/regras/`: regras por arquivo (`complexidade.rs` = CPX001/CPX005, `supressao.rs` = SUP001
  e `guarda:ignorar`, KT001 em `mod.rs`).
- `src/arquitetura.rs`: padrões ArchUnit, presets, detecção (`pontuar`) e ARQ001.
- `src/motor.rs`: varre, analisa em paralelo (ou tira do `cache.rs`), monta o índice, aplica
  supressões e ordena.
- `src/gate.rs`: motor + baseline + gate + GRD001; usado pelo `check` e pelos hooks.
- `src/baseline.rs`: impressão digital, catraca, comparação.
- `src/hook.rs`: hooks (proteger, pos-edicao, final). Cada agente (Claude, Codex,
  Antigravity) tem só um leitor de evento (`ler_evento`) e uma tabela de resposta
  (`responder`); a decisão é uma só. Agente novo = esses dois pontos + `integracao.rs`.
- `src/integracao.rs`: `contexto`, blocos no CLAUDE.md/AGENTS.md, hooks e MCP de cada agente,
  pre-commit do git, schema.
- `src/mcp.rs`: servidor MCP por stdio (JSON-RPC escrito à mão; stdout só com protocolo).
- `src/explicar.rs`: explicação longa de cada regra (todo código de `modelo::REGRAS` precisa de uma).
- `src/codigo_novo.rs`: linhas novas pelo `git diff -U0` contra o ancestral comum.
- `src/cobertura.rs`: XML do JaCoCo/Kover, COV000-003. `src/duplicacao.rs`: tokens e Karp-Rabin.
- `src/config.rs`: `guarda.yml` (serde + schemars). Campo novo exige doc comment (vira a
  descrição no JSON Schema) e muda `impressao_regras` se afetar regra por arquivo.
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
- Hook nunca bloqueia por erro próprio (só por decisão): erro interno é exit 1 no Claude e
  no Codex e a resposta de liberar no Antigravity.
- Protocolo de agente muda: antes de mexer em `ler_evento`/`responder`, confira a
  documentação oficial do agente e registre a data no comentário do topo de `hook.rs`.
- Cada processo git custa dezenas de ms no Windows: rode em paralelo e filtre por arquivo.
- Textos de mensagem e dica em português, curtos, com a correção na dica.
