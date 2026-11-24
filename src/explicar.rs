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
        "{} {}\n\nPor quê: {}\n\nErrado:\n{}\n\nCerto:\n{}\n\nSupressão pontual (com motivo): // guarda:ignorar {} motivo: ...\n",
        e.codigo,
        e.titulo,
        e.porque,
        recuar(e.errado),
        recuar(e.certo),
        e.codigo
    ))
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
