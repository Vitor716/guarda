//! Utilitários de sintaxe comuns às regras de segurança, Spring e correção: chamada de
//! método nas duas gramáticas, literal de string, string montada e nomes sensíveis.

use crate::lang::{eh_comentario, eh_funcao, filhos, texto};
use regex::Regex;
use std::sync::LazyLock;
use tree_sitter::Node;

/// Uma chamada de método, função ou construtor, nas duas gramáticas.
pub struct Chamada<'t> {
    /// Nome simples do método ou do tipo construído.
    pub nome: &'t str,
    /// O que vem antes do ponto (`a.b` em `a.b.c()`), se houver.
    pub receptor: Option<Node<'t>>,
    /// Argumentos (o valor, sem o nome do argumento nomeado do Kotlin).
    pub args: Vec<Node<'t>>,
    /// Lambda final do Kotlin (`csrf { it.disable() }`).
    pub lambda: Option<Node<'t>>,
    /// `new X(...)` no Java ou chamada com nome em maiúscula no Kotlin.
    pub construtor: bool,
}

fn simples(t: &str) -> &str {
    let t = t.split('<').next().unwrap_or(t).trim();
    t.rsplit('.').next().unwrap_or(t)
}

pub fn chamada<'t>(n: Node<'t>, fonte: &'t str) -> Option<Chamada<'t>> {
    match n.kind() {
        "method_invocation" => Some(Chamada {
            nome: texto(n.child_by_field_name("name")?, fonte),
            receptor: n.child_by_field_name("object"),
            args: args_java(n.child_by_field_name("arguments")),
            lambda: None,
            construtor: false,
        }),
        "object_creation_expression" => Some(Chamada {
            nome: simples(texto(n.child_by_field_name("type")?, fonte)),
            receptor: None,
            args: args_java(n.child_by_field_name("arguments")),
            lambda: None,
            construtor: true,
        }),
        "call_expression" => {
            let alvo = n.named_child(0)?;
            let (receptor, nome) = match alvo.kind() {
                "navigation_expression" => {
                    let ultimo =
                        alvo.named_child(alvo.named_child_count().checked_sub(1)? as u32)?;
                    (alvo.named_child(0), texto(ultimo, fonte))
                }
                "identifier" => (None, texto(alvo, fonte)),
                _ => return None,
            };
            let mut args = Vec::new();
            let mut lambda = None;
            for c in filhos(n) {
                match c.kind() {
                    "value_arguments" => {
                        for a in filhos(c)
                            .into_iter()
                            .filter(|a| a.kind() == "value_argument")
                        {
                            let k = a.named_child_count();
                            if let Some(v) = k.checked_sub(1).and_then(|i| a.named_child(i as u32))
                            {
                                args.push(v);
                            }
                        }
                    }
                    "annotated_lambda" => lambda = c.named_child(0),
                    _ => {}
                }
            }
            Some(Chamada {
                nome,
                receptor,
                args,
                lambda,
                construtor: nome.starts_with(|c: char| c.is_ascii_uppercase()),
            })
        }
        _ => None,
    }
}

fn args_java(lista: Option<Node>) -> Vec<Node> {
    lista.map_or_else(Vec::new, |l| {
        filhos(l)
            .into_iter()
            .filter(|c| c.is_named() && !eh_comentario(c.kind()))
            .collect()
    })
}

/// Texto do receptor (vazio se não houver), sem espaços.
pub fn receptor_txt(c: &Chamada, fonte: &str) -> String {
    c.receptor.map_or(String::new(), |r| {
        texto(r, fonte)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect()
    })
}

static INTERPOLACAO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$(\{|[A-Za-z_])").expect("regex"));

fn eh_string(n: Node) -> bool {
    matches!(n.kind(), "string_literal" | "text_block")
}

/// Conteúdo de uma string, sem as aspas (com a interpolação do Kotlin como está).
pub fn conteudo(n: Node, fonte: &str) -> Option<String> {
    if !eh_string(n) {
        return None;
    }
    let t = texto(n, fonte);
    let t = if t.starts_with("\"\"\"") && t.len() >= 6 {
        &t[3..t.len() - 3]
    } else if t.len() >= 2 {
        &t[1..t.len() - 1]
    } else {
        t
    };
    Some(t.to_string())
}

/// String do Kotlin com `$x` ou `${...}`.
pub fn interpolada(n: Node, fonte: &str) -> bool {
    n.kind() == "string_literal"
        && (filhos(n).iter().any(|c| c.kind() == "interpolation")
            || INTERPOLACAO.is_match(texto(n, fonte)))
}

/// Literal de string fixo (sem interpolação).
pub fn literal(n: Node, fonte: &str) -> Option<String> {
    if interpolada(n, fonte) {
        return None;
    }
    conteudo(n, fonte)
}

fn folhas_de_soma<'t>(n: Node<'t>, fonte: &str, v: &mut Vec<Node<'t>>) {
    let n = sem_parenteses(n);
    let soma = n.kind() == "binary_expression"
        && n.child_by_field_name("operator")
            .is_some_and(|o| texto(o, fonte) == "+");
    if soma {
        for lado in ["left", "right"] {
            if let Some(x) = n.child_by_field_name(lado) {
                folhas_de_soma(x, fonte, v);
            }
        }
    } else {
        v.push(n);
    }
}

