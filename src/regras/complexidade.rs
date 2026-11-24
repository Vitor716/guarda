//! CPX001 (complexidade cognitiva, Sonar S3776) e CPX005 (função longa, detekt LongMethod).
//!
//! Complexidade cognitiva segundo o white paper do Sonar (G. Ann Campbell, 2017):
//! - +1 e +nível de aninhamento: if, ternário, switch/when, laços, catch;
//! - +1 sem aninhamento: else if, else, cada sequência de && ou ||, break/continue com rótulo,
//!   recursão direta;
//! - aumentam o aninhamento sem somar: lambdas, classes anônimas e funções aninhadas.

use crate::lang::{Linguagem, corpo_da_funcao, filhos, texto};
use tree_sitter::Node;

pub fn cognitiva(funcao: Node, fonte: &str, ling: Linguagem) -> u32 {
    let nome = funcao
        .child_by_field_name("name")
        .map(|n| texto(n, fonte))
        .unwrap_or("");
    let mut c = Cog {
        fonte,
        nome,
        ling,
        total: 0,
    };
    if let Some(corpo) = corpo_da_funcao(funcao) {
        c.no(corpo, 0);
    }
    c.total
}

struct Cog<'a> {
    fonte: &'a str,
    nome: &'a str,
    ling: Linguagem,
    total: u32,
}

impl Cog<'_> {
    fn filhos(&mut self, n: Node, nivel: u32) {
        for f in filhos(n) {
            self.no(f, nivel);
        }
    }

    fn estrutura(&mut self, n: Node, nivel: u32) {
        self.total += 1 + nivel;
        self.filhos(n, nivel + 1);
    }

    fn no(&mut self, n: Node, nivel: u32) {
        match self.ling {
            Linguagem::Java => self.java(n, nivel),
            Linguagem::Kotlin => self.kotlin(n, nivel),
        }
    }

    fn java(&mut self, n: Node, nivel: u32) {
        match n.kind() {
            "if_statement" => self.java_if(n, nivel, false),
            "ternary_expression"
            | "for_statement"
            | "enhanced_for_statement"
            | "while_statement"
            | "do_statement"
            | "catch_clause" => self.estrutura(n, nivel),
            "switch_expression" => {
                self.total += 1 + nivel;
                if let Some(c) = n.child_by_field_name("condition") {
                    self.no(c, nivel);
                }
                if let Some(b) = n.child_by_field_name("body") {
                    self.filhos(b, nivel + 1);
                }
            }
            "lambda_expression" | "class_body" => self.filhos(n, nivel + 1),
            "binary_expression" => {
                self.logico(n);
                self.filhos(n, nivel);
            }
            "break_statement" | "continue_statement" => {
                if n.named_child_count() > 0 {
                    self.total += 1;
                }
            }
            "method_invocation" => {
                let recursiva = n
                    .child_by_field_name("name")
                    .is_some_and(|m| texto(m, self.fonte) == self.nome)
                    && n.child_by_field_name("object")
                        .is_none_or(|o| o.kind() == "this");
                if recursiva {
                    self.total += 1;
                }
                self.filhos(n, nivel);
            }
            _ => self.filhos(n, nivel),
        }
    }

    fn java_if(&mut self, n: Node, nivel: u32, else_if: bool) {
        self.total += if else_if { 1 } else { 1 + nivel };
        if let Some(c) = n.child_by_field_name("condition") {
            self.no(c, nivel);
        }
        if let Some(c) = n.child_by_field_name("consequence") {
            self.no(c, nivel + 1);
        }
        if let Some(alt) = n.child_by_field_name("alternative") {
            if alt.kind() == "if_statement" {
                self.java_if(alt, nivel, true);
            } else {
                self.total += 1;
                self.no(alt, nivel + 1);
            }
        }
    }

    fn kotlin(&mut self, n: Node, nivel: u32) {
        match n.kind() {
            "if_expression" => self.kotlin_if(n, nivel, false),
            "when_expression" | "for_statement" | "while_statement" | "do_while_statement"
            | "catch_block" => self.estrutura(n, nivel),
            "lambda_literal"
            | "anonymous_function"
            | "object_literal"
            | "class_body"
            | "function_declaration" => self.filhos(n, nivel + 1),
            "binary_expression" => {
                self.logico(n);
                self.filhos(n, nivel);
            }
            "labeled_expression" => {
                // `break@rotulo` / `continue@rotulo` viram labeled_expression com label "break@".
                let salto = n.named_child(0).is_some_and(|l| {
                    l.kind() == "label" && {
                        let t = texto(l, self.fonte);
                        t.starts_with("break@") || t.starts_with("continue@")
                    }
                });
                if salto {
                    self.total += 1;
                }
                self.filhos(n, nivel);
            }
            "call_expression" => {
                let recursiva = n
                    .named_child(0)
                    .is_some_and(|c| c.kind() == "identifier" && texto(c, self.fonte) == self.nome);
                if recursiva {
                    self.total += 1;
                }
                self.filhos(n, nivel);
            }
            _ => self.filhos(n, nivel),
        }
    }

    fn kotlin_if(&mut self, n: Node, nivel: u32, else_if: bool) {
        self.total += if else_if { 1 } else { 1 + nivel };
        let cond = n.child_by_field_name("condition");
        if let Some(c) = cond {
            self.no(c, nivel);
        }
        let mut depois_do_else = false;
        for f in filhos(n) {
            if f.kind() == "else" {
                depois_do_else = true;
                continue;
            }
            if !f.is_named() || Some(f) == cond {
                continue;
            }
            if !depois_do_else {
                self.no(f, nivel + 1);
            } else if f.kind() == "if_expression" {
                self.kotlin_if(f, nivel, true);
            } else {
                self.total += 1;
                self.no(f, nivel + 1);
            }
        }
    }

    /// +1 por sequência de operadores lógicos iguais: `a && b && c` = 1, `a && b || c` = 2.
    fn logico(&mut self, n: Node) {
        fn op<'t>(x: Node<'t>) -> Option<&'t str> {
            x.child_by_field_name("operator").map(|o| o.kind())
        }
        let Some(meu) = op(n) else { return };
        if meu != "&&" && meu != "||" {
            return;
        }
        let continua = n
            .parent()
            .is_some_and(|p| p.kind() == "binary_expression" && op(p) == Some(meu));
        if !continua {
            self.total += 1;
        }
    }
}

