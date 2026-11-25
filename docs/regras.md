# Catálogo de regras

<!-- Gerado pelo teste `catalogo_em_docs_esta_em_dia` (src/explicar.rs). Não edite à mão:
     GUARDA_ATUALIZAR_DOCS=1 cargo test catalogo -->

94 regras. **Hoje** é a severidade padrão; **alvo** é a que a regra terá depois de calibrada em projetos reais (o perfil `rigoroso` já usa o alvo). Regras com **catraca** guardam o valor no baseline e só aceitam que ele caia.

Para o porquê e um exemplo errado e certo: `guarda explicar <CODIGO>`. Para mudar a severidade ou desligar uma regra: `regras.severidade` e `regras.desligar` no `guarda.yml`.

## Leitura

| Código | Regra (origem) | Hoje | Alvo | Catraca |
| --- | --- | --- | --- | --- |
| ERR000 | Arquivo que o parser não leu | erro | erro |  |

## Arquitetura

| Código | Regra (origem) | Hoje | Alvo | Catraca |
| --- | --- | --- | --- | --- |
| ARQ001 | Dependência proibida entre camadas | erro | erro |  |
| ARQ002 | Ciclo entre fatias do projeto (ArchUnit slices) | aviso | erro |  |
| ARQ003 | Framework dentro do domínio | aviso | erro |  |
| ARQ004 | Controlador fora da camada de entrada (Konsist) | aviso | erro |  |

## Regra de negócio

| Código | Regra (origem) | Hoje | Alvo | Catraca |
| --- | --- | --- | --- | --- |
| NEG001 | Exceção HTTP na camada de negócio (Clean Architecture, hexagonal) | aviso | erro |  |
| NEG002 | Regra de negócio com exceção genérica | aviso | aviso |  |

## Complexidade e tamanho

| Código | Regra (origem) | Hoje | Alvo | Catraca |
| --- | --- | --- | --- | --- |
| CPX001 | Complexidade cognitiva (Sonar S3776) | erro | erro | sim |
| CPX002 | Complexidade ciclomática (detekt, PMD) | aviso | erro | sim |
| CPX003 | Aninhamento profundo (detekt NestedBlockDepth) | aviso | erro | sim |
| CPX004 | Condição com muitos operadores (detekt ComplexCondition) | aviso | erro |  |
| CPX005 | Função longa (detekt LongMethod) | erro | erro | sim |
| CPX006 | Classe longa (detekt LargeClass) | aviso | erro | sim |
| CPX007 | Muitos parâmetros (detekt LongParameterList) | aviso | erro | sim |
| CPX008 | Muitas funções na classe (detekt TooManyFunctions) | aviso | erro | sim |
| CPX009 | Muitos return (detekt ReturnCount) | aviso | erro | sim |

## Condições e guard clauses

| Código | Regra (origem) | Hoje | Alvo | Catraca |
| --- | --- | --- | --- | --- |
| CND001 | Retorne cedo: guard clause (Fowler, Replace Nested Conditional with Guard Clauses) | aviso | aviso |  |
| CND002 | else depois de return/throw (ESLint no-else-return, Pylint R1705) | aviso | aviso |  |
| CND003 | Condição que mistura && e \|\| (Fowler, Decompose Conditional) | aviso | aviso |  |
| CND004 | if aninhado que pode juntar (Sonar S1066, PMD CollapsibleIfStatements) | aviso | aviso |  |

## Polimorfismo

| Código | Regra (origem) | Hoje | Alvo | Catraca |
| --- | --- | --- | --- | --- |
| POL001 | Cadeia de instanceof/is (Fowler, Replace Conditional with Polymorphism) | aviso | aviso |  |
| POL002 | switch/when repetido (Fowler, smell Repeated Switches) | aviso | aviso |  |

## Code smells