pub fn sem_parenteses(mut n: Node) -> Node {
    while n.kind() == "parenthesized_expression" {
        match n.named_child(0) {
            Some(x) => n = x,
            None => break,
        }
    }
    n
}

/// String montada com parte variável: concatenação com algo que não é literal,
/// template do Kotlin ou `String.format`/`.formatted`/`.format` com argumentos.
/// Devolve o texto fixo (para saber se parece SQL, comando, URL...).
pub fn montada(n: Node, fonte: &str) -> Option<String> {
    let n = sem_parenteses(n);
    if interpolada(n, fonte) {
        return conteudo(n, fonte);
    }
    if n.kind() == "binary_expression" {
        let mut folhas = Vec::new();
        folhas_de_soma(n, fonte, &mut folhas);
        if folhas.len() < 2 {
            return None;
        }
        let fixos: Vec<String> = folhas.iter().filter_map(|f| literal(*f, fonte)).collect();
        let variavel = folhas.iter().any(|f| literal(*f, fonte).is_none());
        return (variavel && !fixos.is_empty()).then(|| fixos.concat());
    }
    let c = chamada(n, fonte)?;
    match c.nome {
        "format" if c.args.len() >= 2 => c.args.first().and_then(|a| literal(*a, fonte)),
        "formatted" | "format" if !c.args.is_empty() => c.receptor.and_then(|r| literal(r, fonte)),
        _ => None,
    }
}

static SQL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(select\s|insert\s+into|update\s+\S+\s+set|delete\s+from|\swhere\s|order\s+by|group\s+by|\sfrom\s+\S|values\s*\()").expect("regex")
});

pub fn parece_sql(t: &str) -> bool {
    SQL.is_match(t)
}

/// Divide um nome em palavras minúsculas (camelCase, snake_case, kebab-case, pontos).
pub fn palavras(nome: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut atual = String::new();
    let mut anterior_minuscula = false;
    for ch in nome.chars() {
        if ch == '_' || ch == '-' || ch == '.' || ch.is_whitespace() {
            if !atual.is_empty() {
                v.push(std::mem::take(&mut atual));
            }
            anterior_minuscula = false;
            continue;
        }
        if ch.is_uppercase() && anterior_minuscula && !atual.is_empty() {
            v.push(std::mem::take(&mut atual));
        }
        anterior_minuscula = ch.is_lowercase() || ch.is_ascii_digit();
        atual.extend(ch.to_lowercase());
    }
    if !atual.is_empty() {
        v.push(atual);
    }
    v
}

const SEGREDO: &[&str] = &[
    "password",
    "passwd",
    "pwd",
    "senha",
    "secret",
    "segredo",
    "apikey",
    "token",
    "credential",
    "credentials",
    "credencial",
    "credenciais",
];
/// Palavras que, no fim do nome, dizem que ele não guarda o segredo em si.
const NAO_E_SEGREDO: &[&str] = &[
    "expiration",
    "expiracao",
    "expira",
    "validity",
    "validade",
    "ttl",
    "header",
    "cabecalho",
    "name",
    "nome",
    "prefix",
    "prefixo",
    "length",
    "tamanho",
    "size",
    "url",
    "uri",
    "endpoint",
    "path",
    "caminho",
    "enabled",
    "type",
    "tipo",
    "field",
    "campo",
    "param",
    "parametro",
    "regex",
    "pattern",
    "padrao",
    "mask",
    "label",
    "message",
    "mensagem",
    "msg",
    "error",
    "erro",
    "invalid",
    "invalida",
    "invalido",
    "policy",
    "service",
    "repository",
    "provider",
    "manager",
    "handler",
    "util",
    "utils",
    "encoder",
    "validator",
    "filter",
    "config",
    "properties",
    "id",
    "count",
    "key",
    "dto",
    "request",
    "response",
    "requests",
];

