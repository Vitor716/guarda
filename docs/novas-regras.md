# Guia para IA: novas regras e calibração

Manual para um agente de IA **aumentar a base de validação** da `guarda`: escolher uma regra
nova nas ferramentas de mercado, implementar, testar, medir em projetos reais, revisar os
falsos positivos e só então promover. Para **usar** a `guarda` num projeto, o manual é o
[guia-ia.md](guia-ia.md).

Duas coisas se chamam "baseline" aqui. Não confunda:

| | O que é | Onde mora | Quem muda |
| --- | --- | --- | --- |
| **Base de validação** | o catálogo de regras e o comportamento delas | `src/modelo.rs` (`REGRAS`), `src/regras/`, `src/recursos.rs` | este guia |
| **Baseline de um projeto** | a dívida congelada de um projeto que usa a guarda | `<projeto>/.guarda/baseline.json` | só o humano (`guarda baseline ...`) |

---

## 1. O ciclo completo

```text
fonte de mercado ─► critérios (seção 2) ─► AST (cargo run -- arvore) ─► catálogo + explicação
      ─► implementação + teste ─► medir no corpus (tools/medir.py)
      ─► revisar cada achado (tools/ver_achados.py) ─► ajustar falso positivo + teste
      ─► registrar em docs/fase-N.md ─► (depois de calibrada) promover a erro ─► release
```

Uma regra nova entra sempre como **aviso** (`severidade: Aviso`, `alvo` = severidade final).
Ela só vira erro depois de medida em projeto real com **0 falso positivo conhecido**.

---

## 2. Critérios para aceitar uma regra

Uma regra só entra se passar em todos:

1. **Verificável pela sintaxe** de um arquivo (tree-sitter), ou pelo índice do projeto
   (entidades JPA, exceções criadas, camadas). Nada que precise de tipos resolvidos,
   compilação, rede ou execução.
2. **Origem rastreável**: aponte a regra equivalente (Sonar `S1234`, PMD `NomeDaRegra`,
   detekt `nome-da-regra`, FindSecBugs `PADRAO`, ArchUnit, documentação do Spring). O título
   da explicação leva essa origem entre parênteses.
3. **Não duplica**: rode `cargo run -q -- explicar` (lista todas) e procure o id da origem
   em `src/explicar.rs` (`grep -n "S1066" src/explicar.rs`). Se já existe, melhore a
   regra existente em vez de criar outra.
