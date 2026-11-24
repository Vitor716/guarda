//! `guarda explicar <CODIGO>`: a explicação longa de cada regra (por quê, errado, certo),
//! só sob demanda, para a saída padrão continuar curta.

pub struct Explicacao {
    pub codigo: &'static str,
    pub titulo: &'static str,
    pub porque: &'static str,
    pub errado: &'static str,
    pub certo: &'static str,
}

pub const EXPLICACOES: &[Explicacao] = &[
    Explicacao {
        codigo: "TST001",
        titulo: "Teste sem asserção (PMD JUnitTestsShouldIncludeAssert)",
        porque: "Teste que não verifica nada passa sempre: ele só mostra que o código não explodiu. Conta como asserção assert*, verify*, expect*, should*, then, check*, andExpect, coVerify e @Test(expected = ...).",
        errado: "@Test void salva() { servico.salvar(pedido); }",
        certo: "@Test void salva() { servico.salvar(pedido); verify(repo).save(pedido); }",
    },
    Explicacao {
        codigo: "TST002",
        titulo: "Thread.sleep em teste (Sonar S2925)",
        porque: "Esperar um tempo fixo deixa o teste lento e instável: no CI o tempo pode não bastar.",
        errado: "enviar(); Thread.sleep(2000); assertTrue(recebido());",
        certo: "enviar(); await().atMost(5, SECONDS).until(this::recebido);",
    },
    Explicacao {
        codigo: "TST003",
        titulo: "Teste desativado sem motivo (Sonar S1607)",
        porque: "Teste desativado sem motivo vira esquecido. Diga por quê e o ticket para reativar.",
        errado: "@Disabled @Test void calcula() { ... }",
        certo: "@Disabled(\"ABC-12: depende do serviço de frete\") @Test void calcula() { ... }",
    },
    Explicacao {
        codigo: "PRB001",
        titulo: "Import proibido pelo guarda.yml",
        porque: "O time proibiu este import em regras.proibir.imports (ex.: uma biblioteca descontinuada).",
        errado: "import org.joda.time.DateTime;",
        certo: "import java.time.ZonedDateTime;",
    },
    Explicacao {
        codigo: "PRB002",
        titulo: "Tipo proibido no pacote pelo guarda.yml",
        porque: "O time proibiu este tipo nestes pacotes em regras.proibir.tipos-em-pacote (ex.: Double em cálculo monetário).",
        errado: "// em ..calculo..\nDouble total = preco * quantidade;",
        certo: "BigDecimal total = preco.multiply(quantidade);",
    },
    Explicacao {
        codigo: "ARQ002",
        titulo: "Ciclo entre fatias do projeto (ArchUnit slices)",
        porque: "Fatia é cada pacote de primeiro nível abaixo do pacote base (domain, application, infrastructure...). Se A importa B e B importa A, as duas não podem mais mudar nem ser testadas separadamente.",
        errado: "// domain/ importa infrastructure/ e infrastructure/ importa domain/",
        certo: "// só infrastructure/ importa domain/; domain/ declara interfaces (portas)",
    },
    Explicacao {
        codigo: "ARQ003",
        titulo: "Framework dentro do domínio",
        porque: "O domínio guarda a regra de negócio e deve ser Kotlin/Java puro. Anotação de JPA, Spring ou Jackson ali prende a regra ao framework e ao banco. Configurável em arquitetura.sem-framework e arquitetura.frameworks.",
        errado: "// em domain/\nimport jakarta.persistence.Entity;\n@Entity class Usuario { ... }",
        certo: "// em domain/: class Usuario { ... }\n// em infrastructure/: @Entity class UsuarioEntity { ... } + mapper",
    },
    Explicacao {
        codigo: "ARQ004",
        titulo: "Controlador fora da camada de entrada (Konsist)",
        porque: "@RestController, @Controller e @ControllerAdvice são a porta de entrada HTTP: ficam na camada de entrada (adaptadores-entrada, entrada ou web).",
        errado: "// em application/controller/\n@RestController class UsuarioController",
        certo: "// em adapter/in/rest/\n@RestController class UsuarioController",
    },
    Explicacao {
        codigo: "CPX002",
        titulo: "Complexidade ciclomática (detekt, PMD)",
        porque: "Número de caminhos independentes da função: 1 + cada if, laço, case, catch, ternário e operador && ou ||. Cada caminho precisa de um teste. O limite recomendado é 10, o de McCabe (1976) e da NIST SP 500-235; o detekt usa 15.",
        errado: "fun tarifa(...) { if ... else if ... when (...) { 8 casos } ... }",
        certo: "fun tarifa(...) = regras.first { it.aplica(...) }.valor(...)",
    },
    Explicacao {
        codigo: "CPX003",
        titulo: "Aninhamento profundo (detekt NestedBlockDepth)",
        porque: "Blocos dentro de blocos (if, laço, try, switch/when) além do limite escondem o caminho principal.",
        errado: "for (...) { if (...) { try { if (...) { while (...) { } } } } }",
        certo: "// retorne cedo (guard clause) e extraia o miolo em funções",
    },
    Explicacao {
        codigo: "CPX004",
        titulo: "Condição com muitos operadores (detekt ComplexCondition)",
        porque: "Uma condição com muitos && e || é difícil de ler e de testar por inteiro.",
        errado: "if (a && b || c && !d || e) { ... }",
        certo: "val podeAprovar = a && b\nval excecao = c && !d\nif (podeAprovar || excecao || e) { ... }",
    },
    Explicacao {
        codigo: "CPX006",
        titulo: "Classe longa (detekt LargeClass)",
        porque: "Linhas de código da classe, sem brancos e comentários. Classe grande costuma ter mais de uma responsabilidade.",
        errado: "class PedidoService { /* 900 linhas */ }",
        certo: "class CriarPedido, class CancelarPedido, class CalcularFrete",
    },
    Explicacao {
        codigo: "CPX007",
        titulo: "Muitos parâmetros (detekt LongParameterList)",
        porque: "Função com muitos parâmetros é difícil de chamar certo; os parâmetros que andam juntos são um tipo que ainda não existe.",
        errado: "fun criar(nome, cpf, rua, numero, cidade, uf, cep, email)",
        certo: "fun criar(pessoa: Pessoa, endereco: Endereco)",
    },
    Explicacao {
        codigo: "CPX008",
        titulo: "Muitas funções na classe (detekt TooManyFunctions)",
        porque: "Muitas funções numa classe indicam responsabilidades misturadas.",
        errado: "class UsuarioService { 25 funções }",
        certo: "// separe por caso de uso ou por agregado",
    },
    Explicacao {
        codigo: "CPX009",
        titulo: "Muitos return (detekt ReturnCount)",
        porque: "Muitos pontos de saída dificultam seguir a função. Retornos dentro de lambdas não contam.",
        errado: "fun f(): Int { if (a) return 1; if (b) return 2; if (c) return 3; return 4 }",
        certo: "fun f(): Int = when { a -> 1; b -> 2; c -> 3; else -> 4 }",
    },
    Explicacao {
        codigo: "SML001",
        titulo: "God Class (aproximação do PMD GodClass)",
        porque: "Classe muito complexa (soma da complexidade ciclomática dos métodos ≥ 47) e pouco coesa (métodos que quase não compartilham campos). Sem resolução de tipos, é uma aproximação: por isso é só aviso.",
        errado: "class Sistema { 40 métodos que mexem em coisas diferentes }",
        certo: "// quebre pelos grupos de métodos que usam os mesmos campos",
    },
    Explicacao {
        codigo: "SML003",
        titulo: "Número mágico (detekt MagicNumber, Sonar S109)",
        porque: "Número solto no código não diz o que significa. Não contam -1, 0, 1, 2, constantes (static final, const val, companion), anotações e hashCode.",
        errado: "if (idade >= 18) { ... }",
        certo: "private const val MAIORIDADE = 18\nif (idade >= MAIORIDADE) { ... }",
    },
    Explicacao {
        codigo: "SML004",
        titulo: "catch genérico (detekt TooGenericExceptionCaught)",
        porque: "Pegar Exception, Throwable ou RuntimeException captura também os erros que você não esperava (inclusive bugs) e esconde a causa.",
        errado: "try { ... } catch (Exception e) { ... }",
        certo: "try { ... } catch (IOException e) { ... }",
    },
    Explicacao {
        codigo: "SML005",
        titulo: "Exceção engolida ou catch vazio (detekt SwallowedException, EmptyCatchBlock)",
        porque: "Um catch que não relança nem usa a exceção perde o erro e a pilha. Nomeie a variável ignored, expected ou _ quando ignorar for intencional.",
        errado: "catch (e: IOException) { log.warn(\"falhou\") }",
        certo: "catch (e: IOException) { log.warn(\"falhou\", e) }\n// ou: throw ArquivoInvalido(caminho, e)",
    },
    Explicacao {
        codigo: "SML006",
        titulo: "Lançar exceção genérica (detekt TooGenericExceptionThrown)",
        porque: "Exception, RuntimeException, Throwable e Error não dizem o que deu errado nem deixam o chamador tratar o caso.",
        errado: "throw RuntimeException(\"usuário não encontrado\")",
        certo: "throw UsuarioNaoEncontrado(id)",
    },
    Explicacao {
        codigo: "CND001",
        titulo: "Retorne cedo: guard clause (Fowler, Replace Nested Conditional with Guard Clauses)",
        porque: "Quando o trabalho todo da função fica dentro de um if, ou o caminho principal fica no if e a saída (return/throw) no else, o leitor precisa guardar a condição na cabeça até o fim. Inverta a condição, saia primeiro, e o caminho principal fica sem recuo. Conta quando o corpo do if tem 3 instruções ou mais, ou outro bloco de controle dentro.",
        errado: "fun salvar(u: Usuario?) {\n    if (u != null) {\n        validar(u)\n        repo.save(u)\n        avisar(u)\n    }\n}",
        certo: "fun salvar(u: Usuario?) {\n    if (u == null) return\n    validar(u)\n    repo.save(u)\n    avisar(u)\n}",
    },
    Explicacao {
        codigo: "CND002",
        titulo: "else depois de return/throw (ESLint no-else-return, Pylint R1705)",
        porque: "Se o if termina em return, throw, break ou continue, o código do else só roda quando a condição é falsa mesmo sem o else. O else só acrescenta recuo. `else if` não conta, e if usado como valor (`val x = if ...`) também não.",
        errado: "if (saldo < valor) {\n    throw SaldoInsuficiente(conta)\n} else {\n    conta.debitar(valor)\n}",
        certo: "if (saldo < valor) throw SaldoInsuficiente(conta)\nconta.debitar(valor)",
    },
    Explicacao {
        codigo: "CND003",
        titulo: "Condição que mistura && e || (Fowler, Decompose Conditional)",
        porque: "Uma condição com && e || juntos obriga o leitor a resolver a precedência e não diz qual regra ela representa. Dê um nome à regra: uma função pequena (ou val) que devolve o booleano. Acima do limite de operandos, quem cobra é a CPX004.",
        errado: "if (cliente.vip && pedido.total > 100 || cupom != null) aplicarDesconto()",
        certo: "if (temDesconto(cliente, pedido, cupom)) aplicarDesconto()\n\nprivate fun temDesconto(c: Cliente, p: Pedido, cupom: Cupom?) =\n    (c.vip && p.total > 100) || cupom != null",
    },
    Explicacao {
        codigo: "POL001",
        titulo: "Cadeia de instanceof/is (Fowler, Replace Conditional with Polymorphism)",
        porque: "if / else if testando o tipo da mesma variável em 3 ramos ou mais: cada tipo novo exige achar e mudar a cadeia. O comportamento pertence ao tipo. Em Kotlin, `when` com `is` sobre sealed class é idiomático (o compilador cobra os casos) e não conta.",
        errado: "if (f instanceof Circulo c) return PI * c.r() * c.r();\nelse if (f instanceof Quadrado q) return q.l() * q.l();\nelse if (f instanceof Triangulo t) return t.b() * t.h() / 2;",
        certo: "interface Forma { double area(); }\nrecord Circulo(double r) implements Forma { public double area() { return PI * r * r; } }\n// ...\nreturn forma.area();",
    },
    Explicacao {
        codigo: "POL002",
        titulo: "switch/when repetido (Fowler, smell Repeated Switches)",
        porque: "O mesmo valor (ex.: tipo) decidido por switch/when com 3 casos constantes ou mais em dois lugares do arquivo. Cada caso novo exige mudar todos os switches. Troque por polimorfismo: enum com método abstrato, sealed class ou uma estratégia por tipo. when com `is` (sealed) não conta.",
        errado: "fun prazo() = when (tipo) { PAC -> 7; SEDEX -> 2; RETIRA -> 0 }\nfun preco() = when (tipo) { PAC -> 20.0; SEDEX -> 45.0; RETIRA -> 0.0 }",
        certo: "enum class Frete(val prazo: Int, val preco: Double) { PAC(7, 20.0), SEDEX(2, 45.0), RETIRA(0, 0.0) }",
    },
    Explicacao {
        codigo: "NEG001",
        titulo: "Exceção HTTP na camada de negócio (Clean Architecture, hexagonal)",
        porque: "Regra de negócio violada é um fato do domínio: lance uma exceção de domínio (ex.: SaldoInsuficiente) e deixe a camada de entrada traduzi-la em status HTTP (@ControllerAdvice, StatusPages). ResponseStatusException, HttpClientErrorException, WebApplicationException e as exceções HTTP do Ktor e do JAX-RS dentro de domínio, aplicação ou serviço prendem a regra ao protocolo. O tipo é resolvido pelo import (ou pelo nome, para os exclusivos do Spring).",
        errado: "// em application/service\nrepo.findById(id).orElseThrow(() -> new ResponseStatusException(NOT_FOUND))",
        certo: "// em application/service\nrepo.findById(id).orElseThrow(() -> new UsuarioNaoEncontrado(id))\n// em adapter/in/rest\n@ExceptionHandler(UsuarioNaoEncontrado.class) ResponseEntity<?> naoEncontrado(...)",
    },
    Explicacao {
        codigo: "NEG002",
        titulo: "Regra de negócio com exceção genérica",
        porque: "Na camada de negócio (domínio, aplicação, serviço), `throw IllegalArgumentException(\"saldo insuficiente\")` esconde a regra: o chamador não consegue tratar o caso nem a camada de entrada devolver 422 em vez de 500. Crie uma exceção com o nome da regra. `require`/`check` do Kotlin (pré-condição de programação) não contam.",
        errado: "if (valor > saldo) throw IllegalStateException(\"saldo insuficiente\")",
        certo: "class SaldoInsuficiente(val conta: ContaId, val falta: Dinheiro) : RegraDeNegocioException(\"saldo insuficiente\")\n\nif (valor > saldo) throw SaldoInsuficiente(id, valor - saldo)",
    },
    Explicacao {
        codigo: "SML007",
        titulo: "Código comentado (Sonar S125)",
        porque: "Código comentado fica velho e confunde. O histórico do git já guarda o que foi removido.",
        errado: "// if (usuario.ativo) { enviar(usuario); }",
        certo: "(apague; se for preciso, recupere pelo git)",
    },
    Explicacao {
        codigo: "SML008",
        titulo: "Membro privado não usado (PMD UnusedPrivateField/Method)",
        porque: "Campo ou método privado que nada usa é código morto. Classes com Lombok ou JPA e membros anotados ficam de fora (usados por reflexão).",
        errado: "private final Logger antigo = ...; // nunca usado",
        certo: "(apague)",
    },
    Explicacao {
        codigo: "SML009",
        titulo: "TODO/FIXME sem ticket (detekt ForbiddenComment)",
        porque: "TODO sem referência vira dívida invisível. Aponte o ticket (ABC-123 ou #123).",
        errado: "// TODO arrumar depois",
        certo: "// TODO ABC-123 trocar pelo cálculo novo",
    },
    Explicacao {
        codigo: "KT002",
        titulo: "GlobalScope.launch/async (detekt GlobalCoroutineUsage)",
        porque: "Corrotina no GlobalScope não é cancelada com a tela ou o pedido que a criou: vaza e esconde erro.",
        errado: "GlobalScope.launch { sincronizar() }",
        certo: "viewModelScope.launch { sincronizar() } // ou um CoroutineScope injetado",
    },
    Explicacao {
        codigo: "KT003",
        titulo: "Thread.sleep em corrotina (detekt SleepInsteadOfDelay)",
        porque: "Thread.sleep trava a thread inteira, inclusive as outras corrotinas dela. Em corrotina, use delay.",
        errado: "suspend fun esperar() { Thread.sleep(1000) }",
        certo: "suspend fun esperar() { delay(1000) }",
    },
    Explicacao {
        codigo: "KT004",
        titulo: "suspend fun que retorna Flow (detekt SuspendFunWithFlowReturnType)",
        porque: "Flow já é frio e assíncrono; suspend na função que o cria é redundante e confunde quem chama.",
        errado: "suspend fun eventos(): Flow<Evento>",
        certo: "fun eventos(): Flow<Evento>",
    },
    Explicacao {
        codigo: "KT005",
        titulo: "Dispatchers fixo em vez de injetado (detekt InjectDispatcher)",
        porque: "Dispatchers.IO escrito no meio do código impede o teste de controlar a execução. Receba o dispatcher (valor padrão em parâmetro é aceito).",
        errado: "withContext(Dispatchers.IO) { ... }",
        certo: "class Repo(private val io: CoroutineDispatcher = Dispatchers.IO) { withContext(io) { ... } }",
    },
    Explicacao {
        codigo: "KT006",
        titulo: "data class com var (detekt DataClassShouldBeImmutable)",
        porque: "data class é valor: com var, o hashCode muda depois de entrar num Set ou Map.",
        errado: "data class Pedido(var status: Status)",
        certo: "data class Pedido(val status: Status) // e copy(status = ...)",
    },
    Explicacao {
        codigo: "KT007",
        titulo: "println/printStackTrace em produção (detekt ForbiddenMethodCall)",
        porque: "Saída padrão não tem nível, contexto nem destino; em produção use o logger.",
        errado: "println(\"erro: $e\")\ne.printStackTrace()",
        certo: "log.error(\"falhou ao salvar {}\", id, e)",
    },
    Explicacao {
        codigo: "JV001",
        titulo: "System.out/printStackTrace em produção (PMD SystemPrintln)",
        porque: "Saída padrão não tem nível, contexto nem destino; em produção use o logger.",
        errado: "System.out.println(\"erro\");\ne.printStackTrace();",
        certo: "log.error(\"falhou ao salvar {}\", id, e);",
    },
    Explicacao {
        codigo: "JV002",
        titulo: "Recurso sem try-with-resources (PMD UseTryWithResources)",
        porque: "Stream, reader e conexão abertos sem try-with-resources não fecham quando dá exceção.",
        errado: "BufferedReader r = new BufferedReader(new FileReader(f));",
        certo: "try (BufferedReader r = new BufferedReader(new FileReader(f))) { ... }",
    },
    Explicacao {
        codigo: "JV003",
        titulo: "equals sem hashCode (PMD OverrideBothEqualsAndHashcode)",
        porque: "Objetos iguais pelo equals precisam ter o mesmo hashCode, senão somem em HashMap e HashSet.",
        errado: "@Override public boolean equals(Object o) { ... } // sem hashCode",
        certo: "@Override public boolean equals(Object o) { ... }\n@Override public int hashCode() { return Objects.hash(id); }",
    },
    Explicacao {
        codigo: "JV004",
        titulo: "Optional como campo ou parâmetro (Sonar S3553)",
        porque: "Optional foi feito para retorno. Como parâmetro ou campo, ele mesmo pode ser nulo e complica a chamada.",
        errado: "void buscar(Optional<String> filtro)",
        certo: "void buscar(String filtro) // ou duas funções\nOptional<Usuario> buscar(long id)",
    },
    Explicacao {
        codigo: "JV005",
        titulo: "java.util.Date/Calendar",
        porque: "Date e Calendar são mutáveis e confusos com fuso horário. java.time resolve isso.",
        errado: "import java.util.Date;\nDate agora = new Date();",
        certo: "import java.time.Instant;\nInstant agora = Instant.now();",
    },
    Explicacao {
        codigo: "ERR000",
        titulo: "Arquivo que o parser não leu",
        porque: "Um arquivo com erro de sintaxe não pode ser verificado. Em vez de pulá-lo em silêncio, a guarda reporta. Se o código compila, é limite da gramática do tree-sitter: registre o caso.",
        errado: "void m( { }",
        certo: "void m() { }",
    },
    Explicacao {
        codigo: "ARQ001",
        titulo: "Dependência proibida entre camadas",
        porque: "Cada camada só pode importar as camadas permitidas pela arquitetura (veja `guarda contexto`). Importar uma camada de fora (ex.: domínio importando infraestrutura) acopla a regra de negócio ao detalhe técnico.",
        errado: "// em domain/\nimport com.app.infrastructure.entity.UsuarioEntity;",
        certo: "// em domain/: uma porta (interface)\ninterface UsuarioRepositorio { Usuario buscar(Long id); }\n// em infrastructure/: a implementação, que converte UsuarioEntity em Usuario",
    },
    Explicacao {
        codigo: "CPX001",
        titulo: "Complexidade cognitiva (Sonar S3776)",
        porque: "Mede o esforço para entender a função: cada if, laço, catch, switch e sequência de && ou || soma, e soma mais quanto mais aninhado. Acima do limite, a função é difícil de ler e de testar.",
        errado: "for (...) {\n  if (a) {\n    if (b) { ... } else if (c) { ... }\n  }\n}",
        certo: "for (...) { tratar(item); }\n\nvoid tratar(Item i) {\n  if (!a) return;\n  if (b) { ... return; }\n  if (c) { ... }\n}",
    },
    Explicacao {
        codigo: "CPX005",
        titulo: "Função longa (detekt LongMethod)",
        porque: "Linhas de código da função, sem brancos e comentários. Função longa costuma fazer várias coisas; dividir em passos com nome deixa cada um testável.",
        errado: "fun processar() { /* 80 linhas: valida, calcula, grava, notifica */ }",
        certo: "fun processar() { validar(); val r = calcular(); gravar(r); notificar(r) }",
    },
    Explicacao {
        codigo: "KT001",
        titulo: "Uso de !! (detekt UnsafeCallOnNullableType)",
        porque: "`!!` transforma nulo em NullPointerException em tempo de execução, sem mensagem útil.",
        errado: "val nome = usuario!!.nome",
        certo: "val nome = usuario?.nome ?: throw UsuarioNaoEncontrado(id)\n// ou: requireNotNull(usuario) { \"usuário $id não encontrado\" }",
    },
    Explicacao {
        codigo: "SUP001",
        titulo: "Supressão sem motivo",
        porque: "Suprimir um aviso sem dizer por quê esconde dívida. Toda supressão precisa de motivo, para a revisão do PR julgar.",
        errado: "@SuppressWarnings(\"unchecked\")\n// guarda:ignorar KT001",
        certo: "@SuppressWarnings(\"unchecked\") // API legada devolve List cru\n// guarda:ignorar KT001 motivo: o framework garante não nulo aqui",
    },
    Explicacao {
        codigo: "GRD001",
        titulo: "guarda.yml ou baseline alterado",
        porque: "Mudar a configuração ou o baseline afrouxa a regra para todo mundo. É aviso: a mudança é permitida, mas pede revisão humana no PR.",
        errado: "(a IA editando guarda.yml para passar)",
        certo: "(um humano decide e explica a mudança no PR)",
    },
    Explicacao {
        codigo: "COV000",
        titulo: "Os testes falharam ao gerar a cobertura",
        porque: "`--cobertura` roda os testes para gerar o relatório. Com teste falhando não há cobertura confiável, e a tarefa não está pronta.",
        errado: "(terminar a tarefa com teste vermelho)",
        certo: "(corrigir o teste ou o código até a suíte passar)",
    },
    Explicacao {
        codigo: "COV001",
        titulo: "Cobertura do código novo (Sonar way: ≥ 80%)",
        porque: "Conta só as linhas novas ou alteradas em relação à referência (ex.: origin/main), como o Sonar. Linha executável não executada por teste, ou desvio (if, &&) com um dos caminhos sem teste, baixa a cobertura.",
        errado: "fun aliquota(n: Nota) = if (n.isenta) 0.0 else 0.18  // teste só cobre o caso comum",
        certo: "@Test fun `nota isenta tem aliquota zero`() { assertEquals(0.0, aliquota(notaIsenta)) }",
    },
    Explicacao {
        codigo: "COV002",
        titulo: "Cobertura mínima por pacote",
        porque: "Pacotes críticos (ex.: cálculo) podem exigir mais cobertura que o resto. A medida é do pacote inteiro, no último relatório.",
        errado: "(cobertura de ..calculo.. abaixo do mínimo configurado)",
        certo: "(testes para os ramos sem cobertura do pacote)",
    },
    Explicacao {
        codigo: "COV003",
        titulo: "Cobertura geral mínima",
        porque: "Piso de cobertura do projeto inteiro. Funciona como catraca: o baseline guarda o valor atual e ele só pode subir.",
        errado: "(remover testes ou acrescentar código sem teste)",
        certo: "(cada mudança deixa a cobertura igual ou maior)",
    },
    Explicacao {
        codigo: "SML002",
        titulo: "Código duplicado (PMD CPD, Sonar)",
        porque: "Blocos de pelo menos `duplicacao.tokens-minimos` tokens iguais em dois lugares. Corrigir um bug em uma cópia e esquecer a outra é o risco. No código novo, o gate do Sonar aceita no máximo 3% de linhas duplicadas.",
        errado: "(o mesmo mapeamento de DTO copiado em dois serviços)",
        certo: "(uma função ou classe compartilhada, chamada dos dois lugares)",
    },
];

