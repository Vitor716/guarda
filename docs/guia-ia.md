# Guia para Agentes de IA (`guarda`)

Este guia é o manual operacional para qualquer agente de IA (Claude Code, Codex, Antigravity,
Cursor, Copilot ou outro) utilizar a ferramenta `guarda` de forma autônoma, correta e com o
mínimo de tokens em projetos **Java e Kotlin**. Para criar regras novas ou calibrar as
existentes, o manual é o [novas-regras.md](novas-regras.md).

---

## 1. O que é e qual é o contrato

- A `guarda` é um **linter determinístico local** (binário nativo em Rust, offline): mesmo
  código + mesmo `guarda.yml` + mesma versão = exatamente o mesmo resultado.
- Ela valida arquitetura, complexidade, segurança, Spring, correção (bugs), boas práticas,
  testes, duplicação e cobertura de código novo.
- **Saída enxuta para IA**: fora do terminal (ou com `--formato compacto`), cada violação
  ocupa uma linha com a correção depois do `|`. Quando não há erros novos, a resposta é uma
  única linha (`✓ guarda 0 erros ...`).

### Códigos de saída (exit codes)

| Exit code | Significado | Ação esperada da IA |
| :---: | --- | --- |
| `0` | Passou no gate (0 violações na severidade de `gate.falhar-em`) | Pode concluir a tarefa |
| `1` | Há violação nova que reprova o gate | **Corrija as violações apontadas** e rode novamente |
| `2` | Erro de uso, `guarda.yml` inválido ou erro interno | Leia o `stderr` (aponta arquivo e linha do YAML) e informe o humano se for arquivo protegido |

---

## 2. Fluxo padrão ao trabalhar num projeto com `guarda`

Siga sempre esta sequência de 4 passos:

1. **Antes de criar classes ou pacotes novos**: descubra as camadas reais e os limites do projeto.
   - Via MCP: chame a ferramenta `arquitetura`.
   - Via CLI: rode `guarda contexto`.
2. **Durante as edições** (loop rápido de ~50–150ms): verifique só o arquivo que acabou de editar.
   - Via MCP: `verificar` com `arquivos: ["src/main/.../Arquivo.java"]`.
   - Via CLI: `guarda check --arquivos src/main/.../Arquivo.java --formato compacto`.
   - Nota: a arquitetura e a detecção de entidades JPA continuam valendo no modo `--arquivos`
     graças ao índice em cache.
3. **Quando não entender uma regra**: consulte a explicação sob demanda (por quê + exemplo errado e certo).
   - Via MCP: `explicar` com `codigo: "ARQ001"` (ou `CPX001`, `SEC015`, etc.).
   - Via CLI: `guarda explicar ARQ001` (sem código, lista todas as 94 regras).
4. **Antes de encerrar a tarefa**: valide tudo o que mudou contra `HEAD` (incluindo arquivos novos).
   - Via MCP: `verificar` (sem argumentos).
   - Via CLI: `guarda check --alterados --formato compacto`.
   - Se o `guarda.yml` tiver seção `cobertura:` e o rodapé avisar cobertura desatualizada,
     rode `guarda check --alterados --cobertura --formato compacto` para atualizar o XML do
     JaCoCo/Kover e checar `COV001`.

---

## 3. Como ler a saída compacta e JSON

### Saída compacta (`--formato compacto`, padrão quando stdout não é TTY)

```text
src/main/kotlin/com/app/domain/Pedido.kt
 42 CPX001 complexidade 18>15 (baseline 16) | extraia trechos em funções menores e reduza o aninhamento
 88 KT001 x3 l.88,92,105 uso de !! | troque por ?:, ?.let ou requireNotNull com mensagem
✗ 4 erros em 1 arquivo · 1 aviso · baseline 210 · 0,14s
```

- **Cabeçalho**: caminho relativo do arquivo (aparece uma vez por arquivo).
- **Linha de violação**: `<linha> <CODIGO> <mensagem> | <dica de correção>`.
- **Colapso (`x3 l.88,92,105`)**: 3 ou mais repetições idênticas no mesmo arquivo são agrupadas
  em uma única linha.
- **`(baseline 16)`**: regra de **catraca** (`CPX001–003`, `CPX005–009`, `SML001`, `COV002–003`).
  A função já era legada com valor 16 e cresceu para 18; volte para ≤ 16 (ou ≤ limite).
- **`+N ocultas (use --max)`**: teto padrão de 30 linhas; passe `--max 100` se precisar ver o resto.
- **`baseline 210`**: 210 violações legadas foram ignoradas automaticamente — não mexa nelas a
  menos que faça parte da tarefa.

### Saída JSON (`--formato json`)

Um objeto JSON por linha (NDJSON), útil para parsers ou scripts:
```json
{"arquivo":"src/main/java/A.java","linha":12,"codigo":"SPR006","severidade":"aviso","mensagem":"@Transactional em controlador HTTP","dica":"mova @Transactional para o serviço/caso de uso; transação na camada web segura conexão durante o HTTP","simbolo":"PedidoController.criar"}
```

