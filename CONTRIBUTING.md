# Contribuindo

Obrigado por ajudar a melhorar a `guarda`. A ferramenta vira portão de qualidade em projetos
de outras pessoas, então cada mudança precisa ser **determinística, testada e medida**.

## Antes de começar

- **Falso positivo ou regra nova?** Abra uma issue pelos modelos (*Falso positivo*, *Nova
  regra*, *Bug*). Um falso positivo com o trecho mínimo de código é a contribuição mais útil.
- Para mexer em regras, siga a receita de [docs/novas-regras.md](docs/novas-regras.md):
  critérios de aceite, escolha do código, armadilhas das gramáticas e calibração em projetos reais.
- Agentes de IA: leiam o [AGENTS.md](AGENTS.md) (mapa do código e invariantes).

## Ambiente

- Rust estável (o mínimo está em `rust-version` no `Cargo.toml`).
- No Windows, o Visual Studio Build Tools com o workload C++ (as gramáticas do tree-sitter são C).
- Python 3 só para os scripts de calibração em `tools/`.

```bash
cargo build
cargo test                                   # unitários + ponta a ponta (tests/*.rs)
cargo run -- arvore caminho/Arquivo.kt       # árvore sintática, para escrever regra
cargo run --release -- medir <copia>         # tempo, parse e violações num projeto
```

## Regras do projeto

1. **Determinismo**: nada de data, rede ou ordem de `HashMap` na saída. Ordene antes de imprimir.
2. **Regra nova entra como aviso** (`severidade: Aviso`, `alvo` = severidade final), com origem
   rastreável (Sonar, PMD, detekt, Find Security Bugs, ArchUnit) e explicação em `src/explicar.rs`.
3. **Códigos são estáveis**: nunca reaproveite um código removido; não mude o `trecho` de uma
   regra existente sem necessidade (ele faz parte da impressão digital do baseline).
4. **Medição em cópia**: nunca rode a calibração no projeto original de alguém.
5. **Textos em português**, curtos, com a correção na dica.

## Pull request

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` e `cargo test` passando (o CI
  roda em Linux e Windows).
- Mudou o catálogo? Rode `GUARDA_ATUALIZAR_DOCS=1 cargo test catalogo` para regerar
  [docs/regras.md](docs/regras.md).
- Regra nova ou ajuste de regra: tabela antes/depois e revisão dos achados no `docs/fase-N.md`
  da fase atual.
- Entrada em **[Não lançado]** no [CHANGELOG.md](CHANGELOG.md).
- Mensagem de commit no imperativo, em português, dizendo o porquê (`Calibra SML011: catch que
  loga a exceção não perde a causa`).

## Release

Quem mantém o repositório:

1. Suba a versão no `Cargo.toml`, mova o **[Não lançado]** do CHANGELOG para a versão nova e
   atualize a linha do `gh release download` no README.
2. `cargo install --path . --locked`, commit e push.
3. `git tag vX.Y.Z && git push origin vX.Y.Z`: o workflow `release` publica os binários de 4
   alvos com `.sha256`.
