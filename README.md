# guarda

[![ci](https://github.com/Vitor716/guarda/actions/workflows/ci.yml/badge.svg)](https://github.com/Vitor716/guarda/actions/workflows/ci.yml)
[![licença: MIT](https://img.shields.io/badge/licen%C3%A7a-MIT-blue.svg)](LICENSE)
![versão](https://img.shields.io/badge/vers%C3%A3o-0.5.0-informational.svg)

Linter determinístico para **Kotlin e Java**, escrito em Rust. Valida arquitetura,
complexidade, segurança, Spring, boas práticas, testes, duplicação e cobertura do código novo,
e responde numa saída feita para ser lida por pessoas e por agentes de IA com o mínimo de tokens.

**O problema:** a IA trata skill e guia de estilo como sugestão e foge do padrão; o Sonar só
reprova depois do push. A `guarda` é o portão objetivo que o código precisa passar **antes** do
commit: mesmo código + mesmo `guarda.yml` + mesma versão = mesmo resultado.

- **94 regras** com origem rastreável (Sonar, PMD, detekt, Find Security Bugs, ArchUnit),
  calibradas em 41 projetos reais com 0 falso positivo conhecido: [catálogo](docs/regras.md).
- **Legado congelado**: o baseline guarda a dívida existente; só violação nova reprova. Regras
  de limite usam catraca (o valor pode cair, nunca subir).
- **Rápida e offline**: binário nativo, paralelo e com cache; `check --alterados` roda em
  menos de 1 s. Nenhum código sai da máquina.
- **Arquitetura detectada**: presets `hexagonal`, `clean`, `layered`, `mvvm` e `modular`, no
  estilo ArchUnit, sem precisar escrever teste.
- **Para qualquer agente**: hooks para Claude Code, Codex e Antigravity, `AGENTS.md`, servidor
  MCP e pre-commit do git; Gradle, Maven e SARIF para o CI.

## Sumário

- [Instalação](#instalação)
- [Início rápido](#início-rápido)
- [Comandos](#comandos)
- [Agentes de IA](#agentes-de-ia)
- [Regras](#regras)
- [Configuração (`guarda.yml`)](#configuração-guardayml)
- [Como funciona](#como-funciona)
- [Documentação](#documentação)
- [Contribuindo](#contribuindo)
- [Licença](#licença)

## Instalação

Binários prontos para Windows, Linux e macOS (Intel e Apple Silicon), com `.sha256`, em cada
[release](https://github.com/Vitor716/guarda/releases). O repositório é privado, então o
download passa pelo `gh` autenticado:

```bash
gh release download v0.5.0 -R Vitor716/guarda -p "*x86_64-pc-windows-msvc.zip"
```

Descompacte e ponha o `guarda` no `PATH` (hooks, MCP e pre-commit chamam pelo nome). Ou
compile da fonte (Rust estável; no Windows, Visual Studio Build Tools com C++):

```bash
cargo install --path . --locked
```

## Início rápido

Na raiz de um projeto Java/Kotlin:

```bash
guarda init --ia git     # detecta a arquitetura, escreve guarda.yml, congela o legado e liga o pre-commit
guarda check             # verifica o projeto (exit 1 se houver violação nova)
guarda explicar CPX001   # por que a regra existe, com exemplo errado e certo
```

Saída compacta (o tempo é a única parte não determinística):

```text
src/main/kotlin/com/empresa/icms/IcmsService.kt
 42 CPX001 complexidade 22>15 | extraia trechos em funções menores e reduza o aninhamento
 88 KT001 uso de !! | troque por ?:, ?.let ou requireNotNull com mensagem
src/main/kotlin/com/empresa/icms/domain/Nota.kt
  3 ARQ001 dominio importa adaptadores-saida (HttpClient) | dependa de uma interface (porta) em dominio e implemente em adaptadores-saida
✗ 3 erros em 2 arquivos · baseline 412 · 0,18s
```

Tudo certo é uma linha só: `✓ guarda 0 erros · baseline 412 · 0,04s`.

Para levar a um projeto do time (perfil, padrão da empresa, CI, política de baseline), siga
[docs/adocao.md](docs/adocao.md).

## Comandos

```bash
guarda init [raiz]             # detecta a arquitetura, escreve guarda.yml, schema e baseline
guarda init --ia claude --ia git   # ... e integra com agentes (repita --ia)
guarda check [raiz]            # verifica e aplica o gate (exit 1 se houver violação nova)
guarda check --arquivos A B    # só esses arquivos (arquitetura vale pelo índice em cache)
guarda check --alterados       # só o que mudou contra HEAD, incluindo arquivos novos
guarda check --cobertura       # roda os testes antes, para a cobertura estar em dia
guarda check --formato json    # um objeto JSON por linha (também: sarif, texto, compacto)
guarda check --atualizar-baseline  # tira o que foi corrigido (e arquivos removidos); aperta catracas
guarda baseline status         # saúde do baseline: entradas por regra, motivos, corrigidas e catracas
guarda baseline gerar          # regrava o baseline preservando motivos (ou `--regras SEC001 SPR006`)
guarda baseline aceitar CPX001 src/.../Icms.kt --motivo "..."  # congela só isso (aceita --linha e --simbolo)
guarda explicar [CODIGO]       # por quê, errado e certo de uma regra (sem código: a lista)
guarda contexto                # as regras do projeto em ~12 linhas, para o prompt da IA
guarda integrar <alvo>         # claude | codex | antigravity | agents | git | gradle | maven
guarda mcp                     # servidor MCP por stdio: verificar, explicar, arquitetura
guarda schema                  # JSON Schema do guarda.yml
guarda medir [raiz]            # parse, tempo, violações, cobertura e código novo
```

## Agentes de IA

Fluxo que todo agente segue (manual completo em [docs/guia-ia.md](docs/guia-ia.md)):

1. **Antes de criar classes**: `guarda contexto` (ou MCP `arquitetura`) mostra as camadas, as
   pastas reais e os limites do projeto.
2. **Durante a edição**: `guarda check --arquivos <arquivo>` (ou MCP `verificar`); vale também
   para `application*.yml|properties` e `.env`.
3. **Dúvida sobre uma regra**: `guarda explicar <CODIGO>` (ou MCP `explicar`).
4. **Antes de encerrar**: `guarda check --alterados` até sair `✓ guarda 0 erros`.
5. **Decisão humana**: a IA não edita `guarda.yml`, `.guarda/`, hooks nem o plugin de
   cobertura, e não roda `guarda baseline gerar|aceitar` nem `git commit --no-verify`.

A verificação é uma CLI comum; o que muda por agente é só como ela chega até ele.

| Agente | `guarda integrar ...` | O que escreve |
| --- | --- | --- |
| Claude Code | `claude` | `CLAUDE.md`, hooks em `.claude/settings.json`, MCP em `.mcp.json` |
| Codex (ChatGPT) | `codex` | `AGENTS.md`, hooks em `.codex/hooks.json` (revise em `/hooks`); MCP: `[mcp_servers.guarda] command = "guarda" args = ["mcp"]` no `~/.codex/config.toml` |
| Antigravity (Gemini) | `antigravity` | `AGENTS.md`, `.agents/hooks.json`, `.agents/mcp_config.json` |
| Cursor, Copilot, outros | `agents` | `AGENTS.md` (e o MCP, se o agente aceitar: `guarda mcp`) |
| Qualquer um, inclusive sem hook | `git` | pre-commit que roda `guarda check --alterados` |
| Build (CI e máquina de qualquer um) | `gradle` | `gradle/guarda.gradle.kts` (ou `.gradle`) e a linha `apply` no build da raiz: `./gradlew check` roda a guarda |
| Build Maven | `maven` | nada: imprime o trecho do exec-maven-plugin para colar no pom; `mvn verify` roda a guarda |

Os hooks leem o evento de cada agente, decidem do mesmo jeito e respondem no protocolo de
cada um (`guarda hook <fase> --agente claude|codex|antigravity`); a tabela dos protocolos
está em [docs/fase-2.md](docs/fase-2.md).

### Hooks do Claude Code

`guarda integrar claude` (ou `init --ia claude`) acrescenta ao `.claude/settings.json`, sem
mexer no resto:

```json
{
  "hooks": {
    "PreToolUse": [{ "matcher": "Edit|Write|MultiEdit|NotebookEdit|Bash|PowerShell",
      "hooks": [{ "type": "command", "command": "guarda hook proteger", "timeout": 10 }] }],
    "PostToolUse": [{ "matcher": "Edit|Write|MultiEdit",
      "hooks": [{ "type": "command", "command": "guarda hook pos-edicao", "timeout": 30 }] }],
    "Stop": [{ "hooks": [{ "type": "command", "command": "guarda hook final", "timeout": 120 }] }]
  }
}
```

- **proteger**: a IA não edita `guarda.yml`, `.guarda/`, o plugin de cobertura do build nem
  a integração (hooks, MCP, pre-commit), e não roda `guarda baseline`, `guarda init` nem
  `git commit --no-verify`.
- **pos-edicao**: verifica o arquivo recém-editado; violação nova volta para a IA na hora.
- **final**: antes de encerrar, verifica o que mudou; segura a IA até 3 vezes
  (`gate.tentativas-final`) e depois libera, deixando o relatório em `.guarda/cache/final.txt`.

## Regras

| Família | Regras | O que cobre |
| --- | --- | --- |
| Arquitetura e negócio | `ARQ001`–`ARQ004`, `NEG001`–`NEG002` | camadas pelos imports, ciclos, framework no domínio, exceção HTTP ou genérica na regra de negócio |
| Complexidade e fluxo | `CPX001`–`CPX009`, `CND001`–`CND004`, `POL001`–`POL002` | complexidade cognitiva e ciclomática, tamanho, aninhamento, guard clause, condição extraída, polimorfismo |
| Code smells e correção | `SML001`–`SML011`, `BUG001`–`BUG009` | God Class, duplicação, exceções, código morto, bloco vazio, `finally` que engole, `String ==`, `BigDecimal(double)` |
| Segurança | `SEC001`–`SEC020` | credencial fixa (código e configuração), SQL/comando montado, cripto fraca, CORS, XXE, CSRF, JWT, config insegura, fluxo do request até o sink |
| Spring | `SPR001`–`SPR006` | injeção por campo, proxy em `private` e auto-chamada, `@Entity` no `@RequestBody`, `@Transactional` no controller |
| Kotlin e Java | `KT001`–`KT008`, `JV001`–`JV006` | `!!`, `GlobalScope`, estado mutável exposto, `System.out`, recurso sem try-with-resources, `Date` |
| Testes e cobertura | `TST001`–`TST004`, `COV000`–`COV003` | teste sem asserção ou tautológico, `sleep`, cobertura JaCoCo/Kover do código novo, por pacote e geral |
| Política | `ERR000`, `SUP001`, `GRD001`, `PRB001`–`PRB002` | arquivo não lido, supressão sem motivo, configuração alterada, import ou tipo proibido pelo time |

O [catálogo completo](docs/regras.md) traz a origem e a severidade de cada uma (gerado do
código). Regra nova entra como **aviso** e só vira erro depois de calibrada em projetos reais.

Supressão pontual, sempre com motivo (vale para a linha do comentário e a seguinte):

```kotlin
// guarda:ignorar KT001 motivo: contrato do framework garante não nulo
val x = y!!
```

## Configuração (`guarda.yml`)

Opcional: sem ele a guarda usa o perfil recomendado e detecta a arquitetura. Campo
desconhecido é erro com linha (`guarda.yml:3 limites: unknown field ...`).

```yaml
versao: 1
arquitetura:
  preset: hexagonal          # auto | hexagonal | clean | layered | mvvm | modular | nenhum
  base: com.empresa.app      # pacote base; padrões ".." casam abaixo dele
  camadas:                   # opcional: substitui as camadas do preset
    dominio: ["..domain.."]
  excecoes:
    - adaptadores-saida -> adaptadores-entrada
  negocio: [dominio, aplicacao, casos-de-uso, servico, modelo]  # onde valem NEG001/NEG002
limites:
  linhas-funcao: 60
  complexidade-cognitiva: 15
regras:
  desligar: [SUP001]
  severidade: { CPX005: aviso }
gate:
  falhar-em: erro            # erro | aviso
  baseline: .guarda/baseline.json
  tentativas-final: 3        # quantas vezes o hook final segura a IA
cobertura:                   # sem esta seção, cobertura não é verificada
  relatorio: auto            # acha o XML do JaCoCo/Kover em cada módulo
  codigo-novo: { referencia: origin/main, minimo: 80 }
  geral-minimo: 60
  por-pacote: { "..calculo..": 95 }
  ignorar: ["**/*Config.java", "**/dto/**"]
  rodar-no-final: false      # o hook final roda os testes antes (lento)
duplicacao:                  # sem esta seção, duplicação não é verificada
  tokens-minimos: 100
  codigo-novo-maximo: 3      # %
```

Padrão do time, perfil e regras do projeto:

```yaml
estende: ../padrao-time.yml    # base; este arquivo sobrescreve chave a chave
perfil: recomendado            # minimo | recomendado | rigoroso
regras:
  proibir:
    imports: [java.util.Date, "org.joda.**"]
    tipos-em-pacote:
      - tipo: Double
        pacote: "..calculo.."
        dica: use BigDecimal em cálculo monetário
  personalizadas: .guarda/regras  # um YAML por regra, com consulta do tree-sitter
```

Uma regra personalizada (`.guarda/regras/sem-stacktrace.yml`):

```yaml
codigo: USR001
linguagem: java
mensagem: chamada a printStackTrace
dica: use o logger
severidade: erro
consulta: |
  (method_invocation name: (identifier) @n (#eq? @n "printStackTrace")) @alvo
```

## Como funciona

- **Parser**: tree-sitter (`tree-sitter-java` e `tree-sitter-kotlin-ng`), um parser por
  thread, arquivos em paralelo com `rayon`, saída ordenada por arquivo, severidade e linha.
- **Arquitetura**: cada preset vira camadas (padrões de pacote no estilo ArchUnit, `..domain..`)
  e arestas permitidas. A camada de um pacote é a do padrão que casa mais perto da raiz. Só
  imports de pacotes do próprio projeto contam (biblioteca nunca gera ARQ001).
- **Detecção** (`auto`/`init`): cada preset ganha nota pela fração de pacotes distintos (em
  qualquer nível) e de arquivos que ele classifica, descontada metade da fração de arquivos
  com import proibido; nomes típicos do estilo (`port`, `adapter`, `usecase`, `viewmodel`)
  desempatam. Abaixo de 60% a ARQ001 fica desligada até as camadas serem declaradas.
- **Baseline**: impressão digital = código + arquivo + símbolo dono + hash do trecho
  normalizado (sem número de linha). Regras de limite (`CPX001–003`, `CPX005–009`, `SML001`,
  `COV002–003`) usam catraca determinística (inclusive em sobrecargas): o valor pode cair,
  nunca subir. `check --atualizar-baseline` remove o que foi corrigido (inclusive em recursos e
  arquivos deletados) e aperta catracas; `baseline status` audita a saúde do baseline; crescer
  exige `baseline gerar` (preserva `motivo`s e aceita `--regras`) ou `baseline aceitar --motivo`
  (aceita `--linha` e `--simbolo` e atualiza catracas). A IA não roda nenhum dos dois: o hook
  proteger barra.
- **Cache**: `.guarda/cache/` guarda o resultado das regras por arquivo, indexado por
  tamanho, data e hash do conteúdo; a chave inclui a versão e os limites, então o cache
  nunca muda o resultado. Também serve de índice do projeto no modo `--arquivos`.

## Documentação

| Documento | Para quem |
| --- | --- |
| [docs/adocao.md](docs/adocao.md) | quem vai adotar a guarda num projeto do time e no CI |
| [docs/regras.md](docs/regras.md) | catálogo das regras, com origem e severidade |
| [docs/guia-ia.md](docs/guia-ia.md) | agentes de IA usando a guarda num projeto |
| [docs/novas-regras.md](docs/novas-regras.md) | quem vai criar ou calibrar regras (pessoa ou IA) |
| [CHANGELOG.md](CHANGELOG.md) | o que mudou em cada versão |
| [docs/fase-0.md](docs/fase-0.md) … [docs/fase-4.md](docs/fase-4.md) | medições e decisões de cada fase |

## Contribuindo

Falso positivo, regra nova ou bug: abra uma issue pelos modelos. Para código, veja o
[CONTRIBUTING.md](CONTRIBUTING.md); agentes de IA começam pelo [AGENTS.md](AGENTS.md).
Vulnerabilidade: [SECURITY.md](SECURITY.md).

```bash
cargo test                                  # unitários + ponta a ponta (tests/*.rs)
cargo run -- arvore caminho/Arquivo.kt      # árvore sintática (para escrever regras)
```

### Roadmap

| Fase | Entrega | Situação |
| --- | --- | --- |
| 0 · Prova de conceito | parser Kotlin/Java, regras iniciais, saída compacta, baseline | entregue (v0.1.0) |
| 1 · MVP para a IA | JSON Schema, hooks do Claude Code, cache incremental | entregue (v0.1.0) |
| 2 · Sonar local | cobertura JaCoCo/Kover de código novo, duplicação | entregue (v0.2.0); falta comparar os números com o Sonar num PR real |
| 3 · Catálogo completo | regras de complexidade e smells, 5 presets, regras YAML, `explicar`, SARIF | entregue (v0.3.0) |
| 4 · Ecossistema | MCP, releases, Gradle/Maven, regras de fluxo, negócio, segurança e Spring | v0.4.0 e v0.5.0; contínuo |

## Licença

[MIT](LICENSE) © 2026 Vitor Vieira de Oliveira.
