//! Linguagens, parser (tree-sitter) e utilitários sobre a árvore sintática.

use tree_sitter::{Language, Node, Parser, Tree};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Linguagem {
    Java,
    Kotlin,
}

impl Linguagem {
    pub fn do_caminho(caminho: &str) -> Option<Self> {
        let c = caminho.to_ascii_lowercase();
        if c.ends_with(".java") {
            Some(Self::Java)
        } else if c.ends_with(".kt") {
            Some(Self::Kotlin)
        } else {
            None
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Self::Java => "java",
            Self::Kotlin => "kotlin",
        }
    }

    pub fn gramatica(self) -> Language {
        match self {
            Self::Java => tree_sitter_java::LANGUAGE.into(),
            Self::Kotlin => tree_sitter_kotlin_ng::LANGUAGE.into(),
        }
    }
}

/// Um parser por linguagem. Não é `Sync`: cada thread do rayon cria o seu.
pub struct Parsers {
    java: Parser,
    kotlin: Parser,
}

impl Parsers {
    pub fn novo() -> Self {
        let criar = |l: Linguagem| {
            let mut p = Parser::new();
            p.set_language(&l.gramatica())
                .expect("gramática incompatível com a versão do tree-sitter");
            p
        };
        Self {
            java: criar(Linguagem::Java),
            kotlin: criar(Linguagem::Kotlin),
        }
    }

    pub fn ler(&mut self, l: Linguagem, fonte: &str) -> Option<Tree> {
        let p = match l {
            Linguagem::Java => &mut self.java,
            Linguagem::Kotlin => &mut self.kotlin,
        };
        p.parse(fonte, None)
    }
}

pub fn texto<'a>(n: Node, fonte: &'a str) -> &'a str {
    fonte.get(n.byte_range()).unwrap_or("")
}

/// Linha 1-based do início do nó.
pub fn linha(n: Node) -> usize {
    n.start_position().row + 1
}

/// Percorre a árvore em pré-ordem (ordem do documento), sem recursão.
pub fn visitar<'t>(raiz: Node<'t>, mut f: impl FnMut(Node<'t>)) {
    let mut c = raiz.walk();
    'fora: loop {
        f(c.node());
        if c.goto_first_child() {
            continue;
        }
        loop {
            if c.goto_next_sibling() {
                continue 'fora;
            }
            if !c.goto_parent() {
                break 'fora;
            }
        }
    }
}

pub fn filhos<'t>(n: Node<'t>) -> Vec<Node<'t>> {
    let mut c = n.walk();
    n.children(&mut c).collect()
}

pub fn eh_comentario(kind: &str) -> bool {
    matches!(kind, "line_comment" | "block_comment")
}

/// Declarações de função/método/construtor das duas linguagens.
pub fn eh_funcao(kind: &str) -> bool {
    matches!(
        kind,
        "method_declaration"
            | "constructor_declaration"
            | "compact_constructor_declaration"
            | "function_declaration"
            | "secondary_constructor"
    )
}

pub fn eh_lambda(kind: &str) -> bool {
    matches!(
        kind,
        "lambda_expression" | "lambda_literal" | "anonymous_function"
    )
}

/// Corpo da função, se ela tiver um (métodos abstratos e de interface não têm).
pub fn corpo_da_funcao(f: Node) -> Option<Node> {
    if let Some(b) = f.child_by_field_name("body") {
        return Some(b);
    }
    filhos(f)
        .into_iter()
        .find(|c| matches!(c.kind(), "function_body" | "block"))
}

/// Nome do símbolo dono do nó: `Classe.metodo`, do mais externo para o mais interno.
pub fn simbolo_dono(n: Node, fonte: &str) -> String {
    let mut partes = Vec::new();
    let mut atual = Some(n);
    while let Some(x) = atual {
        match x.kind() {
            "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "annotation_type_declaration"
            | "object_declaration"
            | "method_declaration"
            | "constructor_declaration"
            | "compact_constructor_declaration"
            | "function_declaration" => {
                if let Some(nome) = x.child_by_field_name("name") {
                    partes.push(texto(nome, fonte).to_string());
                }
            }
            "secondary_constructor" => partes.push("constructor".into()),
            "companion_object" => partes.push("Companion".into()),
            _ => {}
        }
        atual = x.parent();
    }
    partes.reverse();
    partes.join(".")
}