/// Nome de variável, campo ou chave de configuração que guarda um segredo.
pub fn nome_de_segredo(nome: &str) -> bool {
    const CHAVES: &[&str] = &[
        "apikey",
        "privatekey",
        "clientsecret",
        "accesskey",
        "secretkey",
        "signingkey",
        "encryptionkey",
        "chavesecreta",
        "chaveprivada",
    ];
    let p = palavras(nome);
    let junto = p.concat();
    let tem =
        p.iter().any(|w| SEGREDO.contains(&w.as_str())) || CHAVES.iter().any(|k| junto.contains(k));
    let ultimo_ok = p
        .last()
        .is_some_and(|w| !NAO_E_SEGREDO.contains(&w.as_str()))
        || CHAVES.iter().any(|k| junto.ends_with(k));
    tem && ultimo_ok
}

static SEGREDO_NO_VALOR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"AKIA[0-9A-Z]{16}",
        r"|-----BEGIN ([A-Z]+ )?PRIVATE KEY-----",
        r"|gh[pousr]_[A-Za-z0-9]{36}",
        r"|github_pat_[A-Za-z0-9_]{40,}",
        r"|xox[abprs]-[A-Za-z0-9-]{10,}",
        r"|sk_live_[0-9A-Za-z]{20,}",
        r"|sk-(?:proj|ant)-[A-Za-z0-9_-]{20,}",
        r"|AIza[0-9A-Za-z_-]{35}",
        r"|eyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}",
        r"|(?:jdbc:)?(?:postgresql|postgres|mysql|mariadb|mongodb(?:\+srv)?|redis|rediss|amqp|amqps)://[^/\s:@${}<>]*:[^/\s@${}<>]{4,}@[^\s/]+",
    ))
    .expect("regex")
});

/// Formato de segredo conhecido (chave da AWS, chave privada, token do GitHub, JWT,
/// chave de API de IA, credencial em URL de banco/broker...).
pub fn formato_de_segredo(valor: &str) -> Option<&'static str> {
    let m = SEGREDO_NO_VALOR.find(valor)?;
    let s = m.as_str();
    Some(if s.starts_with("AKIA") {
        "chave de acesso da AWS"
    } else if s.starts_with("-----BEGIN") {
        "chave privada"
    } else if s.starts_with("gh") || s.starts_with("github_pat_") {
        "token do GitHub"
    } else if s.starts_with("xox") {
        "token do Slack"
    } else if s.starts_with("sk_live_") {
        "chave do Stripe"
    } else if s.starts_with("sk-proj-") || s.starts_with("sk-ant-") {
        "chave de API"
    } else if s.starts_with("AIza") {
        "chave do Google"
    } else if s.contains("://") {
        "credencial embutida em URL"
    } else {
        "JWT"
    })
}

/// Valor literal que pode ser um segredo de verdade (não vazio, sem espaço, não é
/// placeholder nem nome de propriedade).
pub fn valor_de_segredo(v: &str) -> bool {
    let v = v.trim();
    if v.len() < 4 || v.chars().any(char::is_whitespace) {
        return false;
    }
    if v.starts_with("${") || v.starts_with('<') || v.starts_with('{') || v.starts_with('%') {
        return false;
    }
    // Chave de propriedade (`jwt.secret`) ou rótulo (`API_KEY`), não o segredo.
    let rotulo = v
        .chars()
        .all(|c| c.is_ascii_lowercase() || c == '.' || c == '-' || c == '_')
        && v.contains(['.', '-']);
    let constante = v
        .chars()
        .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
        && v.contains('_');
    // Nome de chave de SharedPreferences, Bundle, formulário: `senha_key`, `password_field`.
    const ROTULOS: &[&str] = &[
        "key", "field", "campo", "param", "label", "name", "nome", "attr", "extra", "pref",
    ];
    let p = palavras(v);
    let nome_de_chave = v
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && p.iter().any(|w| SEGREDO.contains(&w.as_str()))
        && p.iter().any(|w| ROTULOS.contains(&w.as_str()));
    !(rotulo || constante || nome_de_chave)
}

/// A função mais próxima acima do nó (ou None no nível de classe).
pub fn funcao_dona(n: Node) -> Option<Node> {
    let mut p = n.parent();
    while let Some(x) = p {
        if eh_funcao(x.kind()) {
            return Some(x);
        }
        p = x.parent();
    }
    None
}

/// Nome de uma função ou método.
pub fn nome_da_funcao<'t>(f: Node<'t>, fonte: &'t str) -> &'t str {
    f.child_by_field_name("name")
        .map_or("", |n| texto(n, fonte))
}

