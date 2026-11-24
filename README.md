# guarda

Linter determinístico para **Kotlin e Java**, escrito em Rust, que valida arquitetura,
complexidade e boas práticas e responde numa saída feita para ser lida por uma IA com o
mínimo de tokens.

O problema que resolve: a IA trata skill e baseline como sugestão e foge do padrão; o Sonar
só reprova depois do push. A `guarda` é o portão objetivo que a IA precisa passar antes de
terminar uma tarefa: mesmo código + mesmo `guarda.yml` + mesma versão = mesmo resultado.

> Status: **Fase 1 (MVP para a IA)**, aguardando o portão de uma semana de uso real.
> Ver [docs/fase-0.md](docs/fase-0.md), [docs/fase-1.md](docs/fase-1.md) e o [roadmap](#roadmap).

## Instalação

```bash
cargo install --path .      # põe `guarda` no PATH (os hooks chamam pelo nome)
```

## Uso

```bash
guarda init [raiz]             # detecta a arquitetura, escreve guarda.yml, schema e baseline
guarda init --ia claude        # ... e integra com o Claude Code (CLAUDE.md + hooks)
guarda check [raiz]            # verifica e aplica o gate (exit 1 se houver violação nova)
guarda check --arquivos A B    # só esses arquivos (arquitetura vale pelo índice em cache)
guarda check --alterados       # só o que mudou contra HEAD, incluindo arquivos novos
guarda check --formato json    # um objeto JSON por linha
guarda check --atualizar-baseline  # tira o que foi corrigido; nunca acrescenta
guarda baseline gerar          # regrava o baseline (decisão humana: congela dívida nova)
guarda contexto                # as regras do projeto em ~12 linhas, para o prompt da IA
guarda integrar claude|agents  # contexto no CLAUDE.md/AGENTS.md (+ hooks, no Claude Code)
guarda schema                  # JSON Schema do guarda.yml
guarda medir [raiz]            # parse, tempo e violações por regra
```

## Integração com o Claude Code

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
  os hooks, e não roda `guarda baseline`/`guarda init`.
- **pos-edicao**: verifica o arquivo recém-editado; violação nova volta para a IA na hora.
- **final**: antes de encerrar, verifica o que mudou; segura a IA até 3 vezes
  (`gate.tentativas-final`) e depois libera, deixando o relatório em `.guarda/cache/final.txt`.

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

## Regras

| Código | Regra | Origem | Padrão |
| --- | --- | --- | --- |
| ERR000 | Arquivo que o parser não leu (nunca é pulado em silêncio) | política da guarda | erro |
| ARQ001 | Dependência proibida entre camadas, pelos imports | ArchUnit, Konsist | erro |
| CPX001 | Complexidade cognitiva | Sonar S3776 | > 15 |
| CPX005 | Função longa (linhas de código, sem brancos e comentários) | detekt LongMethod | > 60 |
| KT001 | Uso de `!!` | detekt UnsafeCallOnNullableType | erro |
| SUP001 | `@Suppress`/`@SuppressWarnings` ou `guarda:ignorar` sem motivo | política da guarda | erro |
| GRD001 | `guarda.yml` ou baseline alterado no diff (só em `--alterados`) | política da guarda | aviso |

Supressão pontual, sempre com motivo (vale para a linha do comentário e a seguinte):

```kotlin
// guarda:ignorar KT001 motivo: contrato do framework garante não nulo
val x = y!!
```

## `guarda.yml`

Opcional: sem ele a guarda usa o perfil recomendado e detecta a arquitetura. Campo
desconhecido é erro com linha (`guarda.yml:3 limites: unknown field ...`).

```yaml
versao: 1
arquitetura:
  preset: hexagonal          # auto | hexagonal | clean | layered | nenhum
  base: com.empresa.app      # pacote base; padrões ".." casam abaixo dele
  camadas:                   # opcional: substitui as camadas do preset
    dominio: ["..domain.."]
  excecoes:
    - adaptadores-saida -> adaptadores-entrada
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
```

As seções `cobertura`, `duplicacao`, `estende`, `perfil`, `regras.proibir` e
`regras.personalizadas` já são aceitas (formato final), mas só têm efeito nas próximas fases.

## Como funciona

- **Parser**: tree-sitter (`tree-sitter-java` e `tree-sitter-kotlin-ng`), um parser por
  thread, arquivos em paralelo com `rayon`, saída ordenada por arquivo, severidade e linha.
- **Arquitetura**: cada preset vira camadas (padrões de pacote no estilo ArchUnit, `..domain..`)
  e arestas permitidas. A camada de um pacote é a do padrão que casa mais perto da raiz. Só
  imports de pacotes do próprio projeto contam (biblioteca nunca gera ARQ001).
- **Detecção** (`auto`/`init`): cada preset ganha nota pela fração de pacotes de primeiro nível
  e de arquivos que ele classifica, descontada metade da fração de arquivos com import
  proibido. Abaixo de 60% a ARQ001 fica desligada até as camadas serem declaradas.
- **Baseline**: impressão digital = código + arquivo + símbolo dono + hash do trecho
  normalizado (sem número de linha). Regras de limite (CPX001, CPX005) usam catraca: o valor
  pode cair, nunca subir. `check --atualizar-baseline` só remove; crescer exige
  `baseline gerar`.
- **Cache**: `.guarda/cache/` guarda o resultado das regras por arquivo, indexado por
  tamanho, data e hash do conteúdo; a chave inclui a versão e os limites, então o cache
  nunca muda o resultado. Também serve de índice do projeto no modo `--arquivos`.

## Desenvolvimento

```bash
cargo test                                  # unitários + ponta a ponta (tests/*.rs)
cargo run -- arvore caminho/Arquivo.kt      # imprime a árvore sintática (para escrever regras)
cargo build --release
```

No Windows o build precisa do Visual Studio Build Tools com o workload C++ (as gramáticas
do tree-sitter são C).

## Roadmap

| Fase | Entrega | Portão |
| --- | --- | --- |
| 0 · Prova de conceito | parser Kotlin/Java, 5 regras, saída compacta, baseline | < 1% de erro de parse e < 1 s no projeto todo |
| 1 · MVP para a IA | JSON Schema, hooks do Claude Code, cache incremental | uma semana com a IA sem falso positivo |
| 2 · Sonar local | cobertura JaCoCo/Kover de código novo, duplicação | números iguais aos do Sonar (±1 ponto) |
| 3 · Catálogo completo | ~40 regras, 5 presets, regras YAML, `explicar`, SARIF | init acerta o preset em 3 projetos |
| 4 · Ecossistema | servidor MCP, releases, wrapper Gradle/Maven | contínuo |