/// Pacote declarado no arquivo.
pub fn pacote(raiz: Node, fonte: &str) -> Option<String> {
    for c in filhos(raiz) {
        if matches!(c.kind(), "package_declaration" | "package_header") {
            let t = texto(c, fonte);
            let t = t.trim().trim_start_matches("package").trim_end_matches(';');
            // Java permite anotações antes do nome do pacote; o nome é o último token.
            let nome: String = t
                .split_whitespace()
                .last()
                .unwrap_or("")
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .collect();
            if !nome.is_empty() {
                return Some(nome);
            }
        }
    }
    None
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Import {
    /// Pacote do que foi importado (`a.b` em `import a.b.C`).
    pub pacote: String,
    /// Tipo ou membro importado (`C`), ou `*`.
    pub nome: String,
    pub linha: usize,
    /// Texto normalizado do import (para a impressão digital do baseline).
    pub texto: String,
}

/// `import a.b.C` → (`a.b`, `C`); `import a.b.*` → (`a.b`, `*`);
/// `import static a.b.C.m` → (`a.b`, `C`); `import a.b.funcao` (Kotlin) → (`a.b`, `funcao`).
pub fn resolver_import(caminho: &str, curinga: bool) -> (String, String) {
    let segs: Vec<&str> = caminho.split('.').filter(|s| !s.is_empty()).collect();
    let maiuscula = segs
        .iter()
        .position(|s| s.chars().next().is_some_and(|c| c.is_uppercase()));
    match (maiuscula, curinga) {
        (Some(i), _) => (segs[..i].join("."), segs[i].to_string()),
        (None, true) => (segs.join("."), "*".to_string()),
        (None, false) if segs.len() > 1 => (
            segs[..segs.len() - 1].join("."),
            segs[segs.len() - 1].to_string(),
        ),
        (None, false) => (String::new(), caminho.to_string()),
    }
}

/// Normaliza o texto de um import das duas linguagens: tira `import`, `static`, alias e `;`.
pub fn caminho_do_import(t: &str) -> (String, bool) {
    let mut s = t.trim();
    s = s.strip_prefix("import").unwrap_or(s).trim();
    s = s.strip_prefix("static ").unwrap_or(s).trim();
    if let Some(i) = s.find(" as ") {
        s = &s[..i];
    }
    let s: String = s
        .trim_end_matches(';')
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    match s.strip_suffix(".*") {
        Some(base) => (base.to_string(), true),
        None => (s, false),
    }
}

pub fn imports(raiz: Node, fonte: &str) -> Vec<Import> {
    let mut v = Vec::new();
    for c in filhos(raiz) {
        if matches!(c.kind(), "import_declaration" | "import") {
            let t = texto(c, fonte);
            let (caminho, curinga) = caminho_do_import(t);
            let (pacote, nome) = resolver_import(&caminho, curinga);
            v.push(Import {
                pacote,
                nome,
                linha: linha(c),
                texto: t.split_whitespace().collect::<Vec<_>>().join(" "),
            });
        }
    }
    v
}

/// Anotações das declarações de tipo do arquivo: (nome simples, linha do nome do tipo).
/// Usadas pelas regras de arquitetura (ex.: @RestController fora da camada de entrada).
pub fn anotacoes_de_tipo(raiz: Node, fonte: &str) -> Vec<(String, usize)> {
    let mut v = Vec::new();
    visitar(raiz, |n| {
        if !matches!(
            n.kind(),
            "class_declaration"
                | "interface_declaration"
                | "enum_declaration"
                | "record_declaration"
                | "object_declaration"
        ) {
            return;
        }
        let Some(modificadores) = filhos(n).into_iter().find(|c| c.kind() == "modifiers") else {
            return;
        };
        let l = linha(n.child_by_field_name("name").unwrap_or(n));
        for a in filhos(modificadores) {
            let nome = match a.kind() {
                "marker_annotation" | "annotation" => match a.child_by_field_name("name") {
                    Some(nm) => texto(nm, fonte),
                    // Kotlin: @Nome ou @Nome(args), sem campo "name".
                    None => texto(a, fonte).trim_start_matches('@'),
                },
                _ => continue,
            };
            let nome = nome.split('(').next().unwrap_or(nome).trim();
            let nome = nome.rsplit('.').next().unwrap_or(nome);
            let nome = nome.rsplit(':').next().unwrap_or(nome);
            v.push((nome.to_string(), l));
        }
    });
    v
}

/// Parâmetro recebido no corpo da requisição (`@RequestBody`): tipos citados na declaração.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CorpoRequisicao {
    pub tipos: Vec<String>,
    pub linha: usize,
    pub simbolo: String,
}

/// Classes anotadas com @Entity e parâmetros @RequestBody do arquivo (para a SPR004).
pub fn entidades_e_corpos(raiz: Node, fonte: &str) -> (Vec<String>, Vec<CorpoRequisicao>) {
    use crate::regras::sintaxe::anotacoes;
    let mut entidades = Vec::new();
    let mut corpos = Vec::new();
    visitar(raiz, |n| match n.kind() {
        "class_declaration" if anotacoes(n, fonte).iter().any(|(a, _)| *a == "Entity") => {
            if let Some(nome) = n.child_by_field_name("name") {
                entidades.push(texto(nome, fonte).to_string());
            }
        }
        "formal_parameter" | "parameter" => {
            if !anotacoes(n, fonte).iter().any(|(a, _)| *a == "RequestBody") {
                return;
            }
            let tipo = n.child_by_field_name("type").or_else(|| {
                filhos(n)
                    .into_iter()
                    .find(|c| matches!(c.kind(), "user_type" | "nullable_type"))
            });
            let Some(tipo) = tipo else { return };
            let mut tipos = Vec::new();
            visitar(tipo, |x| {
                if matches!(x.kind(), "type_identifier" | "identifier") {
                    tipos.push(texto(x, fonte).to_string());
                }
            });
            corpos.push(CorpoRequisicao {
                tipos,
                linha: linha(n),
                simbolo: simbolo_dono(n, fonte),
            });
        }
        _ => {}
    });
    (entidades, corpos)
}

