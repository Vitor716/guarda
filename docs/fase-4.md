# Fase 4 — Ecossistema e regras de design

A especificação define a Fase 4 como ecossistema (MCP, releases, Gradle/Maven) com portão
"contínuo". O Vitor acrescentou regras de design: baixa complexidade ciclomática, guard
clauses, condições extraídas para funções, polimorfismo e regra de negócio como exceção.

## Regras (2026-11-24, v0.4.0)

Todas entram como **aviso**. A NEG001 tem alvo **erro**; as outras ficam como aviso, porque
são questão de estilo e cada uma tem exceção legítima.

| Regra | O que pega | Fonte |
| --- | --- | --- |
| CPX002 | Limite recomendado caiu de 15 (detekt) para **10** (McCabe 1976, NIST SP 500-235); rigoroso 7, mínimo 25 | pedido: baixa complexidade ciclomática |
| CND001 | (a) a função termina num if sem else cujo corpo tem 3+ instruções ou outro bloco de controle; (b) if/else com o caminho principal (3+ instruções) no if e um único return/throw no else | Fowler, Replace Nested Conditional with Guard Clauses |
| CND002 | if de instrução cujo corpo termina em return/throw/break/continue, seguido de `else { }` (não `else if`) | ESLint no-else-return, Pylint R1705 |
| CND003 | condição de if/while/do/ternário com `&&` e `\|\|` juntos; acima do limite de operandos fica com a CPX004 | Fowler, Decompose Conditional |
| POL001 | if / else if com 3+ ramos testando `instanceof`/`is` sobre a mesma variável; `when` com `is` (sealed) não conta | Fowler, Replace Conditional with Polymorphism; IntelliJ "Chain of instanceof checks" |
| POL002 | dois ou mais switch/when no mesmo arquivo sobre o mesmo valor, cada um com 3+ casos constantes (`is` e `else` não contam) | Fowler, smell Repeated Switches |
| NEG001 | exceção HTTP criada na camada de negócio: nomes exclusivos do Spring/JAX-RS ou tipo importado de `org.springframework.web/http`, `jakarta/javax.ws.rs`, `io.ktor`, `io.micronaut.http` | Clean Architecture, hexagonal |
| NEG002 | `IllegalArgumentException`/`IllegalStateException` criada na camada de negócio (inclusive em `orElseThrow`); `require`/`check` do Kotlin não contam | política da guarda |

"Camada de negócio" = `arquitetura.negocio` (padrão: dominio, aplicacao, casos-de-uso,
servico, modelo). Sem arquitetura definida ou detectada, as NEG não rodam.

Decisões que importam para falso positivo:

- **if como valor não conta**: `val x = if (...) a else b`, `return if ...` e `fun f() = if ...`
  ficam de fora das CND (só if em posição de instrução).
- **`else if` não conta** na CND002 (o mesmo padrão do `allowElseIf` do ESLint).
- **CND001 antes da CND002**: no `if (ok) { 3 instruções; return x } else { throw ... }` a
  correção certa é inverter, então sai só a CND001.
- **POL001/POL002 respeitam o Kotlin idiomático**: `when (x) { is A -> ...}` sobre sealed class
  é o jeito certo de fazer o que a regra cobra.
- **NEG001 resolve o tipo pelo import exato**; curinga (`import jakarta.ws.rs.*`) não conta,
  porque uma classe do próprio pacote com o mesmo nome venceria.

## Medição — 12 projetos (cópias dos fontes)

| Projeto | Arquivos | Achados novos |
| --- | --- | --- |
| srv-usuario-api | 282 | CPX002 3, CND001 10, CND002 1, CND003 1, NEG001 1, NEG002 33 |
| srvcondominioapi | 157 | CND001 6, NEG002 11 |
| contas-termometro-project | 142 | POL002 2, NEG002 7 |
| flow-study | 86 | CND001 1, CND002 1, CND003 1, NEG002 26 |
| contas-termometro, mensageria-pedidos, catalogo-optimistic-locking, termometro-dlq, auth-demo, snowflake-id | 103 | 0 |

Revisão de cada achado:

