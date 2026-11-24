# Fase 3 — Catálogo completo

Portão da fase: **o `init` acerta o preset de arquitetura em 3 projetos diferentes**
(de preferência um deles em Kotlin, que ainda não foi medido em código real).

A fase é entregue em lotes. Toda regra nova entra como **aviso** (risco da especificação:
"regra nova entra como aviso antes de virar erro"); o catálogo guarda a severidade alvo
(`modelo::REGRAS`, campo `alvo`) e `guarda explicar` mostra as duas.

## Lote 1 — 29 regras (2026-11-24)

| Grupo | Regras | Origem |
| --- | --- | --- |
| Arquitetura | ARQ002 ciclo entre fatias, ARQ003 framework no domínio, ARQ004 controlador fora da entrada | ArchUnit slices, Clean Architecture, Konsist |
| Complexidade | CPX002 ciclomática, CPX003 aninhamento, CPX004 condição, CPX006 classe longa, CPX007 parâmetros, CPX008 funções por classe, CPX009 return | detekt, PMD |
| Smells | SML001 God Class, SML003 número mágico, SML004 catch genérico, SML005 exceção engolida, SML006 exceção genérica lançada, SML007 código comentado, SML008 membro privado não usado, SML009 TODO sem ticket | detekt, PMD, Sonar |
| Java | JV001 System.out/printStackTrace, JV002 recurso sem try-with-resources, JV003 equals sem hashCode, JV004 Optional como campo ou parâmetro, JV005 java.util.Date/Calendar | PMD, Sonar |
| Kotlin | KT002 GlobalScope, KT003 Thread.sleep em corrotina, KT004 suspend fun com Flow, KT005 Dispatchers fixo, KT006 data class com var, KT007 println/printStackTrace | detekt |

Configuração nova no `guarda.yml`:

```yaml
limites:                         # padrões do detekt; violação é acima do limite
  complexidade-ciclomatica: 15
  aninhamento: 4
  condicao-operadores: 4
  linhas-classe: 600
  parametros: 6
  funcoes-por-classe: 11
  retornos: 2
arquitetura:
  ciclos: proibir                # ou permitir (desliga ARQ002)
  sem-framework: [dominio]       # camadas que não importam framework (ARQ003)
  frameworks: [org.springframework, jakarta.persistence, ...]
```

Detalhes de implementação que importam para falso positivo:

- **ARQ002**: fatia = primeiro pacote abaixo da base; componentes fortemente conexos do
  grafo de imports entre fatias. Cada arquivo recebe uma violação por fatia de destino que
  fecha o ciclo (`application → domain fecha um ciclo entre fatias (...)`).
- **ARQ003**: só imports de fora do projeto, por prefixo com limite de segmento
  (`android` não casa `androidx`).
- **SML003**: não contam -1, 0, 1, 2, constantes (`static final`, `const val`, companion),
  anotações, argumento nomeado (Kotlin), enum e `hashCode`.
- **SML005**: ignora variável `ignored`/`expected`/`_` e as exceções que o detekt ignora
  (`NumberFormatException`, `InterruptedException`, `MalformedURLException`, `ParseException`).
- **SML007**: heurística de linha de código; Javadoc/KDoc, URLs e divisores de seção ficam de
  fora; comentários de linha seguidos contam uma vez.
- **SML008**: membros anotados e classes anotadas (Lombok, JPA, Spring) ficam de fora.
- **KT005**: `Dispatchers.IO` como valor padrão de parâmetro é aceito (como no detekt).

### Medição — cópia do srv-usuario-api (277 arquivos Java)

| Regra | Achados | Revisão |
| --- | --- | --- |
| ARQ002 | 211 | ciclo real entre as 4 fatias (application, domain, endpoint, infrastructure) |
| ARQ003 | 178 | reais: 154 Spring, 15 JPA, 5 Jackson, 4 Hibernate Validator no `domain` |
| ARQ004 | 2 | reais: `@RestController` em `application/controller`, `@ControllerAdvice` em `infrastructure` |
| CPX007 / CPX008 / CPX009 | 3 / 2 / 7 | reais |
| JV001 | 26 | reais (`System.out.println` de depuração) |
| JV005 | 1 | real (`java.util.Date`) |
| SML003 | 11 | reais (`substring(7)`, timeouts, porta 80) |
| SML004 / SML006 | 17 / 4 | reais (`catch (Exception)`, `throw new RuntimeException`) |
| SML007 | 28 | reais (classes e métodos inteiros comentados) |
| SML009 | 2 | reais |
| SML001, SML005, SML008, CPX002-004, CPX006, JV002-004 | 0 | — |

A primeira rodada achou 4 desvios, corrigidos antes do commit: SML005 marcava
`NumberFormatException` (o detekt ignora), SML009 apontava a linha do `/**` em vez da do TODO,
SML007 tomava `// ===== Auxiliares =====` por código, e a mensagem da ARQ002 descrevia um
ciclo de 2 fatias quando o real tinha 4.

Tempo do projeto todo com as 41 regras: 0,14–0,16 s.

Também: o cache passou a usar a identidade do executável na chave, porque, durante o
desenvolvimento, regra alterada sem mudar a versão lia resultado velho do cache.

### Limites conhecidos

- Regras Kotlin (KT002-007) só medidas nos testes: falta projeto Kotlin real.
- A gramática Kotlin não aceita `;` separando propriedades no mesmo `companion object`
  (o arquivo vira ERR000).
- SML001 é aproximação sintática do GodClass do PMD (sem ATFD, que exige tipos).
- O binário instalado continua na 0.2.0 até o fim da semana de teste da Fase 1 no
  srv-usuario-api (~2026-12-01); depois, reinstalar e regerar o baseline de lá.

## Lote 2 (a fazer)

TST001-003 (exige analisar `src/test`), presets `mvvm` e `modular`, `regras.proibir`,
regras personalizadas em YAML, `perfil`/`estende`, saída SARIF e modo texto.