| Código | Regra (origem) | Hoje | Alvo | Catraca |
| --- | --- | --- | --- | --- |
| SML001 | God Class (aproximação do PMD GodClass) | aviso | aviso | sim |
| SML002 | Código duplicado (PMD CPD, Sonar) | aviso | aviso |  |
| SML003 | Número mágico (detekt MagicNumber, Sonar S109) | aviso | aviso |  |
| SML004 | catch genérico (detekt TooGenericExceptionCaught) | aviso | erro |  |
| SML005 | Exceção engolida ou catch vazio (detekt SwallowedException, EmptyCatchBlock) | aviso | erro |  |
| SML006 | Lançar exceção genérica (detekt TooGenericExceptionThrown) | aviso | erro |  |
| SML007 | Código comentado (Sonar S125) | aviso | aviso |  |
| SML008 | Membro privado não usado (PMD UnusedPrivateField/Method) | aviso | aviso |  |
| SML009 | TODO/FIXME sem ticket (detekt ForbiddenComment) | aviso | aviso |  |
| SML010 | Bloco vazio (Sonar S108, S1186; detekt empty-blocks) | aviso | aviso |  |
| SML011 | Relançar sem a causa (PMD PreserveStackTrace) | aviso | aviso |  |

## Correção (bugs)

| Código | Regra (origem) | Hoje | Alvo | Catraca |
| --- | --- | --- | --- | --- |
| BUG001 | Desvio dentro de finally (Sonar S1143, detekt ReturnFromFinally, PMD ReturnFromFinallyBlock) | aviso | erro |  |
| BUG002 | Expressões idênticas (Sonar S1764) | aviso | erro |  |
| BUG003 | Auto-atribuição (Sonar S1656) | aviso | erro |  |
| BUG004 | Condição repetida (Sonar S1862) | aviso | erro |  |
| BUG005 | Ramos idênticos (Sonar S1871, S3923) | aviso | erro |  |
| BUG006 | String com == (Sonar S4973) | aviso | erro |  |
| BUG007 | BigDecimal de double (Sonar S2111, PMD AvoidDecimalLiteralsInBigDecimalConstructor) | aviso | erro |  |
| BUG008 | InterruptedException ignorada (Sonar S2142) | aviso | erro |  |
| BUG009 | assert em código de produção Java (Sonar S4274) | aviso | erro |  |

## Segurança

| Código | Regra (origem) | Hoje | Alvo | Catraca |
| --- | --- | --- | --- | --- |
| SEC001 | Credencial fixa (Sonar S2068, S6418, S6437; Cloudflare security-audit) | aviso | erro |  |
| SEC002 | SQL montado com concatenação (Sonar S2077, FindSecBugs SQL_INJECTION, CWE-89) | aviso | erro |  |
| SEC003 | Comando do sistema montado (Sonar S2076/S4721, FindSecBugs COMMAND_INJECTION, CWE-78) | aviso | erro |  |
| SEC004 | Hash fraco (Sonar S4790, S5344; CWE-328, CWE-916) | aviso | erro |  |
| SEC005 | Cifra fraca ou modo inseguro (Sonar S5547, S5542; CWE-327) | aviso | erro |  |
| SEC006 | Aleatório previsível em contexto de segredo (Sonar S2245; Cloudflare: weak randomness) | aviso | aviso |  |
| SEC007 | TLS sem verificação (Sonar S4830, S5527, S4423; CWE-295) | aviso | erro |  |
| SEC008 | CORS aberto (Sonar S5122; CWE-942) | aviso | aviso |  |
| SEC009 | Desserialização insegura (Sonar S4544, FindSecBugs OBJECT_DESERIALIZATION; CWE-502) | aviso | erro |  |
| SEC010 | XML sem proteção contra XXE (Sonar S2755, FindSecBugs XXE; CWE-611) | aviso | erro |  |
| SEC011 | CSRF desligado (Sonar S4502; CWE-352) | aviso | aviso |  |
| SEC012 | Rota liberada ampla ou sensível (Cloudflare security-audit, "obvious things") | aviso | aviso |  |
| SEC013 | Cookie sem Secure/HttpOnly (Sonar S2092, S3330; CWE-614, CWE-1004) | aviso | aviso |  |
| SEC014 | JWT sem verificação (Sonar S5659; CWE-347) | aviso | erro |  |
| SEC015 | Dado sensível no log (CWE-532; Cloudflare: secrets in logs) | aviso | aviso |  |
| SEC016 | TODO de segurança (Cloudflare security-audit, "obvious things") | aviso | aviso |  |
| SEC017 | Configuração insegura (Sonar S4507; Cloudflare: debug gated) | aviso | erro |  |
| SEC018 | Arquivo de segredo versionável (Cloudflare security-audit, "obvious things") | aviso | erro |  |
| SEC019 | IP fixo (Sonar S1313) | aviso | aviso |  |
| SEC020 | Entrada do request em sorvedouro (Sonar S2083, S5144, S5146, S3649; Cloudflare: trace untrusted input to sink) | aviso | erro |  |