4. **Segurança exige um sink concreto**: SQL montado com concatenação, arquivo/URL/comando a
   partir de entrada do usuário, segredo literal etc. Desvio de checklist sem um caminho de
   exploração é ponto de atenção, não vulnerabilidade (lição do
   [security-audit-skill](https://github.com/cloudflare/security-audit-skill)).
5. **Correção acionável**: a `dica` diz o que fazer, em português, numa linha.
6. **Determinística**: nada de data, rede, ordem de `HashMap` ou heurística com
   aleatoriedade.

### Fontes (links conferidos em 2026-11-25)

| Fonte | Para quê |
| --- | --- |
| [Sonar Java](https://rules.sonarsource.com/java/) e [Sonar Kotlin](https://rules.sonarsource.com/kotlin/) | bugs, code smells, segurança (ids `S####`) |
| [PMD Java](https://pmd.github.io/pmd/pmd_rules_java.html) | boas práticas, design, erros comuns |
| [detekt](https://detekt.dev/docs/rules/complexity) | Kotlin: complexidade, estilo, coroutines |
| [Find Security Bugs](https://find-sec-bugs.github.io/bugs.htm) | padrões de segurança Java/Spring com sink |
| [OWASP Cheat Sheets](https://cheatsheetseries.owasp.org/) | correção recomendada para a dica |
| [Cloudflare security-audit-skill](https://github.com/cloudflare/security-audit-skill) | como separar achado real de desvio de checklist |
| [ArchUnit](https://www.archunit.org/userguide/html/000_Index.html) | regras de arquitetura (ARQ, NEG) |
| [Spring @Transactional](https://docs.spring.io/spring-framework/reference/data-access/transaction/declarative/annotations.html) | proxy, auto-invocação (SPR) |
| [Consultas do tree-sitter](https://tree-sitter.github.io/tree-sitter/using-parsers/queries/index.html) | regras personalizadas por YAML (`extras.rs`) |

---

## 3. Escolher o código

Códigos são estáveis: **nunca reaproveite um código removido**. Use o próximo número livre
da família:

```bash
grep -o '"SEC0[0-9]*"' src/modelo.rs | sort | tail -1     # último SEC; o novo é +1
```

| Família | Assunto | Arquivo |
| --- | --- | --- |
| `ARQ`, `NEG` | camadas, ciclos, framework no domínio, exceção HTTP no negócio | `src/arquitetura.rs` |
| `CPX` | complexidade, tamanho, aninhamento (várias com catraca) | `complexidade.rs`, `estrutura.rs` |
| `SML` | code smells, exceções, código morto, bloco vazio | `smells.rs`, `excecoes.rs`, `praticas.rs` |
| `CND`, `POL` | guard clause, condição extraída, polimorfismo | `fluxo.rs` |
| `SEC` | segurança em código | `seguranca.rs` (configuração: `src/recursos.rs`) |
| `SPR` | Spring (injeção, proxy, transação, entidade exposta) | `spring.rs` (`SPR004` no `motor.rs`) |
| `BUG` | erro de correção (equals/hashCode, comparação, recurso não fechado) | `bugs.rs` |
| `KT`, `JV` | específicas de Kotlin ou Java | `kotlin.rs`, `java.rs`, `praticas.rs` |
| `TST` | testes (só arquivos de `fontes.testes`) | `testes.rs` |
| `COV`, `SUP`, `GRD`, `PRB` | cobertura, supressão, proteção da guarda, regras do projeto | `cobertura.rs`, `supressao.rs`, `gate.rs`, `extras.rs` |

---

## 4. Receita: regra por arquivo

### 4.1 Veja a árvore primeiro

Java e Kotlin nomeiam os nós de forma diferente. Escreva um exemplo mínimo nas duas
linguagens e confira:

```bash
cargo run -q -- arvore /tmp/Exemplo.java
cargo run -q -- arvore /tmp/Exemplo.kt
```

### 4.2 Catálogo (`src/modelo.rs`)

```rust
r("SEC021", "descrição curta do problema", Aviso, Erro, false),
//          nome                           hoje   alvo  catraca (só regra de limite)
```

### 4.3 Explicação (`src/explicar.rs`)

O teste `toda_regra_tem_explicacao` reprova o build sem ela.

```rust
Explicacao {
    codigo: "SEC021",
    titulo: "Nome da regra (Sonar S1234; FindSecBugs PADRAO)",
    porque: "Por que é problema e o que NÃO conta (os casos isentos).",
    errado: "código errado mínimo",
    certo: "o mesmo código corrigido",
},
```

### 4.4 Implementação

Coloque na família certa (seção 3). O esqueleto de quase toda regra:

```rust
pub fn verificar(ctx: &Contexto, v: &mut Vec<Violacao>) {
    let f = ctx.fonte;
    visitar(ctx.raiz, |n| match n.kind() {
        "method_invocation" | "call_expression" => {
            let Some(c) = chamada(n, f) else { return };   // Java e Kotlin no mesmo formato
            if c.nome == "exec" && c.args.first().is_some_and(|a| montada(*a, f).is_some()) {
                v.push(Violacao::nova(
                    "SEC021", ctx.arquivo, linha(n),
                    format!("comando montado com {}", c.nome),     // mensagem
                    "passe os argumentos separados, sem concatenar", // dica
                    simbolo_dono(n, f),                             // Classe.metodo
                    c.nome,                                         // trecho
                    None,                                           // valor (só catraca)
                ));
            }
        }
        _ => {}
    });
}
```

**O `trecho` entra na impressão digital do baseline** (`código + arquivo + símbolo + trecho`).
Ele tem que ser estável: nada de número de linha, contagem ou texto que muda quando o código
em volta muda. Se o trecho de uma regra existente mudar, todo projeto que congelou essa regra
vê as violações dela como novas.

Arquivo novo em `src/regras/`: declare `pub mod` e chame `verificar` em `por_arquivo` (código
de produção) ou `de_teste` (testes) no `src/regras/mod.rs`.

### 4.5 Utilitários prontos (não reescreva)

| Onde | Função | Para quê |
| --- | --- | --- |
| `lang.rs` | `visitar`, `filhos`, `texto`, `linha`, `simbolo_dono` | percorrer a árvore e localizar |
| `lang.rs` | `eh_funcao`, `eh_lambda`, `corpo_da_funcao`, `eh_comentario` | tipos de nó nas duas linguagens |
| `estrutura.rs` | `percorrer(n, f)` | percorrer com poda (`f` devolve `false` para não descer) |
| `sintaxe.rs` | `chamada(n, f)` → `Chamada { nome, receptor, args, lambda, construtor }` | chamada de método/construtor Java e Kotlin |
| `sintaxe.rs` | `literal`, `conteudo`, `interpolada`, `montada`, `parece_sql` | strings literais, montadas por `+`/template, SQL |
| `sintaxe.rs` | `nome_de_segredo`, `valor_de_segredo`, `formato_de_segredo` | segredos (nome, valor, formato conhecido) |
| `sintaxe.rs` | `anotacoes`, `tem_modificador`, `funcao_dona`, `nome_da_funcao`, `identificadores`, `palavras` | anotações, modificadores, escopo, nomes |
| `fluxo.rs` | `eh_if`, `partes`, `instrucoes`, `if_de_instrucao` | `if` e corpo de bloco nas duas linguagens |
| `excecoes.rs` | `catch(n, f, ling)` → `(tipo, nome, bloco)`, `ignorada_de_proposito` | blocos catch |

### 4.6 Armadilhas das gramáticas

| Situação | Java (`tree-sitter-java`) | Kotlin (`tree-sitter-kotlin-ng`) |
| --- | --- | --- |
| if | `if_statement` | `if_expression` |
| catch | `catch_clause` | `catch_block` |
| chamada | `method_invocation`, `object_creation_expression` | `call_expression` (construtor parece chamada: use `Chamada::construtor`) |
| interpolação | não existe | `${x}` é nó `interpolation`; `$x` **não vira nó**: detecte no texto |
| palavras-chave | nós próprios | `break`, `continue`, `true` aparecem como `identifier` |
| anotação de parâmetro | dentro do `formal_parameter` | no irmão anterior `parameter_modifiers` |
| when / switch | `switch_expression` (vale para o comando também) | `when_expression` > `when_subject`, `when_entry` |
| classe anônima | `object_creation_expression` > `class_body` > `method_declaration` | `object_literal` |

E na API: `named_child` recebe `u32` (`n.named_child(i as u32)`); `named_child_count()`
devolve `usize`.

### 4.7 Testes

- **Unitário** no próprio módulo (`mod casos`, com o `achar(fonte, ling)` que devolve
  `(código, linha)`): um caso que deve acusar e um que não deve, nas duas linguagens quando
  a regra vale para as duas.
- **Ponta a ponta** em `tests/faseN.rs` quando a regra depende do motor (índice, recursos,
  baseline): `comum::projeto()` monta um projeto temporário.
- `tests/cli.rs` compara a saída exata de propósito: se ele quebrar, confira se a mudança
  é a esperada antes de atualizar.

---

## 5. Outros tipos de regra

- **Regra de projeto** (precisa de mais de um arquivo, como `SPR004` ou `NEG001`): o dado
  sai por arquivo no `lang.rs`, vai para a `Analise` (que é guardada no cache) e é cruzado
  no `motor.rs` ou no `arquitetura.rs`. Campo novo na `Analise` leva
  `#[serde(default)]`. A chave do cache já inclui a identidade do executável, então
  binário novo não lê cache velho.
- **Regra de configuração** (`application*.properties|yml`, `.env`, `.pem`, keystore): em
  `src/recursos.rs`, sobre as chaves já achatadas (`spring.datasource.password`).
- **Regra de limite com catraca** (`catraca: true`): passe o valor medido em `valor`; o
  baseline guarda o valor e só aceita que ele caia.
- **Regra de um projeto só**: não vira código. Use `regras.personalizadas` no `guarda.yml`
  (consulta do tree-sitter, `extras.rs`) ou `regras.proibir`.

---

## 6. Calibração em projetos reais

É aqui que a regra ganha confiança. Os scripts ficam em `tools/` (só Python padrão).

### 6.1 Montar o corpus (fora do repositório, sempre cópias)

```bash
python tools/clonar.py <dono-no-github> <pasta-temp>/gh     # clones rasos Java/Kotlin
```

Cópias de projetos locais: copie só os fontes para a pasta temporária. **Nunca rode a
medição no projeto original**: a guarda escreve cache em `.guarda/cache/` e o projeto pode ter
mudanças não commitadas do dono.

### 6.2 Medir antes e depois

```bash
cargo build --release
cp target/release/guarda.exe <pasta-temp>/guarda-antes.exe      # antes da mudança
python tools/medir.py <pasta-temp>/gh --exe <pasta-temp>/guarda-antes.exe --saida antes.json
# ... implemente / ajuste ...
cargo build --release
python tools/medir.py <pasta-temp>/gh --saida depois.json --comparar antes.json
```

`--regras SEC021,SML011` limita a contagem às regras em estudo. O `--comparar` imprime a
tabela "Regra | Antes | Depois" pronta para o `docs/fase-N.md`. `ERR000` tem de ficar em 0.

### 6.3 Revisar cada achado

```bash
python tools/ver_achados.py depois.json SEC021 --limite 40 --antes 2 --depois 3
```

A amostra pega primeiro um achado de cada projeto. Até 40 achados, revise **todos**;
acima disso, revise pelo menos 40 cobrindo todos os projetos. Classifique cada um:

| Classe | Exemplo | O que fazer |
| --- | --- | --- |
| **Real** | `@Autowired private` em campo; senha do banco no `application.properties` | nada; conte na tabela |
| **Falso positivo** | `KEY_SENHA = "senha_key"` (nome de chave, não senha) | ajuste a regra e crie um teste com o caso mínimo |
| **Intencional** | `onClick` vazio de listener anônimo; `catch` que loga e troca por exceção de negócio | isente o padrão, explique no `porque` da explicação e teste |

Para cada falso positivo ou caso intencional, o teste leva o caso mínimo e cita de onde
veio, como o `calibrado_nos_repos_do_github` em `src/regras/praticas.rs`. Prefira isentar
por **estrutura** (o nó pai, o uso da variável) a isentar por nome de projeto ou de arquivo.

**Segredos**: o `ver_achados.py` mascara valores, mas nunca copie um segredo real para doc,
commit, teste ou conversa. No teste, use valores inventados.

### 6.4 Registrar

Em `docs/fase-N.md` (fase atual), uma seção com data: o corpus (quantos projetos, de onde),
a tabela Regra | Antes | Depois | Revisão, os ajustes feitos e a frase final "0 falso
positivo conhecido" (ou o que ficou em aberto). A seção "Revisão nos 29 repositórios do
GitHub" no [fase-4.md](fase-4.md) é o modelo.

---

## 7. Promover a erro

Promova quando a regra tiver passado pela seção 6 sem falso positivo conhecido e a
correção for sempre possível. Basta trocar `severidade` para o `alvo` em `src/modelo.rs` e
anotar no `docs/fase-N.md`.

Lembre o efeito nos projetos: violação de regra **nova** ainda não está no baseline deles.
Depois de atualizar a guarda, o humano congela só as regras novas com
`guarda baseline gerar --regras SEC021 ...`. A IA nunca roda esse comando; ela só o sugere.

---

## 8. Checklist antes do commit

```text
[ ] critérios da seção 2 (origem no título da explicação)
[ ] código novo em modelo::REGRAS (Aviso, alvo) e explicação em explicar.rs
[ ] testes: acusa e não acusa, Java e Kotlin quando valer para os dois
[ ] trecho estável (sem linha, contagem ou texto variável)
[ ] medido no corpus: tabela antes/depois, todos os achados revisados, ERR000 = 0
[ ] docs/fase-N.md com a revisão; README (tabela de regras) e contagem de regras em
    README.md, AGENTS.md, CLAUDE.md e docs/guia-ia.md
[ ] cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

## 9. Release

1. Suba a versão em `Cargo.toml` (`cargo build` atualiza o `Cargo.lock`) e o status e a
   linha do `gh release download` no README.
2. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
3. `cargo install --path . --locked` (os hooks dos projetos usam o binário do PATH).
4. Commit, `git push`, e então `git tag vX.Y.Z && git push origin vX.Y.Z`: o
   `release.yml` compila os 4 alvos e publica. **Peça o OK do humano antes da tag**: o
   repositório é privado e o build de macOS e Windows consome minutos do Actions.
