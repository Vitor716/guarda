//! Segurança: pontos que o Sonar way (Java e Kotlin), o FindSecBugs e a skill
//! security-audit da Cloudflare ("obvious things", injeção, cripto e segredos) mandam olhar.
//!
//! São pontos de atenção, não vulnerabilidades confirmadas: a guarda vê a sintaxe, não o
//! deploy (a skill da Cloudflare chama de antipadrão tratar desvio de checklist como falha).
//! Por isso cada regra exige um sorvedouro concreto (a chamada perigosa) no próprio código.

use super::Contexto;
use super::sintaxe::{
    Chamada, anotacoes, chamada, conteudo, formato_de_segredo, funcao_dona, identificadores,
    literal, montada, nome_da_funcao, nome_de_segredo, palavras, parece_sql, receptor_txt,
    sem_parenteses, valor_de_segredo,
};
use crate::lang::{
    self, eh_comentario, eh_funcao, eh_lambda, filhos, linha, simbolo_dono, texto, visitar,
};
use crate::modelo::Violacao;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use tree_sitter::Node;

/// Métodos que executam SQL (JDBC, JPA/Hibernate, JdbcTemplate, JdbcClient).
const SQL_SORVEDOUROS: &[&str] = &[
    "addBatch",
    "batchUpdate",
    "createNativeQuery",
    "createQuery",
    "createSQLQuery",
    "execute",
    "executeLargeUpdate",
    "executeQuery",
    "executeUpdate",
    "prepareCall",
    "prepareStatement",
    "query",
    "queryForList",
    "queryForMap",
    "queryForObject",
    "queryForRowSet",
    "queryForStream",
    "sql",
    "update",
];

/// Anotações de parâmetro que trazem dado do cliente (Spring MVC, JAX-RS).
const FONTES_ANOTADAS: &[&str] = &[
    "CookieValue",
    "FormParam",
    "HeaderParam",
    "MatrixVariable",
    "ModelAttribute",
    "PathParam",
    "PathVariable",
    "QueryParam",
    "RequestBody",
    "RequestHeader",
    "RequestParam",
    "RequestPart",
];

/// Chamadas que leem dado do cliente (Servlet, Ktor).
const FONTES_CHAMADAS: &[&str] = &[
    "getCookies",
    "getHeader",
    "getHeaders",
    "getParameter",
    "getParameterMap",
    "getParameterValues",
    "getQueryString",
    "receive",
    "receiveParameters",
    "receiveText",
];

static TODO_DE_SEGURANCA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(TODO|FIXME|HACK|XXX)\b.*\b(auth\w*|autentica\w*|autoriza\w*|seguran\w*|secur\w*|senha\w*|password\w*|permiss\w*|csrf|xss|sql|inje\w*|inject\w*|token\w*|criptogra\w*|encrypt\w*|sanitiz\w*|cors|ssl|tls|certificad\w*)").expect("regex")
});

static IPV4: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:[a-z][a-z0-9+.-]*://)?((?:\d{1,3}\.){3}\d{1,3})(?::\d{1,5})?(?:/.*)?$")
        .expect("regex")
});

struct Achados<'a> {
    ctx: &'a Contexto<'a>,
    v: &'a mut Vec<Violacao>,
    /// Linhas com SEC001 já reportada (nome e formato não duplicam).
    segredos: HashSet<usize>,
    /// Variáveis com SQL montado, por função (id do nó da função).
    sql_montado: HashMap<usize, HashSet<String>>,
}

impl Achados<'_> {
    fn add(
        &mut self,
        codigo: &'static str,
        n: Node,
        msg: impl Into<String>,
        dica: &str,
        trecho: &str,
    ) {
        self.v.push(Violacao::nova(
            codigo,
            self.ctx.arquivo,
            linha(n),
            msg,
            dica,
            simbolo_dono(n, self.ctx.fonte),
            trecho,
            None,
        ));
    }
}

pub fn verificar(ctx: &Contexto, v: &mut Vec<Violacao>) {
    let mut a = Achados {
        ctx,
        v,
        segredos: HashSet::new(),
        sql_montado: HashMap::new(),
    };
    let fonte = ctx.fonte;
    visitar(ctx.raiz, |n| {
        match n.kind() {
            "variable_declarator" => {
                if let (Some(nome), Some(valor)) = (
                    n.child_by_field_name("name"),
                    n.child_by_field_name("value"),
                ) {
                    atribuicao(&mut a, n, texto(nome, fonte), valor);
                }
            }
            "property_declaration" => {
                let nome = filhos(n)
                    .into_iter()
                    .find(|c| c.kind() == "variable_declaration")
                    .and_then(|d| d.named_child(0))
                    .map(|i| texto(i, fonte));
                let valor = n.named_child(n.named_child_count().saturating_sub(1) as u32);
                if let (Some(nome), Some(valor)) = (nome, valor) {
                    atribuicao(&mut a, n, nome, valor);
                }
            }
            "assignment_expression" | "assignment" => {
                if let (Some(esq), Some(dir)) = (
                    n.child_by_field_name("left"),
                    n.child_by_field_name("right"),
                ) {
                    let t = texto(esq, fonte);
                    let nome = t.rsplit('.').next().unwrap_or(t);
                    atribuicao(&mut a, n, nome, dir);
                }
            }
            "value_argument" => {
                // Argumento nomeado do Kotlin: `password = "..."`.
                let fs = filhos(n);
                if fs.len() >= 3 && fs[0].kind() == "identifier" && fs[1].kind() == "=" {
                    atribuicao(&mut a, n, texto(fs[0], fonte), fs[fs.len() - 1]);
                }
            }
            "string_literal" | "text_block" => string(&mut a, n),
            k if eh_comentario(k) => {
                let t = texto(n, fonte);
                if TODO_DE_SEGURANCA.is_match(t) {
                    a.add(
                        "SEC016",
                        n,
                        "TODO/FIXME de segurança",
                        "resolva antes de subir ou abra um ticket e cite-o no comentário",
                        t.trim(),
                    );
                }
            }
            "annotation" | "marker_annotation" => anotacao(&mut a, n),
            "navigation_expression" | "field_access" => {
                let t = texto(n, fonte);
                if [
                    "NoopHostnameVerifier.INSTANCE",
                    "InsecureTrustManagerFactory.INSTANCE",
                ]
                .contains(&t)
                    || t.ends_with("ALLOW_ALL_HOSTNAME_VERIFIER")
                {
                    a.add(
                        "SEC007",
                        n,
                        format!("verificação de TLS desligada ({t})"),
                        "valide certificado e hostname; para teste, use um truststore com o certificado",
                        t,
                    );
                }
            }
            k if eh_funcao(k) => {
                funcao_tls(&mut a, n);
                entrada_do_request(&mut a, n);
            }
            _ => {}
        }
        if let Some(c) = chamada(n, fonte) {
            chamadas(&mut a, n, &c);
        }
    });
}