pub fn explicar(codigo: &str) -> Option<String> {
    let c = codigo.trim().to_uppercase();
    let e = EXPLICACOES.iter().find(|e| e.codigo == c)?;
    Some(format!(
        "{} {}{}\n\nPor quê: {}\n\nErrado:\n{}\n\nCerto:\n{}\n\nSupressão pontual (com motivo): // guarda:ignorar {} motivo: ...\n",
        e.codigo,
        e.titulo,
        severidades(e.codigo),
        e.porque,
        recuar(e.errado),
        recuar(e.certo),
        e.codigo
    ))
}

fn severidades(codigo: &str) -> String {
    match crate::modelo::regra(codigo) {
        Some(r) if r.severidade != r.alvo => {
            " (hoje aviso; vira erro depois de medida em projeto real)".into()
        }
        Some(r) if r.severidade == crate::modelo::Severidade::Aviso => " (aviso)".into(),
        _ => String::new(),
    }
}

pub fn lista() -> String {
    EXPLICACOES
        .iter()
        .map(|e| format!("{} {}\n", e.codigo, e.titulo))
        .collect()
}

fn recuar(t: &str) -> String {
    t.lines()
        .map(|l| format!("    {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn toda_regra_tem_explicacao() {
        for r in crate::modelo::REGRAS {
            assert!(explicar(r.codigo).is_some(), "{} sem explicação", r.codigo);
        }
        assert!(explicar("kt001").unwrap().starts_with("KT001 "));
        assert!(explicar("XYZ999").is_none());
    }
}