| Regra | Achados | Revisão |
| --- | --- | --- |
| CND001 | 17 | todos reais; 11 são o mesmo padrão `if (data != null) { AtualizarCampoSeNaoNulo... }` nos casos de uso de atualização |
| CND002 | 2 | reais (`return` no if e o fluxo normal no else) |
| CND003 | 2 | reais: `(a \|\| b) && "POST".equals(...)` no filtro JWT, `status !in 200..299 \|\| error != null && !error.isNull` |
| CPX002 | 3 | reais, com o limite novo (12, 11 e 13 > 10); com 15 seriam 0 |
| POL002 | 2 | real: o mesmo `when (contexto.recomendacao)` sobre a String `"VIAVEL"`/`"ARRISCADO"` em dois lugares; pede enum |
| POL001 | 0 | — |
| NEG001 | 1 | real: `orElseThrow(() -> new ResponseStatusException(NOT_FOUND, ...))` num caso de uso do domínio |
| NEG002 | 77 | ~73 regras de negócio reais ("não encontrado", "já existe", transição de status proibida); ~4 são invariantes de programação (`"Perfil SINDICO não encontrado no sistema"`, `"Unidade ID não pode ser nulo para validação"`) — aceitáveis com `guarda:ignorar` e motivo |

Nenhum falso positivo de sintaxe (regra disparando no lugar errado). O que pode incomodar é
o volume da NEG002 em projetos que usam `IllegalArgumentException` como convenção; por isso
ela fica como aviso, e o baseline congela o legado.

## Ecossistema

| Item | Como funciona |
| --- | --- |
| `guarda integrar gradle` | Escreve `gradle/guarda.gradle.kts` (ou `.gradle` em projeto Groovy) com a tarefa `guardaCheck` (Exec) ligada ao `check` de todos os módulos, e acrescenta `apply(from = ...)` entre marcadores no build da raiz. Binário: `-Pguarda.bin`, `GUARDA_BIN` ou `guarda` no PATH. |
| `guarda integrar maven` | Imprime o trecho do exec-maven-plugin 3.7.0 (conferido no Maven Central) na fase `verify`, com `<inherited>false</inherited>` para rodar uma vez só no multi-módulo. Não edita o pom. |
| `guarda baseline aceitar CODIGO ARQUIVO --motivo "..."` | Congela só as violações atuais daquela regra naquele arquivo, com o motivo gravado na entrada (mínimo 10 caracteres). O `--atualizar-baseline` preserva o motivo. A IA não roda: o hook proteger já barra `guarda baseline`. |
| `.github/workflows/ci.yml` | fmt, clippy `-D warnings` e testes em Linux e Windows a cada push em `main` e em PR. |
| `.github/workflows/release.yml` | Ao empurrar uma tag `v*`, compila para Linux x86_64, Windows x86_64, macOS arm64 e x86_64, empacota com SHA-256 e cria a release. |
| Servidor MCP | Já existia desde a Fase 2 (`guarda mcp`). |

Testado de verdade (não só nos testes do repositório):

- Gradle 9.5.1, Kotlin DSL: `./gradlew check` roda a `guardaCheck`; com uma SUP001 o build
  falha com a saída da guarda; também com `--configuration-cache`.
- Gradle 9.5.1, Groovy DSL: `./gradlew guardaCheck -Pguarda.bin=...` passa.
- Maven 3.9.8: `mvn verify` baixa o exec-maven-plugin 3.7.0, roda a guarda e falha com a SUP001.

### Limites conhecidos

- O repositório é privado: baixar a release exige `gh` autenticado. Um script de instalação
  sem `gh` (ou um wrapper Gradle que baixa o binário sozinho) só faz sentido com o repo
  público.
- Os minutos do GitHub Actions em repositório privado contam na cota da conta; o runner macOS
  conta 10× e o Windows 2×. O release roda só quando uma tag é empurrada.
- POL002 olha um arquivo por vez; o mesmo switch espalhado por classes diferentes não aparece.

## Lote 5 + Baselines e calibração em projetos reais (2026-11-24)

### Melhorias no sistema de baselines

- **Recursos no baseline (`SEC001`/`SEC017`/`SEC018`)**: arquivos de configuração/segredo
  verificados por `recursos::verificar` agora entram no conjunto de analisados do gate; corrigir
  um segredo em `application.properties` ou `application.yml` remove a entrada em
  `check --atualizar-baseline`.
- **Arquivos removidos (fim de entradas zumbis)**: em verificação do projeto inteiro
  (`Pedido::Tudo`) e em `--alterados`, arquivos deletados/renomeados são contados como corrigidos
  e removidos do baseline em `--atualizar-baseline`.