## Spring

| Código | Regra (origem) | Hoje | Alvo | Catraca |
| --- | --- | --- | --- | --- |
| SPR001 | Injeção por campo (Sonar S6813) | aviso | aviso |  |
| SPR002 | @Transactional/@Async em método private (Sonar S2230) | aviso | erro |  |
| SPR003 | Chamada pelo this a @Transactional/@Async (Sonar S6809) | aviso | erro |  |
| SPR004 | Entidade no @RequestBody (Sonar S4684; CWE-915) | aviso | erro |  |
| SPR005 | @RequestMapping sem método (Sonar S3752; CWE-352) | aviso | aviso |  |
| SPR006 | @Transactional em controlador HTTP (Spring, arquitetura em camadas) | aviso | aviso |  |

## Kotlin

| Código | Regra (origem) | Hoje | Alvo | Catraca |
| --- | --- | --- | --- | --- |
| KT001 | Uso de !! (detekt UnsafeCallOnNullableType) | erro | erro |  |
| KT002 | GlobalScope.launch/async (detekt GlobalCoroutineUsage) | aviso | erro |  |
| KT003 | Thread.sleep em corrotina (detekt SleepInsteadOfDelay) | aviso | erro |  |
| KT004 | suspend fun que retorna Flow (detekt SuspendFunWithFlowReturnType) | aviso | erro |  |
| KT005 | Dispatchers fixo em vez de injetado (detekt InjectDispatcher) | aviso | aviso |  |
| KT006 | data class com var (detekt DataClassShouldBeImmutable) | aviso | aviso |  |
| KT007 | println/printStackTrace em produção (detekt ForbiddenMethodCall) | aviso | erro |  |
| KT008 | Estado mutável exposto (Sonar S6305) | aviso | aviso |  |

## Java

| Código | Regra (origem) | Hoje | Alvo | Catraca |
| --- | --- | --- | --- | --- |
| JV001 | System.out/printStackTrace em produção (PMD SystemPrintln) | aviso | erro |  |
| JV002 | Recurso sem try-with-resources (PMD UseTryWithResources) | aviso | erro |  |
| JV003 | equals sem hashCode (PMD OverrideBothEqualsAndHashcode) | aviso | erro |  |
| JV004 | Optional como campo ou parâmetro (Sonar S3553) | aviso | aviso |  |
| JV005 | java.util.Date/Calendar | aviso | aviso |  |
| JV006 | Literal à direita no equals (PMD LiteralsFirstInComparisons) | aviso | aviso |  |

## Testes

| Código | Regra (origem) | Hoje | Alvo | Catraca |
| --- | --- | --- | --- | --- |
| TST001 | Teste sem asserção (PMD JUnitTestsShouldIncludeAssert) | aviso | erro |  |
| TST002 | Thread.sleep em teste (Sonar S2925) | aviso | aviso |  |
| TST003 | Teste desativado sem motivo (Sonar S1607) | aviso | aviso |  |
| TST004 | Asserção sempre verdadeira em teste (Sonar S2701, S5863) | aviso | erro |  |

## Cobertura

| Código | Regra (origem) | Hoje | Alvo | Catraca |
| --- | --- | --- | --- | --- |
| COV000 | Os testes falharam ao gerar a cobertura | erro | erro |  |
| COV001 | Cobertura do código novo (Sonar way: ≥ 80%) | erro | erro |  |
| COV002 | Cobertura mínima por pacote | erro | erro | sim |
| COV003 | Cobertura geral mínima | erro | erro | sim |

## Supressão

| Código | Regra (origem) | Hoje | Alvo | Catraca |
| --- | --- | --- | --- | --- |
| SUP001 | Supressão sem motivo | erro | erro |  |

## Proteção da guarda

| Código | Regra (origem) | Hoje | Alvo | Catraca |
| --- | --- | --- | --- | --- |
| GRD001 | guarda.yml ou baseline alterado | aviso | aviso |  |

## Proibições do projeto

| Código | Regra (origem) | Hoje | Alvo | Catraca |
| --- | --- | --- | --- | --- |
| PRB001 | Import proibido pelo guarda.yml | erro | erro |  |
| PRB002 | Tipo proibido no pacote pelo guarda.yml | erro | erro |  |