/// Exceção instanciada no arquivo (`new XException(..)`, `XException(..)` no Kotlin), lançada
/// ou não (ex.: `orElseThrow(() -> new X())`). Usada pelas regras de negócio (NEG001/002).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExcecaoCriada {
    /// Nome simples do tipo.
    pub tipo: String,
    pub linha: usize,
    pub simbolo: String,
    pub trecho: String,
}

pub fn excecoes_criadas(raiz: Node, fonte: &str) -> Vec<ExcecaoCriada> {
    let mut v = Vec::new();
    visitar(raiz, |n| {
        let tipo = match n.kind() {
            "object_creation_expression" => n.child_by_field_name("type").map(|t| texto(t, fonte)),
            "call_expression" => n
                .named_child(0)
                .filter(|c| c.kind() == "identifier")
                .map(|c| texto(c, fonte)),
            _ => None,
        };
        let Some(tipo) = tipo else { return };
        let tipo = tipo.split('<').next().unwrap_or(tipo).trim();
        let tipo = tipo.rsplit('.').next().unwrap_or(tipo);
        if !tipo.ends_with("Exception") || !tipo.starts_with(|c: char| c.is_ascii_uppercase()) {
            return;
        }
        v.push(ExcecaoCriada {
            tipo: tipo.to_string(),
            linha: linha(n),
            simbolo: simbolo_dono(n, fonte),
            trecho: texto(n, fonte)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        });
    });
    v
}

/// Para cada linha (índice 0-based), diz se ela tem código, ou seja, se não é em branco
/// nem só comentário. Usada na contagem de linhas de função (CPX005).
pub fn linhas_de_codigo(fonte: &str, raiz: Node) -> Vec<bool> {
    let mut comentarios: Vec<(usize, usize)> = Vec::new();
    visitar(raiz, |n| {
        if eh_comentario(n.kind()) {
            comentarios.push((n.start_byte(), n.end_byte()));
        }
    });
    comentarios.sort_unstable();
    let em_comentario = |off: usize| {
        let i = comentarios.partition_point(|&(ini, _)| ini <= off);
        i > 0 && off < comentarios[i - 1].1
    };
    let mut res = Vec::new();
    let mut inicio = 0usize;
    for linha in fonte.split('\n') {
        let bytes = linha.as_bytes();
        let primeiro = bytes.iter().position(|b| !b.is_ascii_whitespace());
        let ultimo = bytes.iter().rposition(|b| !b.is_ascii_whitespace());
        let tem_codigo = match (primeiro, ultimo) {
            (Some(p), Some(u)) => !(em_comentario(inicio + p) && em_comentario(inicio + u)),
            _ => false,
        };
        res.push(tem_codigo);
        inicio += linha.len() + 1;
    }
    res
}

/// Imprime a árvore (para depurar regras): `tipo [linha] "texto curto"`.
pub fn despejar(raiz: Node, fonte: &str) -> String {
    let mut s = String::new();
    fn rec(n: Node, fonte: &str, nivel: usize, s: &mut String) {
        let t = texto(n, fonte);
        let curto: String = t.chars().take(40).collect::<String>().replace('\n', "⏎");
        let campo = n
            .parent()
            .and_then(|p| {
                let mut c = p.walk();
                let idx = p.children(&mut c).position(|x| x == n)?;
                p.field_name_for_child(idx as u32)
            })
            .map(|f| format!("{f}: "))
            .unwrap_or_default();
        let marca = if n.is_named() { "" } else { "'" };
        s.push_str(&format!(
            "{}{}{}{}{} [{}] {:?}\n",
            "  ".repeat(nivel),
            campo,
            marca,
            n.kind(),
            marca,
            linha(n),
            curto
        ));
        for c in filhos(n) {
            rec(c, fonte, nivel + 1, s);
        }
    }
    rec(raiz, fonte, 0, &mut s);
    s
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn resolve_imports() {
        assert_eq!(resolver_import("a.b.C", false), ("a.b".into(), "C".into()));
        assert_eq!(resolver_import("a.b", true), ("a.b".into(), "*".into()));
        assert_eq!(
            resolver_import("a.b.C.m", false),
            ("a.b".into(), "C".into())
        );
        assert_eq!(
            resolver_import("a.b.funcao", false),
            ("a.b".into(), "funcao".into())
        );
        assert_eq!(
            caminho_do_import("import static a.b.C.*;"),
            ("a.b.C".into(), true)
        );
        assert_eq!(
            caminho_do_import("import a.b.C as D"),
            ("a.b.C".into(), false)
        );
    }
}
