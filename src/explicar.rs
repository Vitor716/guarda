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
        codigo: "TST004",
        titulo: "Asserção sempre verdadeira em teste (Sonar S2701, S5863)",
        porque: "`assertTrue(true)`, `assertFalse(false)`, `assertEquals(x, x)` ou `assertNotNull(\"x\")` nunca falham e criam falsa sensação de cobertura.",
        errado: "@Test void valida() { servico.executar(); assertTrue(true); }",
        certo: "@Test void valida() { assertEquals(Status.OK, servico.executar()); }",
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
        codigo: "SEC001",
        titulo: "Credencial fixa (Sonar S2068, S6418, S6437; Cloudflare security-audit)",
        porque: "Senha, token ou chave escritos no código ou no application.properties vão para o git e ficam no histórico para sempre. Conta pelo nome (password, senha, secret, token, apiKey...) com valor literal, e por formato conhecido (chave da AWS, chave privada, token do GitHub/Slack/Stripe/Google, JWT). `${VAR:padrao}` com padrão fixo também conta. A mensagem nunca repete o valor.",
        errado: "private static final String JWT_SECRET = \"minha-chave\";\nspring.datasource.password=root123",
        certo: "@Value(\"${JWT_SECRET}\") private String jwtSecret;\nspring.datasource.password=${DB_PASSWORD}",
    },
    Explicacao {
        codigo: "SEC002",
        titulo: "SQL montado com concatenação (Sonar S2077, FindSecBugs SQL_INJECTION, CWE-89)",
        porque: "Texto de SQL concatenado com variável (ou template do Kotlin, String.format) e passado a executeQuery, createQuery, jdbcTemplate.query... abre injeção de SQL. Também conta a variável local montada assim e passada depois. Parâmetro ligado (? ou :nome) não conta.",
        errado: "st.executeQuery(\"SELECT * FROM u WHERE nome = '\" + nome + \"'\");",
        certo: "jdbc.query(\"SELECT * FROM u WHERE nome = ?\", mapper, nome);",
    },
    Explicacao {
        codigo: "SEC003",
        titulo: "Comando do sistema montado (Sonar S2076/S4721, FindSecBugs COMMAND_INJECTION, CWE-78)",
        porque: "Runtime.exec e ProcessBuilder com argumento que não é literal: se o valor vier de fora, vira execução de comando.",
        errado: "Runtime.getRuntime().exec(\"ping \" + host);",
        certo: "if (!HOSTS_PERMITIDOS.contains(host)) throw ...;\nnew ProcessBuilder(\"ping\", \"-c\", \"1\", host).start();",
    },
    Explicacao {
        codigo: "SEC004",
        titulo: "Hash fraco (Sonar S4790, S5344; CWE-328, CWE-916)",
        porque: "MD5 e SHA-1 têm colisão prática; para senha, qualquer hash rápido (e NoOpPasswordEncoder, Md5PasswordEncoder...) permite força bruta. Senha pede hash lento com sal.",
        errado: "MessageDigest.getInstance(\"MD5\")\nNoOpPasswordEncoder.getInstance()",
        certo: "MessageDigest.getInstance(\"SHA-256\")\nnew BCryptPasswordEncoder()",
    },
    Explicacao {
        codigo: "SEC005",
        titulo: "Cifra fraca ou modo inseguro (Sonar S5547, S5542; CWE-327)",
        porque: "DES, 3DES, RC2, RC4 e Blowfish são fracos; o modo ECB (padrão quando se pede só \"AES\") vaza padrões do texto. Use AES/GCM com IV aleatório.",
        errado: "Cipher.getInstance(\"AES\")\nCipher.getInstance(\"DES/ECB/PKCS5Padding\")",
        certo: "Cipher.getInstance(\"AES/GCM/NoPadding\")",
    },
    Explicacao {
        codigo: "SEC006",
        titulo: "Aleatório previsível em contexto de segredo (Sonar S2245; Cloudflare: weak randomness)",
        porque: "Random, Math.random, ThreadLocalRandom e RandomStringUtils são previsíveis. Só conta quando o valor vira segredo: função ou variável com nome de token, senha, otp, nonce, salt, sessão, código de verificação.",
        errado: "String gerarToken() { return Integer.toString(new Random().nextInt()); }",
        certo: "private static final SecureRandom RNG = new SecureRandom();",
    },
    Explicacao {
        codigo: "SEC007",
        titulo: "TLS sem verificação (Sonar S4830, S5527, S4423; CWE-295)",
        porque: "TrustManager com checkServerTrusted vazio, HostnameVerifier que devolve true, NoopHostnameVerifier, InsecureTrustManagerFactory ou SSLContext com SSL/TLSv1/TLSv1.1: qualquer um no meio do caminho lê e altera o tráfego.",
        errado: "conn.setHostnameVerifier((h, s) -> true);",
        certo: "(use o verificador padrão; para teste, um truststore com o certificado)",
    },
    Explicacao {
        codigo: "SEC008",
        titulo: "CORS aberto (Sonar S5122; CWE-942)",
        porque: "@CrossOrigin sem origens (o padrão é qualquer uma), allowedOrigins(\"*\") ou o cabeçalho Access-Control-Allow-Origin: * deixam qualquer site chamar a API pelo navegador do usuário.",
        errado: "@CrossOrigin\n@RestController class PedidoController",
        certo: "@CrossOrigin(origins = [\"https://app.exemplo.com\"])",
    },
    Explicacao {
        codigo: "SEC009",
        titulo: "Desserialização insegura (Sonar S4544, FindSecBugs OBJECT_DESERIALIZATION; CWE-502)",
        porque: "ObjectInputStream (sem resolveClass que filtre classes), XMLDecoder, XStream sem allowTypes, Jackson com default typing ou @JsonTypeInfo(use = Id.CLASS): o dado escolhe a classe instanciada, e isso vira execução de código.",
        errado: "new ObjectInputStream(request.getInputStream()).readObject();\nmapper.activateDefaultTyping(...)",
        certo: "mapper.readValue(json, PedidoDto.class);  // tipo fechado",
    },
    Explicacao {
        codigo: "SEC010",
        titulo: "XML sem proteção contra XXE (Sonar S2755, FindSecBugs XXE; CWE-611)",
        porque: "DocumentBuilderFactory, SAXParserFactory, XMLInputFactory, TransformerFactory, SAXBuilder e SAXReader resolvem entidades externas por padrão: um XML malicioso lê arquivos do servidor. Conta quando a função não chama setFeature/setProperty/setAttribute.",
        errado: "DocumentBuilderFactory.newInstance().newDocumentBuilder().parse(in);",
        certo: "var f = DocumentBuilderFactory.newInstance();\nf.setFeature(\"http://apache.org/xml/features/disallow-doctype-decl\", true);",
    },
    Explicacao {
        codigo: "SEC011",
        titulo: "CSRF desligado (Sonar S4502; CWE-352)",
        porque: "Com sessão em cookie, desligar o CSRF deixa outro site disparar ações em nome do usuário. API sem sessão (token no cabeçalho) não precisa: por isso a regra não conta quando o arquivo declara SessionCreationPolicy.STATELESS.",
        errado: "http.csrf().disable().formLogin();",
        certo: "http.csrf(Customizer.withDefaults());\n// ou, API com token: .sessionManagement(s -> s.sessionCreationPolicy(STATELESS))",
    },
    Explicacao {
        codigo: "SEC012",
        titulo: "Rota liberada ampla ou sensível (Cloudflare security-audit, \"obvious things\")",
        porque: "anyRequest().permitAll(), \"/**\" liberado, ignoring() e liberação de actuator, h2-console, /admin, /debug ou heapdump tiram a autenticação de tudo ou do que não devia ser público.",
        errado: ".requestMatchers(\"/h2-console/**\").permitAll()\n.anyRequest().permitAll()",
        certo: ".requestMatchers(\"/public/**\").permitAll()\n.anyRequest().authenticated()",
    },
    Explicacao {
        codigo: "SEC013",
        titulo: "Cookie sem Secure/HttpOnly (Sonar S2092, S3330; CWE-614, CWE-1004)",
        porque: "Sem HttpOnly, um script injetado lê o cookie; sem Secure, ele viaja em HTTP aberto.",
        errado: "Cookie c = new Cookie(\"sessao\", id); resp.addCookie(c);",
        certo: "c.setHttpOnly(true); c.setSecure(true);\n// ou ResponseCookie.from(...).httpOnly(true).secure(true)",
    },
    Explicacao {
        codigo: "SEC014",
        titulo: "JWT sem verificação (Sonar S5659; CWE-347)",
        porque: "JWT.decode (auth0) só decodifica: qualquer um forja o token. Conta quando a função não chama .verify(; também parseClaimsJwt/parseUnsecured* do jjwt e Algorithm.none().",
        errado: "String usuario = JWT.decode(token).getSubject();",
        certo: "String usuario = JWT.require(Algorithm.HMAC256(segredo)).build().verify(token).getSubject();",
    },
    Explicacao {
        codigo: "SEC015",
        titulo: "Dado sensível no log (CWE-532; Cloudflare: secrets in logs)",
        porque: "Log com variável de senha, token, segredo ou credencial espalha o segredo para o agregador de logs, onde muita gente lê.",
        errado: "log.info(\"login de {} com senha {}\", email, senha);",
        certo: "log.info(\"login de {}\", email);",
    },
    Explicacao {
        codigo: "SEC016",
        titulo: "TODO de segurança (Cloudflare security-audit, \"obvious things\")",
        porque: "Comentário TODO/FIXME/HACK sobre autenticação, permissão, senha, token, validação, CSRF, SQL, criptografia: é dívida de segurança conhecida e esquecida.",
        errado: "// TODO validar a permissão do usuário",
        certo: "(implemente a verificação, ou abra um ticket e cite: // TODO SEC-12 ...)",
    },
    Explicacao {
        codigo: "SEC017",
        titulo: "Configuração insegura (Sonar S4507; Cloudflare: debug gated)",
        porque: "No application.properties/yml principal: todos os endpoints do actuator expostos, console do H2 aberto para a rede, debug=true, stack trace na resposta, actuator mostrando valores. Perfis -dev, -local e -test ficam de fora.",
        errado: "management.endpoints.web.exposure.include=*",
        certo: "management.endpoints.web.exposure.include=health,info",
    },
    Explicacao {
        codigo: "SEC018",
        titulo: "Arquivo de segredo versionável (Cloudflare security-audit, \"obvious things\")",
        porque: ".env, .pem, .key, .p12, .jks, id_rsa, credentials.json que o .gitignore não cobre acabam no git. Arquivos de exemplo (.env.example) e de src/test ficam de fora.",
        errado: "(.env na raiz do projeto, sem regra no .gitignore)",
        certo: "echo .env >> .gitignore && git rm --cached .env",
    },
    Explicacao {
        codigo: "SEC019",
        titulo: "IP fixo (Sonar S1313)",
        porque: "Endereço IP no código amarra o build a um ambiente e expõe a rede interna. Loopback, 0.0.0.0, faixas de documentação e textos com cara de versão (1.2.3.4) não contam.",
        errado: "val host = \"10.0.0.15:8080\"",
        certo: "@Value(\"${servico.host}\") lateinit var host: String",
    },
    Explicacao {
        codigo: "SEC020",
        titulo: "Entrada do request em sorvedouro (Sonar S2083, S5144, S5146, S3649; Cloudflare: trace untrusted input to sink)",
        porque: "Parâmetro @RequestParam/@PathVariable/@RequestBody... (ou request.getParameter) que chega, na mesma função, a caminho de arquivo, URL buscada pelo servidor (SSRF), redirecionamento, comando, SQL montado, Class.forName ou SpEL. A guarda segue variáveis locais; validação no meio não é reconhecida, então trate como ponto a revisar.",
        errado: "byte[] baixar(@RequestParam String nome) { return Files.readAllBytes(Paths.get(\"/dados/\" + nome)); }",
        certo: "Path p = BASE.resolve(nome).normalize();\nif (!p.startsWith(BASE)) throw new AcessoNegado();",
    },
    Explicacao {
        codigo: "SPR001",
        titulo: "Injeção por campo (Sonar S6813)",
        porque: "@Autowired/@Inject em campo esconde as dependências, impede campo final e obriga reflexão no teste. Pelo construtor, a classe declara o que precisa.",
        errado: "@Autowired private PedidoRepository repo;",
        certo: "private final PedidoRepository repo;\nPedidoService(PedidoRepository repo) { this.repo = repo; }",
    },
    Explicacao {
        codigo: "SPR002",
        titulo: "@Transactional/@Async em método private (Sonar S2230)",
        porque: "O proxy do Spring só intercepta método público chamado de fora: em método private, @Transactional, @Async, @Cacheable, @PreAuthorize e afins não fazem nada.",
        errado: "@Transactional private void salvarTudo() { ... }",
        certo: "@Transactional public void salvarTudo() { ... }  // chamado de outro bean",
    },
    Explicacao {
        codigo: "SPR003",
        titulo: "Chamada pelo this a @Transactional/@Async (Sonar S6809)",
        porque: "Chamar, de dentro da própria classe, um método com @Transactional, @Async, @Cacheable... pula o proxy: a anotação é ignorada. Não conta quando quem chama já tem a mesma anotação.",
        errado: "public void fechar() { salvar(); }  // salvar() é @Transactional",
        certo: "(anote fechar() ou mova salvar() para outro bean)",
    },
    Explicacao {
        codigo: "SPR004",
        titulo: "Entidade no @RequestBody (Sonar S4684; CWE-915)",
        porque: "Receber a entidade @Entity direto do JSON deixa o cliente preencher qualquer campo (id, perfil, saldo): é atribuição em massa. A regra cruza as classes @Entity do projeto todo com os parâmetros @RequestBody.",
        errado: "@PostMapping Usuario criar(@RequestBody Usuario u)",
        certo: "@PostMapping UsuarioDto criar(@RequestBody NovoUsuario dados)",
    },
    Explicacao {
        codigo: "SPR005",
        titulo: "@RequestMapping sem método (Sonar S3752; CWE-352)",
        porque: "Sem method, o endpoint aceita GET, POST, DELETE...: uma ação de escrita passa a responder a GET, que não leva proteção CSRF.",
        errado: "@RequestMapping(\"/excluir\") void excluir(...)",
        certo: "@DeleteMapping(\"/excluir\") void excluir(...)",
    },
    Explicacao {
        codigo: "BUG001",
        titulo: "Desvio dentro de finally (Sonar S1143, detekt ReturnFromFinally, PMD ReturnFromFinallyBlock)",
        porque: "return, throw, break ou continue no finally descartam a exceção do try sem aviso.",
        errado: "try { salvar(); } finally { return true; }",
        certo: "try { salvar(); } finally { fechar(); }\nreturn true;",
    },
    Explicacao {
        codigo: "BUG002",
        titulo: "Expressões idênticas (Sonar S1764)",
        porque: "a == a, x && x, a - a: o resultado é fixo, quase sempre é cópia errada. Lados com chamada de método não contam.",
        errado: "if (inicio.isBefore(fim) && inicio.isBefore(fim))",
        certo: "if (inicio.isBefore(fim) && fim.isBefore(limite))",
    },
    Explicacao {
        codigo: "BUG003",
        titulo: "Auto-atribuição (Sonar S1656)",
        porque: "x = x não faz nada; normalmente faltou o this. no setter ou construtor.",
        errado: "void setNome(String nome) { nome = nome; }",
        certo: "void setNome(String nome) { this.nome = nome; }",
    },
    Explicacao {
        codigo: "BUG004",
        titulo: "Condição repetida (Sonar S1862)",
        porque: "A segunda ocorrência da mesma condição num if/else if ou when nunca roda: é cópia errada.",
        errado: "if (x > 0) a(); else if (y > 0) b(); else if (x > 0) c();",
        certo: "if (x > 0) a(); else if (y > 0) b(); else if (z > 0) c();",
    },
    Explicacao {
        codigo: "BUG005",
        titulo: "Ramos idênticos (Sonar S1871, S3923)",
        porque: "Dois ramos de if/when com o mesmo código (de 2 instruções ou mais), ou todos os ramos iguais: ou a condição não serve, ou um ramo foi copiado e não ajustado.",
        errado: "if (vip) { desconto(10); avisar(); } else { desconto(10); avisar(); }",
        certo: "if (vip) { desconto(10); avisar(); } else { desconto(5); }",
    },
    Explicacao {
        codigo: "BUG006",
        titulo: "String com == (Sonar S4973)",
        porque: "Em Java, == compara a referência: duas Strings com o mesmo texto podem dar false. (No Kotlin, == já é equals.)",
        errado: "if (status == \"ATIVO\")",
        certo: "if (\"ATIVO\".equals(status))",
    },
    Explicacao {
        codigo: "BUG007",
        titulo: "BigDecimal de double (Sonar S2111, PMD AvoidDecimalLiteralsInBigDecimalConstructor)",
        porque: "new BigDecimal(0.1) guarda 0.1000000000000000055511151231257827...: o double já perdeu precisão antes. Em dinheiro, isso aparece no centavo.",
        errado: "new BigDecimal(0.1)",
        certo: "new BigDecimal(\"0.1\")  // ou BigDecimal.valueOf(0.1)",
    },
    Explicacao {
        codigo: "BUG008",
        titulo: "InterruptedException ignorada (Sonar S2142)",
        porque: "Pegar InterruptedException e seguir em frente apaga o pedido de parada da thread: o shutdown trava. Restaure o sinal ou relance.",
        errado: "try { Thread.sleep(100); } catch (InterruptedException e) { log.warn(\"x\"); }",
        certo: "catch (InterruptedException e) { Thread.currentThread().interrupt(); return; }",
    },
    Explicacao {
        codigo: "SML010",
        titulo: "Bloco vazio (Sonar S108, S1186; detekt empty-blocks)",
        porque: "if, laço, try, finally ou função com o corpo vazio: código esquecido ou intenção sem registro. Funções anotadas (@Override, @Pointcut...), override/open, default de interface e catch (que é da SML005) não contam; um comentário dentro do bloco basta.",
        errado: "if (pedido.cancelado()) { }",
        certo: "if (pedido.cancelado()) { return; }  // ou um comentário dizendo por que fica vazio",
    },
    Explicacao {
        codigo: "SML011",
        titulo: "Relançar sem a causa (PMD PreserveStackTrace)",
        porque: "Lançar uma exceção nova dentro do catch sem passar a original perde a pilha e a causa raiz.",
        errado: "catch (IOException e) { throw new ArquivoInvalido(\"não leu\"); }",
        certo: "catch (IOException e) { throw new ArquivoInvalido(\"não leu\", e); }",
    },
    Explicacao {
        codigo: "CND004",
        titulo: "if aninhado que pode juntar (Sonar S1066, PMD CollapsibleIfStatements)",
        porque: "if sem else cujo corpo é só outro if sem else: um nível de recuo a mais sem motivo.",
        errado: "if (pedido != null) {\n    if (pedido.pago) enviar(pedido)\n}",
        certo: "if (pedido != null && pedido.pago) enviar(pedido)",
    },
    Explicacao {
        codigo: "KT008",
        titulo: "Estado mutável exposto (Sonar S6305)",
        porque: "Propriedade pública do tipo MutableStateFlow, MutableSharedFlow ou MutableLiveData deixa qualquer um mudar o estado da classe (ex.: a tela mudando o ViewModel).",
        errado: "val estado = MutableStateFlow(Estado())",
        certo: "private val _estado = MutableStateFlow(Estado())\nval estado: StateFlow<Estado> = _estado.asStateFlow()",
    },
    Explicacao {
        codigo: "JV006",
        titulo: "Literal à direita no equals (PMD LiteralsFirstInComparisons)",
        porque: "texto.equals(\"x\") quebra com NullPointerException quando texto é null; \"x\".equals(texto) devolve false.",
        errado: "if (status.equals(\"ATIVO\"))",
        certo: "if (\"ATIVO\".equals(status))",
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
    Explicacao {
        codigo: "SPR006",
        titulo: "@Transactional em controlador HTTP (Spring, arquitetura em camadas)",
        porque: "Abrir transação de banco no @RestController/@Controller segura a conexão do pool durante a serialização HTTP e mistura regra de transação com a camada web.",
        errado: "@RestController class PedidoController {\n    @PostMapping @Transactional ResponseEntity<?> criar(...) { ... }\n}",
        certo: "@Service class PedidoService {\n    @Transactional Pedido criar(...) { ... }\n}",
    },
    Explicacao {
        codigo: "BUG009",
        titulo: "assert em código de produção Java (Sonar S4274)",
        porque: "Na JVM, `assert` vem desligado por padrão (só executa com `-ea`). Usar `assert` para checar nulidade ou invariante em produção faz a checagem desaparecer em runtime.",
        errado: "assert retorno != null;\nreturn retorno.mensagem();",
        certo: "if (retorno == null) throw new IllegalStateException(\"retorno ausente\");",
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