---

## 4. O que a IA PODE e NÃO PODE fazer

### Proibido (bloqueado pelo hook `proteger` e sinalizado por `GRD001`)
- **Nunca edite** `guarda.yml`, `.guarda/**` (incluindo `.guarda/baseline.json`), hooks
  (`.claude/settings*.json`, `.codex/hooks.json`, `.agents/hooks.json`, `.git/hooks/pre-commit`),
  `.mcp.json` nem o plugin de cobertura (`jacoco`/`kover`) do `pom.xml`/`build.gradle(.kts)`.
- **Nunca execute** `guarda baseline gerar`, `guarda baseline aceitar`, `guarda init` (em
  projeto já configurado) ou `git commit --no-verify`.
- **Nunca burle regras de teste ou supressão**:
  - `@Suppress` / `@SuppressWarnings` sem comentário de motivo gera `SUP001` (erro).
  - `assertTrue(true)`, `assertFalse(false)`, `assertEquals(x, x)` ou `assertNotNull("...")`
    para driblar `TST001` geram `TST004`.

### Permitido
- Rodar `guarda check` (`--alterados`, `--arquivos`, `--cobertura`, `--sem-baseline`, `--formato`),
  `guarda contexto`, `guarda explicar`, `guarda baseline status` (somente leitura) e `guarda medir`.
- Usar supressão pontual **somente** quando houver justificativa técnica real e inevitável,
  sempre com motivo explícito:
  ```kotlin
  // guarda:ignorar KT001 motivo: contrato do framework garante valor não nulo aqui
  val x = y!!
  ```
  (vale na linha do comentário e na linha seguinte; para múltiplas regras: `KT001,CPX001 motivo: ...`).
- Quando uma violação for dívida legítima que precisa ser aceita no baseline ou exige mudar
  `guarda.yml`, **pare e peça ao humano** informando exatamente o comando sugerido (ex.:
  `guarda baseline aceitar CPX001 caminho/Arquivo.java --linha 42 --motivo "..."`).

---

## 5. Referência das ferramentas MCP (`guarda mcp`)

Se o agente estiver conectado ao servidor MCP da `guarda`, prefira as ferramentas MCP em vez de
abrir shell:

| Ferramenta | Parâmetros | Quando usar |
| --- | --- | --- |
| `arquitetura` | `{}` | Antes de criar classe/pacote: retorna camadas, pastas reais, arestas permitidas e limites |
| `verificar` | `arquivos?: string[]`, `projeto_todo?: boolean` | Sem argumentos verifica `--alterados`; com `arquivos` verifica só os editados |
| `explicar` | `codigo: string` | Retorna o porquê, exemplo errado e exemplo certo de qualquer regra |

---

## 6. Como instalar e configurar a `guarda` em um projeto (quando um humano pedir)

Se o humano pedir para adotar a `guarda` em um repositório novo ou legado:

1. **Verifique se o binário está no PATH**: `guarda --version`.
   - Se não estiver, compile da fonte (`cargo install --path <caminho-do-repo-guarda> --locked`)
     ou baixe da release (`gh release download <tag> -R Vitor716/guarda`).
2. **Inicialize na raiz do projeto** (congela o legado sem quebrar o time):
   ```bash
   guarda init
   ```
   Gera `guarda.yml`, `.guarda/baseline.json` e `.guarda/guarda.schema.json` (`.guarda/cache/`
   já cria seu próprio `.gitignore`).
3. **Integre aos alvos pedidos pelo humano**:
   ```bash
   guarda integrar git          # pre-commit (qualquer humano ou IA)
   guarda integrar claude       # CLAUDE.md + .claude/settings.json + .mcp.json
   guarda integrar codex        # AGENTS.md + .codex/hooks.json
   guarda integrar antigravity  # AGENTS.md + .agents/hooks.json + .agents/mcp_config.json
   guarda integrar agents       # apenas AGENTS.md (Cursor, Copilot, etc.)
   guarda integrar gradle       # tarefa guardaCheck ligada ao ./gradlew check
   guarda integrar maven        # imprime o trecho do exec-maven-plugin para o pom.xml
   ```
4. **Manutenção de baseline (somente quando um humano pedir explicitamente)**:
   - Auditar saúde do baseline: `guarda baseline status`
   - Limpar o que já foi corrigido e apertar catracas: `guarda check --atualizar-baseline`
   - Congelar somente regras novas após atualizar a `guarda` (sem tocar nas regras já ativas):
     `guarda baseline gerar --regras SEC001 SEC017 SPR006 BUG009`
   - Aceitar uma ocorrência específica com justificativa:
     `guarda baseline aceitar <CODIGO> <ARQUIVO> --linha <N> --motivo "<justificativa com 10+ chars>"`