/// Linhas de código da função (sem brancos nem comentários), do nome até o fim.
pub fn linhas(funcao: Node, codigo: &[bool]) -> u32 {
    let ini = funcao
        .child_by_field_name("name")
        .unwrap_or(funcao)
        .start_position()
        .row;
    let fim = funcao.end_position().row;
    (ini..=fim)
        .filter(|&r| codigo.get(r).copied().unwrap_or(false))
        .count() as u32
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::lang::{Parsers, eh_funcao, visitar};

    /// Complexidade da primeira função do trecho.
    fn cc(ling: Linguagem, fonte: &str) -> u32 {
        let mut p = Parsers::novo();
        let t = p.ler(ling, fonte).unwrap();
        assert!(!t.root_node().has_error(), "trecho com erro de sintaxe");
        let mut f = None;
        visitar(t.root_node(), |n| {
            if f.is_none() && eh_funcao(n.kind()) {
                f = Some(n);
            }
        });
        cognitiva(f.unwrap(), fonte, ling)
    }

    fn java(corpo: &str) -> u32 {
        cc(Linguagem::Java, &format!("class A {{ {corpo} }}"))
    }

    // Exemplos do white paper "Cognitive Complexity" (SonarSource).
    #[test]
    fn sonar_soma_de_primos() {
        let f = "int sumOfPrimes(int max) {
            int total = 0;
            OUT: for (int i = 1; i <= max; ++i) {
                for (int j = 2; j < i; ++j) {
                    if (i % j == 0) { continue OUT; }
                }
                total += i;
            }
            return total;
        }";
        assert_eq!(java(f), 7);
    }

    #[test]
    fn sonar_switch() {
        let f = r#"String getWords(int number) {
            switch (number) {
                case 1: return "one";
                case 2: return "a couple";
                default: return "lots";
            }
        }"#;
        assert_eq!(java(f), 1);
    }

    #[test]
    fn sonar_try_catch_aninhado() {
        let f = "void myMethod() {
            try {
                if (condition1) {
                    for (int i = 0; i < 10; i++) {
                        while (condition2) { }
                    }
                }
            } catch (ExcepType1 | ExcepType2 e) {
                if (condition2) { }
            }
        }";
        assert_eq!(java(f), 9);
    }

    #[test]
    fn sequencias_de_operadores_logicos() {
        assert_eq!(java("void m() { if (a && b && c || d || e && f) {} }"), 4);
    }

    #[test]
    fn else_if_e_else_nao_somam_aninhamento() {
        // for 1, if 2 (aninhado), else if 1, else 1
        assert_eq!(
            java("void m() { for (;;) { if (a) {} else if (b) {} else {} } }"),
            5
        );
    }

    #[test]
    fn lambda_aumenta_aninhamento() {
        assert_eq!(java("void m() { lista.forEach(x -> { if (x) {} }); }"), 2);
    }

    #[test]
    fn recursao() {
        // ternário 1 + duas chamadas recursivas
        assert_eq!(
            java("int f(int n) { return n < 2 ? n : f(n - 1) + this.f(n - 2); }"),
            3
        );
    }

    #[test]
    fn kotlin_rotulos_else_if_e_recursao() {
        let f = "fun f(a: Int?, b: Boolean, c: Boolean): Int {
            externo@ for (i in 1..10) {
                if (a!! > i && b || c) {
                    continue@externo
                } else if (b) {
                    break@externo
                } else {
                    return f(a, b, c)
                }
            }
            return 0
        }";
        // for 1 + if 2 + && 1 + || 1 + continue@ 1 + else if 1 + break@ 1 + else 1 + recursão 1
        assert_eq!(cc(Linguagem::Kotlin, f), 10);
    }

    #[test]
    fn kotlin_when_aninhado() {
        let f = "fun g(x: Int, a: Boolean) { if (a) { when (x) { 1 -> println(1)\n else -> println(2) } } }";
        assert_eq!(cc(Linguagem::Kotlin, f), 3);
    }

    #[test]
    fn kotlin_if_como_expressao() {
        assert_eq!(
            cc(Linguagem::Kotlin, "fun h(a: Boolean) = if (a) 1 else 2"),
            2
        );
    }

    #[test]
    fn linhas_ignoram_branco_e_comentario() {
        let fonte = "class A {\n  void m() {\n    // nota\n\n    int x = 1; /* fim */\n    /*\n     * bloco\n     */\n  }\n}\n";
        let mut p = Parsers::novo();
        let t = p.ler(Linguagem::Java, fonte).unwrap();
        let codigo = crate::lang::linhas_de_codigo(fonte, t.root_node());
        let mut f = None;
        visitar(t.root_node(), |n| {
            if eh_funcao(n.kind()) {
                f = Some(n)
            }
        });
        // "void m() {", "int x = 1;", "}"
        assert_eq!(linhas(f.unwrap(), &codigo), 3);
    }
}