/// Nomes das anotações de um nó de declaração (classe, método, campo, parâmetro).
pub fn anotacoes<'t>(n: Node<'t>, fonte: &'t str) -> Vec<(&'t str, Node<'t>)> {
    let mut v = Vec::new();
    let mut junta = |m: Node<'t>| {
        for a in filhos(m) {
            if !matches!(a.kind(), "marker_annotation" | "annotation") {
                continue;
            }
            let nome = match a.child_by_field_name("name") {
                Some(nm) => texto(nm, fonte),
                None => {
                    // Kotlin: @Nome ou @Nome(args).
                    let t = texto(a, fonte).trim_start_matches('@');
                    let fim = t.find(['(', ' ', '\n', '\r']).unwrap_or(t.len());
                    &t[..fim]
                }
            };
            let nome = nome.rsplit('.').next().unwrap_or(nome);
            let nome = nome.rsplit(':').next().unwrap_or(nome);
            v.push((nome, a));
        }
    };
    for c in filhos(n) {
        if matches!(c.kind(), "modifiers" | "parameter_modifiers") {
            junta(c);
        }
    }
    // Parâmetro do Kotlin: as anotações vêm no irmão anterior (`parameter_modifiers`).
    if n.kind() == "parameter"
        && let Some(p) = n
            .prev_named_sibling()
            .filter(|p| p.kind() == "parameter_modifiers")
    {
        junta(p);
    }
    v
}

/// Modificador de visibilidade/override presente (`private`, `override`...).
pub fn tem_modificador(n: Node, fonte: &str, m: &str) -> bool {
    filhos(n)
        .into_iter()
        .filter(|c| c.kind() == "modifiers")
        .any(|c| texto(c, fonte).split_whitespace().any(|w| w == m))
}

/// Identificadores usados dentro do nó (inclusive os de `$x` em templates do Kotlin).
pub fn identificadores<'t>(n: Node<'t>, fonte: &'t str) -> Vec<&'t str> {
    let mut v = Vec::new();
    crate::lang::visitar(n, |x| match x.kind() {
        "identifier" => v.push(texto(x, fonte)),
        "string_literal" => {
            let t = texto(x, fonte);
            for m in TEMPLATE_SIMPLES.captures_iter(t) {
                if let Some(g) = m.get(1) {
                    v.push(g.as_str());
                }
            }
        }
        _ => {}
    });
    v
}

static TEMPLATE_SIMPLES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").expect("regex"));

#[cfg(test)]
mod casos {
    use super::*;

    #[test]
    fn nomes() {
        for s in [
            "password",
            "senhaAdmin",
            "JWT_SECRET",
            "apiKey",
            "client-secret",
            "accessToken",
            "dbPassword",
        ] {
            assert!(nome_de_segredo(s), "{s}");
        }
        for s in [
            "tokenService",
            "passwordEncoder",
            "TOKEN_HEADER",
            "senhaInvalida",
            "tokenExpiration",
            "secretKeyName",
            "tokenId",
        ] {
            assert!(!nome_de_segredo(s), "{s}");
        }
        assert!(nome_de_segredo("SECRET_KEY"));
    }

    #[test]
    fn valores() {
        assert!(valor_de_segredo("minhaSenha123"));
        assert!(valor_de_segredo("senha123"));
        // Nome de chave de SharedPreferences/Bundle, não a senha (AppResturante, KEY_SENHA).
        assert!(!valor_de_segredo("senha_key"));
        assert!(!valor_de_segredo("password_field"));
        assert!(!valor_de_segredo("jwt.secret"));
        assert!(!valor_de_segredo("${JWT_SECRET}"));
        assert!(!valor_de_segredo("Senha inválida"));
        assert!(!valor_de_segredo("API_KEY"));
        assert_eq!(
            formato_de_segredo("x AKIAABCDEFGHIJKLMNOP y"),
            Some("chave de acesso da AWS")
        );
        assert_eq!(
            formato_de_segredo("jdbc:postgresql://admin:s3nh4Forte@db.interno:5432/app"),
            Some("credencial embutida em URL")
        );
        assert!(formato_de_segredo("jdbc:postgresql://admin:${DB_PASS}@db:5432/app").is_none());
        assert_eq!(
            formato_de_segredo("sk-ant-api03-1234567890abcdefghij"),
            Some("chave de API")
        );
    }

    #[test]
    fn sql() {
        assert!(parece_sql("SELECT * FROM t WHERE id = "));
        assert!(parece_sql(" and nome = ' order by x"));
        assert!(!parece_sql("Usuário não encontrado"));
    }
}
