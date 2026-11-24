# Fase 0 — medições

Portão da fase: **menos de 1% de arquivos com erro de parse** e **menos de 1 s no projeto
todo**, medidos em projeto real, não em exemplo.

## srv-usuario-api (Condomínio v2) — 2026-11-24

Spring Boot 3.4, Java 21, Maven, um módulo. Comando: `guarda medir` e depois `guarda init`.

| Medida | Resultado |
| --- | --- |
| Arquivos analisados | 277 Java (11.292 linhas), `src/main/**` |
| Erro de parse (ERR000) | 0 (0,00%) |
| Tempo, projeto todo | 0,04 s com cache de disco quente; ~2 s na primeiríssima execução do binário (disco frio e antivírus) |
| Tempo por arquivo | 0,49 ms em média; o mais lento 5,4 ms |
| Arquitetura detectada | hexagonal, confiança 67% (clean 67%, layered 55%) |
| Baseline | 183 violações congeladas em 3 regras |

**Portão: passou** (parse 0% < 1%; 0,04 s < 1 s).

Violações congeladas:

| Regra | Qtde | O que é |
| --- | --- | --- |
| ARQ001 | 180 | 113 `domain` → `infrastructure` (casos de uso importando `*Entity`, `*Mapper`, `FormatadorCpf`, exceções); 47 `application` → `infrastructure`; 12 `endpoint` → `infrastructure`; 5 `domain` → `application` (portas e serviços); 2 portas de `application` importando DTOs de `endpoint`; 1 configuração de segurança importando o filtro HTTP |
| CPX001 | 2 | `UnidadeAcessoAspect.extrairUnidadeId` (23) e `JwtAutenticacaoFilter.doFilterInternal` (17) |
| CPX005 | 1 | `JwtAutenticacaoFilter.doFilterInternal` (67 linhas) |

### Falsos positivos

Revisei por amostragem: todas as categorias de ARQ001 acima conferem com o código, e a
complexidade 23 de `extrairUnidadeId` foi recontada à mão pelas regras do Sonar (for 1, dois
`if` aninhados 2+2, dois ternários 3+3, quatro `if` em nível 2: 3+3+3+3). Nenhum falso
positivo encontrado nesta amostra.

### O que a medição mudou no código

- A primeira versão da detecção pontuava só por arquivos classificados × arquivos sem
  violação e escolheu `layered` (76%): ele deixa `infrastructure/` inteiro sem camada e por
  isso "não vê" as 113 violações de domínio. A nota passou a usar os pacotes de primeiro
  nível (a decisão de arquitetura do projeto) e a pesar as violações pela metade.
- `medir` passou a separar a execução a frio da quente.

### Pendências e limites conhecidos

- **Kotlin não foi medido em projeto real**: só nos testes. O portão de parse da gramática
  `kotlin-ng` ainda precisa de um projeto Kotlin de verdade (e da comparação com a
  gramática do fwcd, como pede a especificação).
- Empate hexagonal × clean (67% cada) é resolvido pela ordem dos presets; os dois classificam
  este projeto da mesma forma. Um critério melhor de desempate fica para a Fase 3.
- `adapter/` (só com `adapter/in/rest`) e `config/` não entram em camada nenhuma.
- ARQ001 ainda não olha nomes totalmente qualificados no corpo (`com.app.infra.Http()`),
  só imports; a especificação prevê os dois.
- `?:` (elvis) não soma na complexidade cognitiva do Kotlin. **Verificar** se o Sonar Kotlin
  conta antes de comparar números na Fase 2.