// ---------------------------------------------------------------------------------------
// SEC001 credenciais, SEC002 (variável com SQL montado), SEC019 IP fixo

fn atribuicao(a: &mut Achados, n: Node, nome: &str, valor: Node) {
    let fonte = a.ctx.fonte;
    let valor = sem_parenteses(valor);
    if let Some(lit) = literal(valor, fonte)
        && nome_de_segredo(nome)
        && valor_de_segredo(&lit)
        && a.segredos.insert(linha(n))
    {
        a.add(
            "SEC001",
            n,
            format!("credencial fixa no código ({nome})"),
            "leia de variável de ambiente ou cofre de segredos; troque a credencial, ela já vazou no git",
            nome,
        );
    }
    if montada(valor, fonte).is_some_and(|t| parece_sql(&t))
        && let Some(f) = funcao_dona(n)
    {
        a.sql_montado
            .entry(f.id())
            .or_default()
            .insert(nome.to_string());
    }
}

fn string(a: &mut Achados, n: Node) {
    let fonte = a.ctx.fonte;
    let Some(t) = literal(n, fonte) else { return };
    if let Some(tipo) = formato_de_segredo(&t) {
        if a.segredos.insert(linha(n)) {
            a.add(
                "SEC001",
                n,
                format!("{tipo} no código"),
                "revogue a chave e leia de variável de ambiente ou cofre de segredos",
                tipo,
            );
        }
        return;
    }
    if let Some(ip) = IPV4.captures(&t).and_then(|c| c.get(1)) {
        let ip = ip.as_str();
        let octetos: Vec<u32> = ip.split('.').filter_map(|o| o.parse().ok()).collect();
        let valido = octetos.len() == 4 && octetos.iter().all(|o| *o <= 255);
        let especial = ip.starts_with("127.")
            || ip == "0.0.0.0"
            || ip == "255.255.255.255"
            || ip.starts_with("192.0.2.")
            || ip.starts_with("198.51.100.")
            || ip.starts_with("203.0.113.");
        // Tudo abaixo de 10 parece versão (1.2.3.4), não endereço.
        let versao = octetos.iter().all(|o| *o < 10);
        if valido && !especial && !versao {
            a.add(
                "SEC019",
                n,
                format!("IP fixo no código ({ip})"),
                "leia o endereço da configuração (variável de ambiente ou application.yml)",
                ip,
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// Anotações: SEC008 @CrossOrigin, SEC009 @JsonTypeInfo

fn anotacao(a: &mut Achados, n: Node) {
    let fonte = a.ctx.fonte;
    let t = texto(n, fonte);
    let nome = t.trim_start_matches('@');
    let nome = nome[..nome.find(['(', ' ', '\n']).unwrap_or(nome.len())]
        .rsplit('.')
        .next()
        .unwrap_or("");
    match nome {
        "CrossOrigin" => {
            let sem_origem = !t.contains('(') || t.ends_with("()");
            if sem_origem || t.contains("\"*\"") {
                a.add(
                    "SEC008",
                    n,
                    "CORS liberado para qualquer origem (@CrossOrigin)",
                    "liste as origens confiáveis: @CrossOrigin(origins = [\"https://app.exemplo.com\"])",
                    "CrossOrigin",
                );
            }
        }
        "JsonTypeInfo" if t.contains("Id.CLASS") || t.contains("Id.MINIMAL_CLASS") => {
            a.add(
                "SEC009",
                n,
                "desserialização polimórfica pelo nome da classe (@JsonTypeInfo Id.CLASS)",
                "use JsonTypeInfo.Id.NAME com @JsonSubTypes fechados",
                "JsonTypeInfo",
            );
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------
// Chamadas

fn so_literais(n: Node, fonte: &str) -> bool {
    let mut ok = true;
    visitar(n, |x| match x.kind() {
        "identifier" => {
            let t = texto(x, fonte);
            if !matches!(
                t,
                "listOf" | "arrayOf" | "mutableListOf" | "List" | "of" | "Arrays" | "asList"
            ) {
                ok = false;
            }
        }
        "string_literal" if super::sintaxe::interpolada(x, fonte) => ok = false,
        _ => {}
    });
    ok
}

fn escopo<'t>(n: Node<'t>, raiz: Node<'t>) -> Node<'t> {
    funcao_dona(n).unwrap_or(raiz)
}

fn chamadas(a: &mut Achados, n: Node, c: &Chamada) {
    let fonte = a.ctx.fonte;
    let r = receptor_txt(c, fonte);
    let arg0 = c.args.first().copied();
    let lit0 = arg0
        .and_then(|x| literal(x, fonte))
        .map(|s| s.to_uppercase());

    // SEC002 SQL montado.
    if !c.construtor
        && SQL_SORVEDOUROS.contains(&c.nome)
        && let Some(x) = arg0
    {
        let direto = montada(x, fonte).is_some_and(|t| parece_sql(&t));
        let variavel = x.kind() == "identifier"
            && funcao_dona(n).is_some_and(|f| {
                a.sql_montado
                    .get(&f.id())
                    .is_some_and(|s| s.contains(texto(x, fonte)))
            });
        if direto || variavel {
            a.add(
                "SEC002",
                n,
                format!("SQL montado com concatenação em {}()", c.nome),
                "use parâmetros (? ou :nome) e passe os valores separados",
                texto(x, fonte),
            );
        }
    }

    // SEC003 comando do sistema montado.
    let comando = (c.nome == "exec" && r.contains("Runtime"))
        || (c.construtor && c.nome == "ProcessBuilder")
        || (c.nome == "command" && r.contains("ProcessBuilder"));
    if comando && c.args.iter().any(|x| !so_literais(*x, fonte)) {
        a.add(
            "SEC003",
            n,
            format!("comando do sistema com parte variável ({})", c.nome),
            "passe uma lista fixa de argumentos e valide o valor contra uma lista permitida",
            c.nome,
        );
    }

    // SEC004 hash fraco; SEC005 cifra fraca; SEC007 protocolo fraco.
    if c.nome == "getInstance" {
        let alvo = r.rsplit('.').next().unwrap_or(&r);
        match (alvo, lit0.as_deref()) {
            ("MessageDigest", Some(alg))
                if matches!(alg, "MD2" | "MD4" | "MD5" | "SHA" | "SHA1" | "SHA-1") =>
            {
                a.add(
                    "SEC004",
                    n,
                    format!("hash fraco ({alg})"),
                    "use SHA-256 ou maior; para senha, BCrypt/Argon2 (PasswordEncoder)",
                    alg,
                );
            }
            ("Cipher" | "KeyGenerator" | "SecretKeyFactory", Some(alg)) if cifra_fraca(alg) => {
                a.add(
                    "SEC005",
                    n,
                    format!("cifra fraca ou modo inseguro ({alg})"),
                    "use AES/GCM/NoPadding com IV aleatório",
                    alg,
                );
            }
            ("SSLContext", Some(p))
                if matches!(p, "SSL" | "SSLV2" | "SSLV3" | "TLSV1" | "TLSV1.1") =>
            {
                a.add(
                    "SEC007",
                    n,
                    format!("protocolo TLS fraco ({p})"),
                    "use TLSv1.2 ou TLSv1.3",
                    p,
                );
            }
            (_, _) if senha_sem_hash(alvo) => {
                a.add(
                    "SEC004",
                    n,
                    format!("senha com hash fraco ou sem hash ({alvo})"),
                    "use BCryptPasswordEncoder ou Argon2PasswordEncoder",
                    alvo,
                );
            }
            _ => {}
        }
    }
    if c.construtor && senha_sem_hash(c.nome) {
        a.add(
            "SEC004",
            n,
            format!("senha com hash fraco ou sem hash ({})", c.nome),
            "use BCryptPasswordEncoder ou Argon2PasswordEncoder",
            c.nome,
        );
    }
    if (r.ends_with("DigestUtils")
        && matches!(
            c.nome,
            "md2" | "md2Hex" | "md5" | "md5Hex" | "sha" | "shaHex" | "sha1" | "sha1Hex"
        ))
        || (r == "Hashing" && matches!(c.nome, "md5" | "sha1"))
    {
        a.add(
            "SEC004",
            n,
            format!("hash fraco ({}.{})", r, c.nome),
            "use SHA-256 ou maior; para senha, BCrypt/Argon2",
            c.nome,
        );
    }

    // SEC006 gerador previsível em contexto de segredo.
    let aleatorio = (c.construtor && c.nome == "Random")
        || (r == "Math" && c.nome == "random")
        || (r == "ThreadLocalRandom" && c.nome == "current")
        || (r == "Random" && c.nome.starts_with("next"))
        || (r.ends_with("RandomStringUtils") && c.nome != "secure");
    if aleatorio && contexto_de_segredo(n, fonte) {
        a.add(
            "SEC006",
            n,
            "gerador aleatório previsível para valor secreto",
            "use SecureRandom (Java) ou java.security.SecureRandom no Kotlin",
            c.nome,
        );
    }

    // SEC007 verificação de hostname desligada.
    if matches!(
        c.nome,
        "setHostnameVerifier"
            | "hostnameVerifier"
            | "setDefaultHostnameVerifier"
            | "setSSLHostnameVerifier"
    ) {
        let aceita_tudo = c
            .args
            .iter()
            .chain(c.lambda.iter())
            .any(|x| lambda_sempre_true(*x, fonte));
        if aceita_tudo {
            a.add(
                "SEC007",
                n,
                "hostname do TLS não é verificado",
                "remova o HostnameVerifier que aceita tudo",
                c.nome,
            );
        }
    }
    if c.construtor && matches!(c.nome, "NoopHostnameVerifier" | "TrustAllStrategy") {
        a.add(
            "SEC007",
            n,
            format!("verificação de TLS desligada ({})", c.nome),
            "valide certificado e hostname",
            c.nome,
        );
    }

    // SEC008 CORS.
    let origens = matches!(
        c.nome,
        "allowedOrigins"
            | "allowedOriginPatterns"
            | "addAllowedOrigin"
            | "addAllowedOriginPattern"
            | "setAllowedOrigins"
            | "setAllowedOriginPatterns"
    );
    if origens && c.args.iter().any(|x| texto(*x, fonte).contains("\"*\"")) {
        a.add(
            "SEC008",
            n,
            format!("CORS liberado para qualquer origem ({})", c.nome),
            "liste as origens confiáveis em vez de \"*\"",
            c.nome,
        );
    }
    if matches!(c.nome, "setHeader" | "addHeader" | "header")
        && lit0.as_deref() == Some("ACCESS-CONTROL-ALLOW-ORIGIN")
        && c.args.get(1).and_then(|x| literal(*x, fonte)).as_deref() == Some("*")
    {
        a.add(
            "SEC008",
            n,
            "CORS liberado para qualquer origem (cabeçalho)",
            "devolva só origens confiáveis",
            c.nome,
        );
    }

    // SEC009 desserialização insegura.
    let des = (c.construtor && c.nome == "ObjectInputStream" && !fonte.contains("resolveClass"))
        || (c.construtor && c.nome == "XMLDecoder")
        || (c.construtor
            && c.nome == "XStream"
            && !fonte.contains("allowTypes")
            && !fonte.contains("addPermission"))
        || matches!(
            c.nome,
            "enableDefaultTyping" | "activateDefaultTyping" | "activateDefaultTypingAsProperty"
        );
    if des {
        a.add(
            "SEC009",
            n,
            format!("desserialização insegura ({})", c.nome),
            "não desserialize objeto Java de fonte externa; use JSON com tipos fechados",
            c.nome,
        );
    }

    // SEC010 XML sem proteção contra XXE.
    let fabrica = r.rsplit('.').next().unwrap_or(&r);
    let xml = (matches!(
        c.nome,
        "newInstance" | "newFactory" | "newDefaultFactory" | "newDefaultInstance"
    ) && matches!(
        fabrica,
        "DocumentBuilderFactory"
            | "SAXParserFactory"
            | "XMLInputFactory"
            | "TransformerFactory"
            | "SAXTransformerFactory"
            | "SchemaFactory"
    )) || (fabrica == "XMLReaderFactory" && c.nome == "createXMLReader")
        || (c.construtor && matches!(c.nome, "SAXBuilder" | "SAXReader"));
    if xml {
        let corpo = texto(escopo(n, a.ctx.raiz), fonte);
        let protegido = [
            "setFeature",
            "setProperty",
            "setAttribute",
            "setExpandEntityReferences",
            "ACCESS_EXTERNAL",
            "disallow-doctype-decl",
            "SUPPORT_DTD",
            "IS_SUPPORTING_EXTERNAL_ENTITIES",
        ]
        .iter()
        .any(|p| corpo.contains(p));
        if !protegido {
            let qual = if c.construtor { c.nome } else { fabrica };
            a.add(
                "SEC010",
                n,
                format!("parser XML sem proteção contra XXE ({qual})"),
                "desligue DTD e entidades externas: setFeature(\"http://apache.org/xml/features/disallow-doctype-decl\", true)",
                qual,
            );
        }
    }

    // SEC011 CSRF desligado (exceto API sem sessão: SessionCreationPolicy.STATELESS).
    let csrf_off = (c.nome == "disable"
        && c.receptor
            .and_then(|x| chamada(x, fonte))
            .is_some_and(|x| x.nome == "csrf"))
        || (c.nome == "csrf"
            && c.args
                .iter()
                .chain(c.lambda.iter())
                .any(|x| texto(*x, fonte).contains("disable")));
    if csrf_off && !fonte.contains("STATELESS") {
        a.add(
            "SEC011",
            n,
            "proteção CSRF desligada numa aplicação com sessão",
            "mantenha o CSRF ligado ou declare a API sem sessão (SessionCreationPolicy.STATELESS)",
            "csrf",
        );
    }

    // SEC012 rota liberada ampla ou sensível.
    if let Some(m) = rota_liberada(c, fonte) {
        a.add(
            "SEC012",
            n,
            format!("rota liberada sem autenticação ({m})"),
            "libere só rotas públicas específicas; proteja actuator, console e admin",
            &m,
        );
    }

    // SEC013 cookie sem Secure/HttpOnly.
    if c.construtor && c.nome == "Cookie" && c.args.len() == 2 {
        let corpo = texto(escopo(n, a.ctx.raiz), fonte);
        let falta = flags_de_cookie(corpo, false);
        if !falta.is_empty() {
            a.add(
                "SEC013",
                n,
                format!("cookie sem {}", falta.join(" e ")),
                "marque o cookie com setHttpOnly(true) e setSecure(true)",
                "Cookie",
            );
        }
    }
    if c.nome == "from" && r.ends_with("ResponseCookie") {
        let mut topo = n;
        while let Some(p) = topo.parent().filter(|p| {
            matches!(
                p.kind(),
                "method_invocation" | "call_expression" | "navigation_expression" | "field_access"
            )
        }) {
            topo = p;
        }
        let falta = flags_de_cookie(texto(topo, fonte), true);
        if !falta.is_empty() {
            a.add(
                "SEC013",
                n,
                format!("cookie sem {}", falta.join(" e ")),
                "acrescente .httpOnly(true).secure(true) ao ResponseCookie",
                "ResponseCookie",
            );
        }
    }

    // SEC014 JWT sem verificar a assinatura.
    let jwt = (c.nome == "decode"
        && r == "JWT"
        && !texto(escopo(n, a.ctx.raiz), fonte).contains(".verify("))
        || matches!(
            c.nome,
            "parseClaimsJwt" | "parseUnsecuredClaims" | "parseUnsecuredContent"
        )
        || (c.nome == "none" && r == "Algorithm");
    if jwt {
        a.add(
            "SEC014",
            n,
            format!("JWT lido sem verificar a assinatura ({})", c.nome),
            "verifique antes de confiar: JWT.require(algoritmo).build().verify(token)",
            c.nome,
        );
    }

    // SEC015 dado sensível no log.
    if eh_log(c, &r) {
        let sensivel = c.args.iter().find_map(|x| dado_sensivel_no_log(*x, fonte));
        if let Some(i) = sensivel {
            a.add(
                "SEC015",
                n,
                format!("log com dado sensível ({i})"),
                "não registre segredo nem token; registre um identificador ou mascare o valor",
                i,
            );
        }
    }
}

fn cifra_fraca(alg: &str) -> bool {
    let base = alg.split('/').next().unwrap_or(alg);
    matches!(
        base,
        "DES" | "DESEDE" | "TRIPLEDES" | "RC2" | "RC4" | "ARCFOUR" | "BLOWFISH"
    ) || alg.contains("/ECB/")
        || alg == "AES"
        || alg == "RSA/NONE/NOPADDING"
}

fn senha_sem_hash(nome: &str) -> bool {
    matches!(
        nome,
        "NoOpPasswordEncoder"
            | "StandardPasswordEncoder"
            | "Md4PasswordEncoder"
            | "Md5PasswordEncoder"
            | "ShaPasswordEncoder"
            | "MessageDigestPasswordEncoder"
            | "LdapShaPasswordEncoder"
    )
}

fn contexto_de_segredo(n: Node, fonte: &str) -> bool {
    const EXTRAS: &[&str] = &[
        "otp",
        "nonce",
        "salt",
        "verificacao",
        "verification",
        "reset",
        "recuperacao",
        "recovery",
        "session",
        "sessao",
        "convite",
        "invite",
    ];
    let sensivel = |nome: &str| {
        nome_de_segredo(nome) || palavras(nome).iter().any(|w| EXTRAS.contains(&w.as_str()))
    };
    let mut p = n.parent();
    while let Some(x) = p {
        let nome = match x.kind() {
            "variable_declarator" => x.child_by_field_name("name").map(|i| texto(i, fonte)),
            "property_declaration" => filhos(x)
                .into_iter()
                .find(|c| c.kind() == "variable_declaration")
                .map(|d| texto(d, fonte)),
            k if eh_funcao(k) => Some(nome_da_funcao(x, fonte)),
            _ => None,
        };
        if nome.is_some_and(sensivel) {
            return true;
        }
        if eh_funcao(x.kind()) {
            return false;
        }
        p = x.parent();
    }
    false
}

/// Lambda (ou referência) que aceita qualquer hostname: corpo `true`.
fn lambda_sempre_true(n: Node, fonte: &str) -> bool {
    let t = texto(n, fonte);
    if t.contains("NoopHostnameVerifier") || t.contains("ALLOW_ALL") {
        return true;
    }
    if !eh_lambda(n.kind()) {
        return false;
    }
    let corpo = match n.kind() {
        "lambda_expression" => n.child_by_field_name("body").map(|b| texto(b, fonte)),
        _ => filhos(n)
            .into_iter()
            .rfind(|c| c.is_named() && c.kind() != "lambda_parameters")
            .map(|b| texto(b, fonte)),
    };
    corpo.is_some_and(|b| {
        let b: String = b.chars().filter(|c| !c.is_whitespace()).collect();
        matches!(b.as_str(), "true" | "{returntrue;}" | "returntrue")
    })
}

fn flags_de_cookie(t: &str, builder: bool) -> Vec<&'static str> {
    let sem_espaco: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    let tem = |java: &str, kotlin: &str, b: &str| {
        if builder {
            sem_espaco.contains(b)
        } else {
            sem_espaco.contains(java) || sem_espaco.contains(kotlin)
        }
    };
    let mut falta = Vec::new();
    if !tem("setHttpOnly(true)", "isHttpOnly=true", ".httpOnly(true)") {
        falta.push("HttpOnly");
    }
    if !tem("setSecure(true)", "secure=true", ".secure(true)") {
        falta.push("Secure");
    }
    falta
}

fn rota_liberada(c: &Chamada, fonte: &str) -> Option<String> {
    const MATCHERS: &[&str] = &[
        "requestMatchers",
        "antMatchers",
        "mvcMatchers",
        "regexMatchers",
        "pathMatchers",
    ];
    let ampla_ou_sensivel = |s: &str| {
        let l = s.to_lowercase();
        s == "/**"
            || s == "/*"
            || [
                "actuator",
                "h2-console",
                "/admin",
                "/debug",
                "heapdump",
                "/env",
            ]
            .iter()
            .any(|k| l.contains(k))
    };
    // Kotlin DSL: authorize(anyRequest, permitAll) / authorize("/**", permitAll).
    if c.nome == "authorize" && c.args.len() == 2 && texto(c.args[1], fonte) == "permitAll" {
        let alvo = texto(c.args[0], fonte);
        if alvo == "anyRequest" {
            return Some("anyRequest".into());
        }
        if let Some(s) = literal(c.args[0], fonte).filter(|s| ampla_ou_sensivel(s)) {
            return Some(s);
        }
        return None;
    }
    let liberar = c.nome == "permitAll";
    let ignorar = c
        .receptor
        .and_then(|x| chamada(x, fonte))
        .is_some_and(|x| x.nome == "ignoring");
    let (alvo, quem) = if liberar {
        (c.receptor.and_then(|x| chamada(x, fonte))?, "permitAll")
    } else if ignorar && (MATCHERS.contains(&c.nome) || c.nome == "anyRequest") {
        (
            Chamada {
                nome: c.nome,
                receptor: None,
                args: c.args.clone(),
                lambda: None,
                construtor: false,
            },
            "ignoring",
        )
    } else {
        return None;
    };
    if alvo.nome == "anyRequest" {
        return Some(format!("anyRequest().{quem}"));
    }
    if MATCHERS.contains(&alvo.nome) {
        let preflight = alvo.args.first().is_some_and(|x| {
            let t = texto(*x, fonte).trim();
            t == "OPTIONS"
                || t == "HEAD"
                || t.ends_with(".OPTIONS")
                || t.ends_with(".HEAD")
                || matches!(literal(*x, fonte).as_deref(), Some("OPTIONS" | "HEAD"))
        });
        return alvo
            .args
            .iter()
            .filter_map(|x| literal(*x, fonte))
            .find(|s| {
                if preflight && matches!(s.as_str(), "/**" | "/*") {
                    return false;
                }
                ampla_ou_sensivel(s)
            });
    }
    None
}

/// Variável ou campo que carrega um valor sensível ao ser logado diretamente.
/// Além de `nome_de_segredo`, inclui `authorization`/`authHeader` (que guardam o Bearer token),
/// mas exclui constantes ALL_CAPS (ex.: `TOKEN_HEADER` = nome do cabeçalho).
fn nome_sensivel_no_log(nome: &str) -> bool {
    if nome_de_segredo(nome) {
        return true;
    }
    let tudo_maiuscula = nome
        .chars()
        .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit());
    if tudo_maiuscula {
        return false;
    }
    let p = palavras(nome);
    let junto = p.concat();
    matches!(
        junto.as_str(),
        "authorization" | "authheader" | "authorizationheader" | "bearer" | "bearertoken" | "jwt"
    )
}

/// Membro que devolve o próprio valor do objeto (ou um pedaço dele), não um metadado seguro.
fn membro_expoe_valor(membro: &str) -> bool {
    nome_sensivel_no_log(membro)
        || matches!(
            membro,
            "toString"
                | "value"
                | "getValue"
                | "asString"
                | "trim"
                | "strip"
                | "substring"
                | "toCharArray"
                | "getBytes"
                | "chars"
                | "raw"
                | "getRaw"
                | "getToken"
                | "getPassword"
                | "getSecret"
        )
}

/// Encontra identificador sensível efetivamente logado numa expressão de argumento.
/// Quando `token` é apenas receptor de um getter seguro (`token.usuarioId()`,
/// `dadosToken.perfis()`, `senha.isEmpty()`), o valor logado não é o segredo.
fn dado_sensivel_no_log<'t>(arg: Node<'t>, fonte: &'t str) -> Option<&'t str> {
    let mut achado: Option<&'t str> = None;
    visitar(arg, |x| {
        if achado.is_some() {
            return;
        }
        match x.kind() {
            "string_literal" => {
                // Template simples do Kotlin: `$senha` (mas não `${dadosToken.usuarioId}`,
                // cujos nós internos são visitados normalmente como AST).
                for i in identificadores(x, fonte) {
                    if !texto(x, fonte).contains(&format!("${{{i}.")) && nome_sensivel_no_log(i) {
                        // Só conta se veio de `$i` (não de dentro de `${...}`).
                        if texto(x, fonte).contains(&format!("${i}")) {
                            achado = Some(i);
                            return;
                        }
                    }
                }
            }
            "identifier" => {
                let nome = texto(x, fonte);
                if !nome_sensivel_no_log(nome) {
                    return;
                }
                // Se for receptor de método ou acesso a campo seguro, não vaza o segredo.
                if let Some(p) = x.parent() {
                    let membro = match p.kind() {
                        "method_invocation" if p.child_by_field_name("object") == Some(x) => {
                            p.child_by_field_name("name").map(|m| texto(m, fonte))
                        }
                        "field_access" if p.child_by_field_name("object") == Some(x) => {
                            p.child_by_field_name("field").map(|m| texto(m, fonte))
                        }
                        "navigation_expression" if p.named_child(0) == Some(x) => {
                            p.named_child(1).map(|m| {
                                let t = texto(m, fonte);
                                t.trim_start_matches('.')
                            })
                        }
                        _ => None,
                    };
                    if let Some(m) = membro
                        && !membro_expoe_valor(m)
                    {
                        return;
                    }
                }
                achado = Some(nome);
            }
            _ => {}
        }
    });
    achado
}

fn eh_log(c: &Chamada, r: &str) -> bool {
    let l = r.to_lowercase();
    let logger = matches!(
        c.nome,
        "trace" | "debug" | "info" | "warn" | "warning" | "error" | "fatal"
    ) && (l.ends_with("log") || l.ends_with("logger") || l.ends_with("logger()"));
    let saida = matches!(c.nome, "println" | "print" | "printf")
        && (r.is_empty() || r.ends_with("System.out") || r.ends_with("System.err"));
    logger || saida
}

// ---------------------------------------------------------------------------------------
// SEC007 TrustManager/HostnameVerifier feitos à mão

fn funcao_tls(a: &mut Achados, f: Node) {
    let fonte = a.ctx.fonte;
    let nome = nome_da_funcao(f, fonte);
    let Some(corpo) = lang::corpo_da_funcao(f) else {
        return;
    };
    let instrucoes: Vec<Node> = {
        let bloco = if corpo.kind() == "function_body" {
            filhos(corpo)
                .into_iter()
                .find(|c| c.kind() == "block")
                .unwrap_or(corpo)
        } else {
            corpo
        };
        filhos(bloco)
            .into_iter()
            .filter(|c| c.is_named() && !eh_comentario(c.kind()))
            .collect()
    };
    let vazio = instrucoes.is_empty() && corpo.kind() != "function_body"
        || (corpo.kind() == "function_body" && instrucoes.is_empty());
    if nome == "checkServerTrusted" && vazio {
        a.add(
            "SEC007",
            f,
            "TrustManager aceita qualquer certificado (checkServerTrusted vazio)",
            "use o TrustManager padrão; para teste, um truststore com o certificado",
            nome,
        );
    }
    if nome == "verify" && fonte.contains("HostnameVerifier") {
        let t: String = texto(corpo, fonte)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        if matches!(t.as_str(), "{returntrue;}" | "=true" | "{returntrue}") {
            a.add(
                "SEC007",
                f,
                "HostnameVerifier aceita qualquer hostname",
                "não sobrescreva a verificação de hostname",
                nome,
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// SEC020 entrada do request chegando a um sorvedouro, dentro da mesma função

fn parametros_do_request<'t>(f: Node<'t>, fonte: &'t str) -> Vec<&'t str> {
    let mut v = Vec::new();
    let lista = f.child_by_field_name("parameters").or_else(|| {
        filhos(f)
            .into_iter()
            .find(|c| c.kind() == "function_value_parameters")
    });
    let Some(lista) = lista else { return v };
    for p in filhos(lista) {
        let nome = match p.kind() {
            "formal_parameter" => p.child_by_field_name("name"),
            "parameter" => p.named_child(0).filter(|i| i.kind() == "identifier"),
            _ => None,
        };
        let Some(nome) = nome else { continue };
        if anotacoes(p, fonte)
            .iter()
            .any(|(a, _)| FONTES_ANOTADAS.contains(a))
        {
            v.push(texto(nome, fonte));
        }
    }
    v
}

/// Identificadores usados como valor (não o nome do método nem o campo depois do ponto).
fn usos<'t>(n: Node<'t>, fonte: &'t str) -> Vec<&'t str> {
    let mut v = Vec::new();
    visitar(n, |x| {
        if x.kind() == "string_literal" {
            v.extend(identificadores(x, fonte));
            return;
        }
        if x.kind() != "identifier" {
            return;
        }
        let Some(p) = x.parent() else { return };
        let depois_do_ponto = match p.kind() {
            "field_access" => p.child_by_field_name("field") == Some(x),
            "method_invocation" => p.child_by_field_name("name") == Some(x),
            "navigation_expression" => p.named_child(0) != Some(x),
            _ => false,
        };
        if !depois_do_ponto {
            v.push(texto(x, fonte));
        }
    });
    v
}

fn le_do_request(n: Node, fonte: &str) -> bool {
    let mut achou = false;
    visitar(n, |x| {
        if let Some(c) = chamada(x, fonte)
            && FONTES_CHAMADAS.contains(&c.nome)
        {
            achou = true;
        }
        if x.kind() == "indexing_expression" || x.kind() == "index_expression" {
            let t = texto(x, fonte);
            if t.contains("parameters[") || t.contains("queryParameters[") {
                achou = true;
            }
        }
    });
    achou
}

fn sorvedouro(c: &Chamada, r: &str) -> Option<&'static str> {
    let rl = r.to_lowercase();
    if c.construtor
        && matches!(
            c.nome,
            "File"
                | "FileInputStream"
                | "FileOutputStream"
                | "FileReader"
                | "FileWriter"
                | "RandomAccessFile"
        )
    {
        return Some("arquivo");
    }
    if (r == "Paths" && c.nome == "get")
        || (r == "Path" && c.nome == "of")
        || c.nome == "resolve"
        || (r == "Files"
            && matches!(
                c.nome,
                "readAllBytes"
                    | "readString"
                    | "readAllLines"
                    | "lines"
                    | "newInputStream"
                    | "newOutputStream"
                    | "newBufferedReader"
                    | "newBufferedWriter"
                    | "write"
                    | "writeString"
                    | "delete"
                    | "deleteIfExists"
                    | "copy"
                    | "move"
                    | "list"
                    | "walk"
                    | "exists"
            ))
    {
        return Some("arquivo");
    }
    if (c.construtor && matches!(c.nome, "URL" | "URI"))
        || (r == "URI" && c.nome == "create")
        || (r == "Jsoup" && c.nome == "connect")
        || c.nome == "uri"
        || (c.nome == "url" && rl.contains("builder"))
        || ((rl.contains("rest") || rl.contains("template") || rl.contains("client"))
            && matches!(
                c.nome,
                "getForObject"
                    | "getForEntity"
                    | "postForObject"
                    | "postForEntity"
                    | "exchange"
                    | "patchForObject"
                    | "headForHeaders"
            ))
    {
        return Some("URL (SSRF)");
    }
    if c.nome == "sendRedirect" || (c.construtor && c.nome == "RedirectView") {
        return Some("redirecionamento");
    }
    if (c.nome == "exec" && r.contains("Runtime")) || (c.construtor && c.nome == "ProcessBuilder") {
        return Some("comando do sistema");
    }
    if (r == "Class" && c.nome == "forName") || c.nome == "loadClass" {
        return Some("carga de classe");
    }
    if c.nome == "parseExpression" {
        return Some("expressão SpEL");
    }
    None
}

fn entrada_do_request(a: &mut Achados, f: Node) {
    let fonte = a.ctx.fonte;
    let Some(corpo) = lang::corpo_da_funcao(f) else {
        return;
    };
    let mut sujos: HashSet<&str> = parametros_do_request(f, fonte).into_iter().collect();
    if sujos.is_empty() && !le_do_request(corpo, fonte) {
        return;
    }
    let contaminado = |n: Node, sujos: &HashSet<&str>| {
        usos(n, fonte).iter().any(|i| sujos.contains(i)) || le_do_request(n, fonte)
    };
    let mut achados: Vec<(Node, String, String)> = Vec::new();
    visitar(corpo, |n| {
        // Propagação: variável local que recebe valor sujo também fica suja.
        let atrib = match n.kind() {
            "variable_declarator" => n
                .child_by_field_name("name")
                .zip(n.child_by_field_name("value")),
            "property_declaration" => filhos(n)
                .into_iter()
                .find(|c| c.kind() == "variable_declaration")
                .and_then(|d| d.named_child(0))
                .zip(n.named_child(n.named_child_count().saturating_sub(1) as u32)),
            "assignment_expression" | "assignment" => n
                .child_by_field_name("left")
                .zip(n.child_by_field_name("right")),
            _ => None,
        };
        if let Some((nome, valor)) = atrib
            && nome.kind() == "identifier"
            && contaminado(valor, &sujos)
        {
            sujos.insert(texto(nome, fonte));
        }
        if n.kind() == "binary_expression"
            && let Some(esq) = n
                .child_by_field_name("left")
                .and_then(|x| conteudo(x, fonte))
        {
            let dir = n.child_by_field_name("right");
            if esq.starts_with("redirect:") && dir.is_some_and(|d| contaminado(d, &sujos)) {
                achados.push((n, "redirecionamento".into(), "redirect:".into()));
            }
        }
        let Some(c) = chamada(n, fonte) else { return };
        let r = receptor_txt(&c, fonte);
        let tipo = if !c.construtor && SQL_SORVEDOUROS.contains(&c.nome) {
            // No SQL só o texto da consulta conta: parâmetro ligado (?) é o jeito certo.
            c.args
                .first()
                .filter(|x| montada(**x, fonte).is_some() && contaminado(**x, &sujos))
                .map(|_| "SQL")
        } else {
            sorvedouro(&c, &r).filter(|_| {
                c.args
                    .iter()
                    .chain(c.lambda.iter())
                    .any(|x| contaminado(*x, &sujos))
            })
        };
        if let Some(tipo) = tipo {
            achados.push((n, tipo.to_string(), c.nome.to_string()));
        }
    });
    let mut vistos = HashSet::new();
    for (n, tipo, nome) in achados {
        // Chamadas aninhadas na mesma linha (Files.readAllBytes(Paths.get(x))) contam uma vez.
        if !vistos.insert((linha(n), tipo.clone())) {
            continue;
        }
        a.add(
            "SEC020",
            n,
            format!("entrada do request chega em {tipo} ({nome})"),
            "valide contra uma lista permitida ou normalize e confira o resultado antes de usar",
            &format!("{tipo}:{nome}"),
        );
    }
}

#[cfg(test)]
mod casos {
    use super::*;
    use crate::config::Limites;
    use crate::lang::{Linguagem, Parsers};

    fn achar(fonte: &str, ling: Linguagem) -> Vec<(String, usize, String)> {
        let mut p = Parsers::novo();
        let t = p.ler(ling, fonte).unwrap();
        assert!(!t.root_node().has_error(), "sintaxe");
        let lim = Limites::default();
        let ctx = Contexto {
            arquivo: "A",
            fonte,
            ling,
            raiz: t.root_node(),
            limites: &lim,
            pacote: None,
            extras: None,
        };
        let mut v = Vec::new();
        verificar(&ctx, &mut v);
        v.into_iter()
            .map(|x| (x.codigo.to_string(), x.linha, x.mensagem))
            .collect()
    }

    fn codigos(v: &[(String, usize, String)]) -> Vec<(&str, usize)> {
        v.iter().map(|(c, l, _)| (c.as_str(), *l)).collect()
    }

    #[test]
    fn credenciais() {
        let j = r#"class A {
  private static final String JWT_SECRET = "s3gr3d0-forte";
  private static final String TOKEN_HEADER = "Authorization";
  private String senha = "Senha inválida";
  String k = "AKIAABCDEFGHIJKLMNOP";
  String chave = "jwt.secret";
}"#;
        let v = achar(j, Linguagem::Java);
        assert_eq!(codigos(&v), [("SEC001", 2), ("SEC001", 5)], "{v:?}");
        let k = "object Config {\n    const val apiKey = \"abc123xyz\"\n    fun f() = conectar(password = \"root1234\")\n}";
        let v = achar(k, Linguagem::Kotlin);
        assert_eq!(codigos(&v), [("SEC001", 2), ("SEC001", 3)], "{v:?}");
    }

    #[test]
    fn sql_e_comando() {
        let j = r#"class Dao {
  void f(String nome, int id) throws Exception {
    st.executeQuery("SELECT * FROM u WHERE nome = '" + nome + "'");
    String sql = "DELETE FROM u WHERE id = " + id;
    st.executeUpdate(sql);
    jdbc.query("SELECT * FROM u WHERE id = ?", id);
    Runtime.getRuntime().exec("ping " + nome);
    new ProcessBuilder("ls", "-la").start();
  }
}"#;
        let v = achar(j, Linguagem::Java);
        assert_eq!(
            codigos(&v),
            [("SEC002", 3), ("SEC002", 5), ("SEC003", 7)],
            "{v:?}"
        );
        let k = "fun f(id: Long) {\n    jdbc.query(\"SELECT * FROM t WHERE id = $id\")\n    em.createQuery(\"select u from U u\")\n}";
        let v = achar(k, Linguagem::Kotlin);
        assert_eq!(codigos(&v), [("SEC002", 2)], "{v:?}");
    }

    #[test]
    fn cripto_e_aleatorio() {
        let j = r#"class C {
  void f() throws Exception {
    MessageDigest.getInstance("MD5");
    MessageDigest.getInstance("SHA-256");
    Cipher.getInstance("AES/ECB/PKCS5Padding");
    Cipher.getInstance("AES/GCM/NoPadding");
    PasswordEncoder p = NoOpPasswordEncoder.getInstance();
    SSLContext.getInstance("TLSv1");
  }
  String gerarToken() { return String.valueOf(new Random().nextInt()); }
  int sortear() { return new Random().nextInt(6); }
}"#;
        let v = achar(j, Linguagem::Java);
        assert_eq!(
            codigos(&v),
            [
                ("SEC004", 3),
                ("SEC005", 5),
                ("SEC004", 7),
                ("SEC007", 8),
                ("SEC006", 10)
            ],
            "{v:?}"
        );
    }

    #[test]
    fn web() {
        let j = r#"@CrossOrigin
class Ctl {
  SecurityFilterChain f(HttpSecurity http) throws Exception {
    http.csrf().disable()
        .authorizeHttpRequests(a -> a.requestMatchers("/h2-console/**").permitAll().anyRequest().authenticated());
    Cookie c = new Cookie("sessao", valor);
    c.setHttpOnly(true);
    return http.build();
  }
  void g(HttpServletResponse r) { r.setHeader("Access-Control-Allow-Origin", "*"); }
}"#;
        let v = achar(j, Linguagem::Java);
        let c: Vec<&str> = v.iter().map(|x| x.0.as_str()).collect();
        assert_eq!(
            c,
            ["SEC008", "SEC011", "SEC012", "SEC013", "SEC008"],
            "{v:?}"
        );
        assert!(v[3].2 == "cookie sem Secure", "{v:?}");
        // API sem sessão: CSRF desligado é o esperado.
        let k = "fun f(http: HttpSecurity) {\n    http.csrf { it.disable() }.sessionManagement { it.sessionCreationPolicy(SessionCreationPolicy.STATELESS) }\n}";
        assert!(achar(k, Linguagem::Kotlin).is_empty());
    }

    #[test]
    fn tls_jwt_xml_desserializacao() {
        let j = r#"class T implements X509TrustManager {
  public void checkServerTrusted(X509Certificate[] c, String a) { }
  void f(String token, InputStream in) throws Exception {
    conn.setHostnameVerifier((h, s) -> true);
    DecodedJWT d = JWT.decode(token);
    DocumentBuilderFactory.newInstance().newDocumentBuilder();
    new ObjectInputStream(in).readObject();
  }
  void g() { var f = DocumentBuilderFactory.newInstance(); f.setFeature("x", true); }
}"#;
        let v = achar(j, Linguagem::Java);
        assert_eq!(
            codigos(&v),
            [
                ("SEC007", 2),
                ("SEC007", 4),
                ("SEC014", 5),
                ("SEC010", 6),
                ("SEC009", 7)
            ],
            "{v:?}"
        );
    }

    #[test]
    fn log_todo_ip() {
        let k = r#"class S {
    fun entrar(senha: String, tokenService: TokenService) {
        // TODO validar a permissão do usuário
        log.info("login com senha $senha")
        log.info("usando ${tokenService.nome}")
        val host = "10.0.0.15:8080"
        val versao = "1.2.3.4"
    }
}"#;
        let v = achar(k, Linguagem::Kotlin);
        assert_eq!(
            codigos(&v),
            [("SEC016", 3), ("SEC015", 4), ("SEC019", 6)],
            "{v:?}"
        );

        let j = r#"class LogCaso {
  void f(DecodedToken dadosToken, String token, String authHeader) {
    log.info("usuario={}", dadosToken.usuarioId());
    System.out.println("perfis=" + dadosToken.perfis());
    log.info("token={}", token);
    log.debug("auth={}", authHeader);
    log.warn("raw={}", dadosToken.toString());
  }
}"#;
        let v = achar(j, Linguagem::Java);
        assert_eq!(
            codigos(&v),
            [("SEC015", 5), ("SEC015", 6), ("SEC015", 7)],
            "{v:?}"
        );

        // Preflight CORS OPTIONS em /** não dispara SEC012.
        let sec = r#"class Sec {
  void f(HttpSecurity http) {
    http.authorizeHttpRequests(a -> {
      a.requestMatchers(HttpMethod.OPTIONS, "/**").permitAll();
      a.requestMatchers("/actuator/**").permitAll();
    });
  }
}"#;
        let v = achar(sec, Linguagem::Java);
        assert_eq!(codigos(&v), [("SEC012", 5)], "{v:?}");
    }

    #[test]
    fn entrada_do_request_ate_o_sorvedouro() {
        let j = r#"class Ctl {
  @GetMapping("/arq")
  byte[] baixar(@RequestParam String nome, @RequestParam int id) throws Exception {
    String caminho = "/dados/" + nome;
    jdbc.query("SELECT * FROM t WHERE id = ?", id);
    return Files.readAllBytes(Paths.get(caminho));
  }
  String ir(@RequestParam String url) { return "redirect:" + url; }
  void fixo() throws Exception { Files.readAllBytes(Paths.get("/etc/app.conf")); }
}"#;
        let v = achar(j, Linguagem::Java);
        let sec020: Vec<_> = v.iter().filter(|x| x.0 == "SEC020").collect();
        assert_eq!(sec020.len(), 2, "{v:?}");
        assert!(sec020[0].2.contains("arquivo"), "{v:?}");
        assert!(sec020[1].2.contains("redirecionamento"), "{v:?}");

        let k = "@GetMapping\nfun buscar(@RequestParam url: String) = RestTemplate().getForObject(url, String::class.java)";
        let v = achar(k, Linguagem::Kotlin);
        assert!(
            v.iter().any(|x| x.0 == "SEC020" && x.2.contains("SSRF")),
            "{v:?}"
        );
    }
}
