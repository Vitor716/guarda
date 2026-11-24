# AGENTS.md

Instruções para agentes de IA (Codex, Antigravity, Claude Code, Cursor, Copilot e outros)
trabalhando **neste repositório (`guarda`)** ou usando a ferramenta `guarda` em outros projetos.

> Se você é um agente que vai **usar** a `guarda` para validar um projeto Java/Kotlin, leia
> primeiro [docs/guia-ia.md](docs/guia-ia.md).

## O que é este repositório

Linter determinístico para **Kotlin e Java** escrito em Rust (tree-sitter + blake3 + rayon),
com **94 regras**, baseline com catraca, cobertura JaCoCo/Kover de código novo, duplicação
Karp-Rabin, hooks de agentes e servidor MCP. Documentação para humanos em [README.md](README.md)
e medições em projetos reais em `docs/fase-*.md`.

## Comandos de desenvolvimento

```bash
cargo fmt --check                       # formatação (exigida no CI)
cargo clippy --all-targets -- -D warnings  # lint sem warnings (exigido no CI)
cargo test                              # unitários (104) + ponta a ponta (tests/*.rs)
cargo run -- arvore caminho/Arquivo.kt  # imprime AST do tree-sitter (use antes de criar regra)
cargo run --release -- medir <copia>    # mede parse, tempo e violações em cópia de projeto real
cargo install --path . --locked         # atualiza `guarda` no PATH (só após testar tudo!)
```

## Mapa do código

- `src/lang.rs`: parser tree-sitter (`tree-sitter-java`, `tree-sitter-kotlin-ng`), pacote,
  imports, anotações de tipo, exceções criadas, entidades JPA / `@RequestBody` e linhas de código.
- `src/regras/`: regras por arquivo (`mod.rs` orquestra produção e teste):
  - `complexidade.rs`: `CPX001`, `CPX005`.
  - `estrutura.rs`: `CPX002–004`, `CPX006–009`, `SML001` (catraca por WMC), `JV003`, `percorrer`.
  - `excecoes.rs`: `SML004–006`.
  - `smells.rs`: `SML003`, `SML007–009`.
  - `fluxo.rs`: `CND001–004`, `POL001–002`.
  - `seguranca.rs`: `SEC001–016`, `SEC019–020` (com análise de fluxo intra-função e filtro de falsos positivos em logs/preflight/redirect relativo).
  - `spring.rs`: `SPR001–003`, `SPR005–006` (`SPR004` roda no motor com o índice de `@Entity`).
  - `bugs.rs`: `BUG001–009`.
  - `praticas.rs`: `SML010–011`, `KT008`, `JV006`.
  - `sintaxe.rs`: utilitários comuns de AST (chamadas Java/Kotlin, literais, strings montadas, detecção de segredos e URLs com credencial).
  - `java.rs` (`JV001–002`, `JV004–005`), `kotlin.rs` (`KT001–007`), `supressao.rs` (`SUP001`, `guarda:ignorar`), `testes.rs` (`TST001–004`, só em `fontes.testes`), `extras.rs` (`PRB001–002` e regras YAML customizadas).
- `src/recursos.rs`: verificação de `application*.properties|yml|yaml` e arquivos de segredo (`.env`, `.key`, `.pem`, keystores) para `SEC001`, `SEC017`, `SEC018`.
- `src/arquitetura.rs`: padrões ArchUnit, presets (`hexagonal`, `clean`, `layered`, `mvvm`, `modular`), pontuação de detecção, `ARQ001–004` e `NEG001–002`.
- `src/baseline.rs`: impressão digital estável, catraca determinística (inclusive em sobrecargas), `regerar` (preserva motivos e aceita filtro por regras), `aceitar` (atualiza catracas) e `comparar` (limpa arquivos removidos em escopo total).
- `src/motor.rs`: varredura paralela com `rayon`, `cache.rs`, índice do projeto, `SPR004`, `recursos_verificados`, duplicação e ordenação.
- `src/gate.rs`: motor + cobertura + duplicação + `GRD001` + baseline + gate.
- `src/cobertura.rs` (`COV000–003`), `src/codigo_novo.rs`, `src/duplicacao.rs` (`SML002`).
- `src/config.rs`: `guarda.yml` (`estende`, `perfil`, `schemars` JSON Schema). Use `cfg.desligada()` e `cfg.severidade()`.
- `src/hook.rs`: `proteger`, `pos-edicao` (código + recursos) e `final` para Claude, Codex e Antigravity.
- `src/integracao.rs`, `src/build.rs`, `src/mcp.rs`, `src/explicar.rs`, `src/saida.rs`, `src/main.rs`.

## Invariantes obrigatórias

1. **Determinismo absoluto**: nenhuma data, rede ou ordem de `HashMap` pode afetar a saída; ordene sempre antes de imprimir ou salvar.
2. **Catálogo e explicação em sincronia**: toda regra deve estar em `modelo::REGRAS` e ter entrada correspondente em `explicar::EXPLICACOES` (o teste `toda_regra_tem_explicacao` garante). Nunca reaproveite um código removido.
3. **Regra nova entra como `Aviso`** (`severidade: Aviso`, `alvo` = severidade final) com origem rastreável (Sonar, detekt, PMD, ArchUnit, Spring) e só é promovida após medição e revisão de falsos positivos em cópias de projetos reais registradas em `docs/fase-*.md`.
4. **Confira a AST antes de escrever regra**: use `cargo run -- arvore <arquivo>` (Java e Kotlin nomeiam nós de forma diferente).
5. **Medições sempre em cópia (scratchpad)**, nunca alterando projetos reais durante testes.
6. **Hooks nunca bloqueiam por erro interno da própria guarda** — apenas por decisão explícita.
7. **Textos em português**, curtos e acionáveis (a correção vem sempre na `dica`).
8. **Qualidade antes de finalizar**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` e `cargo test` devem passar com zero avisos.