- **Catraca determinística em sobrecargas**: pareamento do maior valor atual para o menor por
  chave, evitando falso positivo quando há dois métodos sobrecarregados com o mesmo símbolo e
  valores diferentes no baseline.
- **`SML001` (God Class) com catraca**: guarda a complexidade total (WMC), impedindo que uma
  God Class congelada continue crescendo.
- **`guarda baseline aceitar`**: agora atualiza catracas que cresceram (gravando novo limite e
  motivo) e aceita filtros cirúrgicos `--linha <N>` e `--simbolo <S>`.
- **`guarda baseline gerar`**: preserva os `motivo`s já registrados em vez de apagá-los,
  mostra o delta (`+novas, -removidas, motivos preservados`) e aceita `--regras CODIGO...` para
  congelar só regras recém-ativadas sem tocar nas demais.
- **`guarda baseline status`**: diagnóstico da saúde do baseline (entradas por regra, legado ×
  aceitas com motivo, corrigidas, arquivos removidos e catracas que podem apertar).

### Calibração e novos cenários validados

- **`SEC015` (log com dado sensível)**: corrigidos 9/10 falsos positivos no `srv-usuario-api`
  causados por chamadas a getters seguros sobre objetos com "token" no nome (`dadosToken.usuarioId()`,
  `dadosToken.perfis()`). Passou a capturar também `authHeader`/`authorization` logados diretamente
  (encontrou vazamento real do header `Authorization` na l.73 do `JwtAutenticacaoFilter.java`).
- **`SEC012` (rota liberada)**: `requestMatchers(HttpMethod.OPTIONS, "/**").permitAll()` (preflight
  CORS) não gera mais falso positivo.
- **`SEC020` (open redirect)**: `"redirect:/rota-fixa/" + id` fica travado no mesmo host e não
  gera mais falso positivo (validado no `spring-projects/spring-petclinic`); `"redirect:" + url`,
  `"redirect:/" + path` e `"redirect:https://" + host` continuam sendo cobrados.
- **`SEC017` (configuração insegura)**: ampliada para `server.error.include-stacktrace: on_param`/`on-param`,
  `server.error.include-exception: true`, `management.endpoint.health.show-details: always`,
  `spring.h2.console.enabled: true` e exposição explícita de endpoints sensíveis do Actuator
  (`env`, `heapdump`, `shutdown`, `configprops`, `beans`, `mappings`, `threaddump`, `logfile`).
- **`SEC001`**: adicionada detecção de credenciais embutidas em URLs (`jdbc:postgresql://user:pass@host`,
  `redis://`, `mongodb://`, `amqp://`) e chaves `sk-proj-`/`sk-ant-`.
- **Hook `pos-edicao`**: agora também verifica arquivos de configuração/segredo (`application*.yml|properties`, `.env`).
- **Novas regras (entram como aviso)**:
  - `SPR006` (`@Transactional` em `@RestController`/`@Controller`, alvo: aviso).
  - `BUG009` (`assert` em código de produção Java — Sonar S4274, alvo: erro).
  - `TST004` (asserção tautológica em teste: `assertTrue(true)`, `assertEquals(x, x)`, `x shouldBe x`, `assertNotNull("...")` — Sonar S2701/S5863, alvo: erro; fecha a brecha contra IA tentando burlar `TST001`).

### Medição — 4 projetos locais (cópias dos fontes, 0 erros de parse)

| Projeto | Arquivos | Tempo s/ cache | Achados Lote 5 + novos | Falsos positivos |
| --- | --- | --- | --- | --- |
| srv-usuario-api | 282 Java | 0,13 s | SEC001 4, SEC008 2, SEC012 1, SEC014 1, SEC015 2, SEC017 1, SPR001 1, SPR006 21, SML010 1, SML011 1, CND004 1, JV006 4 | 0 (SEC015: 9 FPs eliminados; SEC012: 1 FP eliminado) |
| srvcondominioapi | 157 Java | 0,08 s | SEC001 2, SEC017 2, SPR006 4, BUG009 1, CND004 2 | 0 |
| flow-study | 83 Kotlin | 0,09 s | 0 | 0 |
| contas-termometro-project | 127 Kotlin + 12 Java | 0,12 s | SEC002 2, SML011 1 | 0 |
