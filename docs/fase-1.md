# Fase 1 — MVP para a IA

Portão da fase: **uma semana de uso real com a IA sem falso positivo nas regras ativas**.
Esse portão só se mede com os hooks ligados num projeto de verdade; o que está abaixo é o
que foi entregue e medido antes disso.

## Entregue

| Item | Como usar |
| --- | --- |
| JSON Schema do `guarda.yml` | `guarda schema`; o `init` grava `.guarda/guarda.schema.json` e põe `# yaml-language-server: $schema=...` na primeira linha do `guarda.yml` (autocomplete e validação no editor). Valores fechados (`preset`, `falhar-em`, `severidade`) viram enum, e um valor errado é recusado com linha. |
| Cache incremental | `.guarda/cache/arquivos.json`. Arquivo com mesmo tamanho e data nem é lido; data nova → compara hash; só conteúdo novo passa pelo parser. A chave inclui a versão e os limites. O próprio cache se põe fora do git (`.guarda/cache/.gitignore` com `*`). |
| Índice pelo cache | `--arquivos` e `--alterados` usam o cache do projeto inteiro como índice, no lugar da leitura por regex da Fase 0: a arquitetura vale mesmo verificando um arquivo só. |
| `guarda contexto` | As regras do projeto em ~12 linhas, incluindo as pastas reais de cada camada, geradas do mesmo `guarda.yml` que a verificação usa. |
| `guarda integrar claude\|agents` | Escreve o contexto entre marcadores no `CLAUDE.md`/`AGENTS.md` (troca no lugar, não duplica) e, no Claude Code, junta os hooks ao `.claude/settings.json` preservando o resto. `init --ia claude` faz o mesmo no fim do init. |
| `guarda hook proteger` (PreToolUse) | Recusa Edit/Write em `guarda.yml` e `.guarda/**`, edição que mexa nas linhas de JaCoCo/Kover do `pom.xml`/`build.gradle(.kts)` ou nos hooks da guarda em `.claude/settings*.json`, e comandos `guarda baseline`, `guarda init` ou de shell que alterem esses caminhos. |
| `guarda hook pos-edicao` (PostToolUse) | Verifica só o arquivo editado, com baseline; violação nova volta para a IA na hora. |
| `guarda hook final` (Stop) | Verifica `--alterados`; segura o encerramento até `gate.tentativas-final` (padrão 3) vezes, depois libera, grava `.guarda/cache/final.txt` e avisa você por `systemMessage`. A mesma violação liberada não prende a IA de novo na sessão. |
| GRD001 (aviso) | `guarda.yml` ou o baseline mudaram no diff: revisão humana no PR. Aparece no `check --alterados` e chega a você pelo hook final. |

Contrato dos hooks conferido na documentação do Claude Code em 2026-11-24: exit 2 bloqueia e
o stderr volta para a IA; no Stop, exit 2 impede o encerramento; exit 1 é erro que não
bloqueia (a guarda usa exit 1 para erro próprio, para nunca prender a IA por bug).

## Medições — srv-usuario-api (277 arquivos Java), 2026-11-24

| Medida | Resultado | Meta da especificação |
| --- | --- | --- |
| Projeto todo, sem cache | 0,05 s | < 2 s |
| Projeto todo, com cache | 0,03 s | — |
| `check --arquivos` (um arquivo), processo inteiro | ~90 ms | < 50 ms por arquivo (regras) |
| `check --alterados`, processo inteiro com git | ~215 ms | < 300 ms |
| `hook pos-edicao`, processo inteiro | ~75 ms (362 ms na primeira, a frio) | — |
| Erros de parse | 0 | < 1% |

## Limites conhecidos

- O `proteger` para shell é heurístico: recusa comando que cite `guarda.yml`, `.guarda` ou
  `.claude/settings` junto com um verbo que altera (rm, mv, sed, `>`, Set-Content,
  `git checkout`...). Um script que escreva nesses caminhos sem citá-los passa; o GRD001 no
  hook final é a segunda barreira.
- `MultiEdit` e `NotebookEdit` estão no matcher por segurança, mas não aparecem na
  documentação atual dos hooks.
- O hook `final` roda a cada encerramento da IA; violação nova do próprio humano no diff
  também segura a IA (até o limite de tentativas).
- Cobertura no hook `final` (`--cobertura`) fica para a Fase 2.
- O comando `guarda` precisa estar no PATH do Claude Code (`cargo install --path .`).
