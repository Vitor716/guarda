# Changelog

Mudanças relevantes por versão. Formato baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/);
a versão segue [SemVer](https://semver.org/lang/pt-BR/). Enquanto a versão for `0.x`, uma
versão menor (`0.5` → `0.6`) pode trazer regra nova, que aparece como violação nova nos
projetos até o baseline ser atualizado (`guarda baseline gerar --regras ...`).

As medições em projetos reais de cada fase estão em [docs/](docs/).

## [Não lançado]

### Adicionado
- `docs/regras.md`: catálogo das 94 regras gerado do código (um teste garante que está em dia).
- `docs/adocao.md`: como adotar a guarda num projeto do time e no CI.
- Licença MIT, `CONTRIBUTING.md`, `SECURITY.md`, modelos de issue e de pull request e
  Dependabot para Cargo e GitHub Actions.

### Corrigido
- Descrição do pacote no `Cargo.toml` com acentuação quebrada.

## [0.5.0] - 2026-11-25

### Adicionado
- Lote 5: 41 regras tiradas das baselines de Sonar, PMD, detekt, Find Security Bugs e do
  security-audit-skill da Cloudflare: segurança (`SEC001`–`SEC020`, inclusive arquivos de
  configuração e de segredo), Spring (`SPR001`–`SPR006`), correção (`BUG001`–`BUG009`) e
  boas práticas (`SML010`, `SML011`, `CND004`, `KT008`, `JV006`, `TST004`). Todas entram como aviso.
- `SEC020`: fluxo dentro da função de entrada do request (`@RequestParam`, `getParameter`...)
  até arquivo, URL (SSRF), redirecionamento, comando, SQL, `Class.forName` ou SpEL.
- Baseline: `baseline status`, `baseline gerar --regras` (congela só regras novas, preserva
  motivos), `baseline aceitar --linha/--simbolo`, limpeza de arquivos removidos e de recursos.
- `docs/novas-regras.md` e `tools/` (`clonar.py`, `medir.py`, `ver_achados.py`): receita e
  scripts para criar e calibrar regras em projetos reais.
- `docs/guia-ia.md`: manual operacional para agentes de IA.

### Alterado
- Calibração em 41 projetos (29 repositórios do GitHub e 12 cópias locais): 0 falso positivo
  conhecido. `SEC001` ignora nome de chave (`senha_key`) e senha de H2 em memória; `SML010`
  ignora método de classe anônima; `SML011` só acusa quando a exceção some de vez.

## [0.4.0] - 2026-11-24

### Adicionado
- 7 regras de fluxo e de negócio (53 no total): `CND001`–`CND003` (guard clause, `else` depois de `return`,
  condição mista), `POL001`–`POL002` (polimorfismo), `NEG001`–`NEG002` (exceção de negócio).
- `CPX002` com limite 10.
- `integrar gradle` (tarefa `guardaCheck` ligada ao `check`) e `integrar maven`.
- `baseline aceitar --motivo`.
- CI (fmt, clippy e testes em Linux e Windows) e release por tag com binários de 4 alvos.

## [0.3.0] - 2026-11-24

### Adicionado
- Catálogo da Fase 3 (46 regras no total): complexidade, smells, Kotlin, Java e testes, 5 presets de
  arquitetura, `regras.proibir`, regras personalizadas por consulta do tree-sitter, `explicar`
  e saída SARIF.

## [0.2.0] - 2026-11-24

### Adicionado
- Sonar local: cobertura JaCoCo/Kover do código novo (`COV000`–`COV003`) e duplicação (`SML002`).
- Integração com Codex, Antigravity, `AGENTS.md`, pre-commit do git e servidor MCP.

## [0.1.0] - 2026-11-24

### Adicionado
- Parser Kotlin/Java com tree-sitter, primeiras regras, baseline com catraca e saída compacta.
- JSON Schema do `guarda.yml`, cache incremental, hooks do Claude Code e `guarda contexto`.

[Não lançado]: https://github.com/Vitor716/guarda/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/Vitor716/guarda/releases/tag/v0.5.0
[0.4.0]: https://github.com/Vitor716/guarda/commit/a2e0b1a
[0.3.0]: https://github.com/Vitor716/guarda/commit/82960d8
[0.2.0]: https://github.com/Vitor716/guarda/commit/9b13dab
[0.1.0]: https://github.com/Vitor716/guarda/commit/47af731
