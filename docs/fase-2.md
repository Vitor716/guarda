# Fase 2 — Sonar local e qualquer agente

A Fase 2 começou em 2026-11-24, antes do fim do portão da Fase 1 (a semana de uso real
com os hooks no srv-usuario-api, que vai até ~2026-12-01). Decisão do dono do projeto.

Portão da fase, pela especificação: **números iguais aos do Sonar da empresa em um PR real
(±1 ponto)**. Ainda não medido: precisa de um PR que também passe pelo Sonar.

## Entregue

### Sonar local

| Item | Como funciona |
| --- | --- |
| Código novo | Linhas acrescentadas ou alteradas desde o ancestral comum com `cobertura.codigo-novo.referencia` (padrão `origin/main`; cai para `main`, `master` ou `HEAD` se não existir), incluindo o que não foi commitado e arquivos novos não rastreados. No `--arquivos`/`--alterados`, o diff é só desses arquivos. |
| Leitura do JaCoCo/Kover | XML em streaming (`quick-xml`), procurado em cada módulo (`target/site/jacoco/jacoco.xml`, `build/reports/kover/report.xml`...) ou listado em `cobertura.relatorio`. O `.exec` não é lido (ver a especificação). |
| Conta | Como o Sonar: (linhas cobertas + desvios cobertos) / (linhas executáveis + desvios). |
| COV001 | Código novo abaixo de `minimo` (Sonar way: 80%): uma linha-resumo em `[cobertura]` e, por arquivo, as faixas sem teste e os desvios com um caminho sem teste (até 6 faixas, depois `+N`). Nunca entra no baseline: é sobre a mudança atual. |
| COV002 / COV003 | Pacote (`por-pacote`, padrões ArchUnit) e projeto (`geral-minimo`). Catraca no baseline sobre a parte descoberta: a cobertura pode subir, nunca cair. |
| Relatório velho | Arquivo alterado depois do XML sai da conta e vira nota no rodapé: `cobertura desatualizada em N arquivo(s): rode guarda check --cobertura`. Nunca reprova por relatório velho. |
| `check --cobertura` | Roda os testes antes (`mvn -q test jacoco:report`, `gradlew test jacocoTestReport`/`koverXmlReport`, ou `cobertura.comando`). Teste falhando vira COV000. No hook final só com `cobertura.rodar-no-final: true` (lento, desligado por padrão). |
| SML002 | Blocos de `tokens-minimos` (padrão 100) tokens iguais, por hash rolante Karp-Rabin confirmado token a token, entre arquivos e dentro do mesmo arquivo. Por bloco: aviso, com a outra cópia (`também em Total.java:3`). No código novo, acima de `codigo-novo-maximo` (Sonar way: 3%) de linhas duplicadas: erro em `[duplicação]`. Tokens no cache. |
| `init` | Escreve `duplicacao` sempre e `cobertura` quando acha JaCoCo/Kover no build, com a referência detectada. |

### Qualquer agente

O núcleo (CLI, saída compacta, baseline) já não dependia de agente. A Fase 2 tira o Claude
Code do centro da integração:

| Canal | Serve para | Comando |
| --- | --- | --- |
| `AGENTS.md` | Codex, Antigravity, Cursor, Copilot e o que mais ler o padrão | `guarda integrar agents` |
| MCP (`guarda mcp`) | Qualquer agente com MCP: ferramentas `verificar`, `explicar`, `arquitetura` | escrito por `integrar claude` (`.mcp.json`) e `integrar antigravity` (`.agents/mcp_config.json`); no Codex, uma linha no `~/.codex/config.toml` |
| Hooks | Bloquear antes, corrigir depois, não encerrar com erro | `integrar claude`, `integrar codex`, `integrar antigravity` |
| Pre-commit do git | Qualquer IA, inclusive sem hook (ChatGPT no navegador, copiar e colar), e pessoas | `guarda integrar git` |
| `guarda explicar` | A explicação longa de cada regra, sob demanda | CLI e ferramenta MCP |

Os hooks traduzem o evento de cada agente para um formato único e respondem no protocolo
de cada um (conferido na documentação oficial em 2026-11-24):

| | Claude Code | Codex (ChatGPT) | Antigravity (Gemini) |
| --- | --- | --- | --- |
| Arquivo | `.claude/settings.json` | `.codex/hooks.json` | `.agents/hooks.json` |
| Edição chega como | `Edit`/`Write` (`file_path`) | `apply_patch` (patch em `command`) | `replace_file_content`/`write_to_file` (`TargetFile`) |
| Recusar ferramenta | exit 2 + stderr | exit 2 + stderr | `{"decision":"deny"}` |
| Corrigir após edição | exit 2 + stderr | exit 2 + stderr | sem canal: não registrado |
| Não encerrar | exit 2 + stderr | `{"decision":"block"}` | `{"decision":"continue"}` |

O Gemini CLI foi substituído pelo Antigravity CLI em junho de 2026; por isso o alvo é o
Antigravity. O `proteger` também protege os arquivos de integração dos três agentes, o
`.mcp.json` e o pre-commit, e recusa `git commit --no-verify`.

## Medições — srv-usuario-api, 2026-11-24

Numa cópia do projeto, com `cobertura` e `duplicacao` ligadas (o projeto real segue com a
configuração da Fase 1 até o fim da semana de teste).

| Medida | Resultado | Meta |
| --- | --- | --- |
| Relatório JaCoCo | `target/site/jacoco/jacoco.xml`, de 23/04/2026 | — |
| Cobertura geral (relatório) | 2,1% | — |
| Código novo contra origin/main | ~14 mil linhas em 340 arquivos (o branch atual está bem à frente da main) | — |
| Cobertura do código novo | 2,4% (199 arquivos com linha sem teste) | Sonar way: 80% |
| Blocos duplicados | 9 (ex.: `MasterService` l.138 × l.171, DTOs de síndico) | — |
| `check --alterados` | ~220 ms (era ~370 ms antes de paralelizar git e XML) | < 300 ms |
| `check --arquivos` (1 arquivo) | ~150 ms | — |
| Projeto todo | 0,26 s | < 2 s |

O resultado de cobertura é o esperado para esse branch: quase nada do código novo tem teste,
e o Sonar reprovaria igual. Para uso diário, a referência de código novo provavelmente deve
ser o branch de onde a feature saiu, não `origin/main`; é configuração do projeto.

## Limites conhecidos

- **Paridade com o Sonar não medida** (portão da fase). Diferenças possíveis: o Sonar só
  conta linhas de código novo em arquivos que ele analisa, e o período de código novo pode
  ser outro (versão anterior, data).
- `check --cobertura` roda os testes do projeto todo, não só dos módulos afetados.
- No Antigravity, o PostToolUse não devolve nada à IA: a verificação do arquivo editado fica
  para o Stop.
- No Codex, hooks de projeto só rodam em projeto confiável e depois de revisados em `/hooks`;
  o MCP do Codex é configurado à mão no `~/.codex/config.toml`.
- A densidade de duplicação e a cobertura de código novo só valem no projeto todo e no
  `--alterados`; no `--arquivos` (e no hook pós-edição) só aparecem os blocos e as faixas.
