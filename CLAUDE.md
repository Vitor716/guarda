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
- `src/regras/`: regras por arquivo. `complexidade.rs` (CPX001/CPX005), `estrutura.rs`
  (métricas de função e classe: CPX002-004, CPX006-009, SML001, JV003; tem o `percorrer` com
  poda), `excecoes.rs` (SML004-006), `smells.rs` (SML003, SML007-009), `java.rs` (JV001/002/
  004/005), `kotlin.rs` (KT001-007), `supressao.rs` (SUP001 e `guarda:ignorar`), `testes.rs`
  (TST001-003, só para arquivos de `fontes.testes`), `extras.rs` (PRB001/002 e regras
  personalizadas por consulta do tree-sitter, carregadas uma vez por execução).
- `src/regras/fluxo.rs`: CND001-003 (guard clause, else depois de return, condição mista) e
  POL001/002 (cadeia de instanceof/is, switch repetido).
- `src/arquitetura.rs`: padrões ArchUnit, presets (o `modular` é montado dos pacotes do
  projeto: `preset_do_projeto`), detecção (`pontuar`: pacotes distintos + arquivos − violações
  + marcas do estilo), ARQ001, ARQ002 (ciclos entre fatias), ARQ003 (framework), ARQ004 e
  NEG001/002 (exceções criadas na camada de negócio; precisam da camada, por isso moram aqui,
  sobre `lang::excecoes_criadas` guardado no cache).
- `src/config.rs`: também `estende` (mescla de YAML) e `perfil`; use `cfg.desligada()` e
  `cfg.severidade()` em vez de ler `regras.desligar`/`regras.severidade` direto.
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
- `src/build.rs`: `integrar gradle` (script aplicado no build da raiz) e o trecho do Maven.
- `src/saida.rs`: formato compacto, JSON, SARIF e texto. `src/main.rs`: CLI (clap).
- `.github/workflows/`: `ci.yml` (fmt, clippy, testes em Linux e Windows) e `release.yml`
  (binários de 4 alvos ao empurrar uma tag `v*`).

## Regras do projeto

- Determinismo acima de tudo: nada de data, rede ou ordem de HashMap na saída; ordene antes
  de imprimir.
- Regra nova só se for verificável pela sintaxe e tiver origem rastreável (detekt, PMD, Sonar,
  ArchUnit). Registre em `modelo::REGRAS`; código nunca é reaproveitado.
- Antes de escrever uma regra, confira os nomes dos nós com `guarda arvore` (as gramáticas
  Java e Kotlin nomeiam diferente: `if_statement` × `if_expression`, `catch_clause` ×
  `catch_block`).
- Toda regra entra com teste; mudança de saída quebra `tests/cli.rs` de propósito.
- Regra nova entra como aviso (`severidade: Aviso`, `alvo` = a severidade final) e precisa de
  uma explicação em `explicar.rs`. Antes de promover a erro, meça num projeto real e revise
  amostras de cada achado (`docs/fase-N.md` registra a revisão).
- Binário do PATH: `cargo install --path . --locked` só com o trabalho testado; os hooks dos
  projetos reais usam ele. Durante o desenvolvimento, teste com `target/release/guarda.exe`.
- Medição em projeto real: sempre em cópia dos fontes (scratchpad), nunca no original.
- Hook nunca bloqueia por erro próprio (só por decisão): erro interno é exit 1 no Claude e
  no Codex e a resposta de liberar no Antigravity.
- Protocolo de agente muda: antes de mexer em `ler_evento`/`responder`, confira a
  documentação oficial do agente e registre a data no comentário do topo de `hook.rs`.
- Cada processo git custa dezenas de ms no Windows: rode em paralelo e filtre por arquivo.
- Textos de mensagem e dica em português, curtos, com a correção na dica.
