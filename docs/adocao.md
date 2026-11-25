# Adotar a guarda num projeto do time

Roteiro para colocar a `guarda` num repositório Java/Kotlin existente **sem travar o time**:
o legado é congelado no baseline e só o código novo precisa passar.

## 1. Instalar

O repositório da guarda é privado, então o download passa pelo `gh` autenticado com acesso a
ele:

```bash
# Windows
gh release download v0.5.0 -R Vitor716/guarda -p "*x86_64-pc-windows-msvc.zip"
# Linux
gh release download v0.5.0 -R Vitor716/guarda -p "*x86_64-unknown-linux-gnu.tar.gz*"
sha256sum -c guarda-v0.5.0-x86_64-unknown-linux-gnu.tar.gz.sha256
```

Descompacte e ponha o `guarda` no `PATH` (os hooks, o MCP e o pre-commit chamam pelo nome).
Com Rust instalado, também dá para compilar: `cargo install --git https://github.com/Vitor716/guarda --tag v0.5.0 --locked`.

## 2. Configurar o projeto (uma pessoa, uma vez)

Numa branch:

```bash
guarda init                 # detecta a arquitetura, escreve guarda.yml, schema e baseline
guarda contexto             # confira: camadas, pacote base e limites que ela entendeu
guarda baseline status      # quanto legado ficou congelado, por regra
```

Revise o `guarda.yml` antes de commitar:

- **`arquitetura.preset` e `base`**: se a detecção ficou abaixo de 60%, a `ARQ001` fica
  desligada até você declarar as camadas.
- **`perfil`**: `minimo` (só as regras que já são erro; bom para começar), `recomendado`
  (padrão) ou `rigoroso` (todas na severidade alvo).
- **`cobertura` e `duplicacao`**: só entram se você declarar as seções. Para cobertura,
  `codigo-novo.referencia` é a branch de comparação (normalmente `origin/main`).
- **Padrão da empresa**: com `estende: ../padrao-time.yml`, vários repositórios partem do
  mesmo arquivo, e cada um sobrescreve só o que precisa.

Commite `guarda.yml`, `.guarda/baseline.json` e `.guarda/guarda.schema.json`. O
`.guarda/cache/` já vem com o próprio `.gitignore`.

## 3. Ligar no fluxo de cada pessoa

```bash
guarda integrar git          # pre-commit: barra commit com violação nova (pessoa ou IA)
guarda integrar claude       # ou codex, antigravity, agents: hooks, MCP e instruções do agente
```

O pre-commit roda `guarda check --alterados` (só o que mudou, em menos de 1 s). No dia a dia:

```bash
guarda check --alterados     # antes de abrir o PR
guarda explicar CPX001       # por que a regra existe, exemplo errado e certo
```

## 4. Ligar no build e no CI

O CI é a garantia de verdade: o hook local pode ser pulado, o CI não.

**Gradle / Maven** (roda junto com `check` / `verify`):

```bash
guarda integrar gradle       # cria gradle/guarda.gradle.kts e a linha apply no build da raiz
guarda integrar maven        # imprime o trecho do exec-maven-plugin para colar no pom.xml
```

**GitHub Actions** (crie o segredo `GUARDA_TOKEN`: token *fine-grained* com
*Contents: read* no repositório `Vitor716/guarda`):

```yaml
name: guarda
on: pull_request

jobs:
  guarda:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
        with:
          fetch-depth: 0          # código novo e cobertura comparam com a branch de referência
      - name: Instalar guarda
        env:
          GH_TOKEN: ${{ secrets.GUARDA_TOKEN }}
        run: |
          gh release download v0.5.0 -R Vitor716/guarda -p "*x86_64-unknown-linux-gnu.tar.gz*"
          sha256sum -c guarda-*.sha256
          mkdir -p "$HOME/.local/bin"
          tar -xzf guarda-*.tar.gz --strip-components=1 -C "$HOME/.local/bin"
          echo "$HOME/.local/bin" >> "$GITHUB_PATH"
      # Com cobertura no guarda.yml, gere o XML antes (./gradlew test jacocoTestReport ou mvn verify).
      - run: guarda check --formato compacto
```

Para ver os achados como anotações no PR (*code scanning*), acrescente
`permissions: security-events: write` ao job e os passos abaixo. Em repositório privado, isso
exige o GitHub Code Security (Advanced Security) na organização.

```yaml
      - run: guarda check --formato sarif > guarda.sarif || true
      - uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: guarda.sarif
          category: guarda
```

Outro CI (GitLab, Jenkins, Azure): os mesmos dois passos, baixar o binário e rodar
`guarda check`. Exit `0` passa, `1` é violação nova e `2` é erro de configuração.

## 5. Regras do jogo para o time

- **O baseline é dívida assumida, não lixeira.** Crescer o baseline é decisão humana e fica
  registrada: `guarda baseline aceitar CODIGO arquivo --linha N --motivo "..."`. A IA não
  consegue rodar esse comando (o hook `proteger` barra).
- **Mudança no `guarda.yml` ou no baseline** aparece como `GRD001` no `check --alterados`
  (hook do agente e pre-commit). No PR, revise esses arquivos como se fossem código.
- **Supressão pontual exige motivo**: `// guarda:ignorar KT001 motivo: ...`. Sem motivo vira
  `SUP001`.
- **Corrigiu dívida?** `guarda check --atualizar-baseline` tira do baseline o que já foi
  corrigido e aperta as catracas (complexidade, tamanho), para o código não voltar a piorar.

## 6. Atualizar a guarda

Leia o [CHANGELOG](../CHANGELOG.md). Regra nova entra como **aviso** e não reprova o gate
padrão (`gate.falhar-em: erro`). Se a versão nova trouxer regras que você quer congelar no
legado, sem mexer nas que já estão ativas:

```bash
guarda baseline gerar --regras SEC001 SPR006      # só essas regras; preserva os motivos
```

Achou um falso positivo? Abra uma issue pelo modelo *Falso positivo* com o trecho mínimo, e
enquanto isso use `guarda:ignorar` com o motivo.
