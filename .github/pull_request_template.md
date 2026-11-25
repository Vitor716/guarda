## O que muda e por quê

<!-- Uma ou duas frases. Ligue a issue: Closes #N -->

## Checklist

- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` e `cargo test` passando
- [ ] Regra nova entra como aviso, com origem no título e explicação em `src/explicar.rs`
- [ ] Mudou o catálogo: `GUARDA_ATUALIZAR_DOCS=1 cargo test catalogo` (docs/regras.md)
- [ ] Regra nova ou ajuste: medição antes/depois e revisão dos achados no `docs/fase-N.md`
- [ ] Entrada em **[Não lançado]** no CHANGELOG.md
- [ ] Nenhum segredo real em teste, doc ou commit
