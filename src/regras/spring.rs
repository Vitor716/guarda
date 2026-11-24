//! Spring: SPR001 injeção por campo (Sonar S6813), SPR002 anotação de proxy em método
//! private (Sonar S2230), SPR003 chamada interna a método de proxy (Sonar S6809),
//! SPR005 @RequestMapping sem método HTTP (Sonar S3752). A SPR004 (entidade JPA no
//! @RequestBody, Sonar S4684) precisa do projeto todo e roda no motor.

use super::Contexto;
use super::sintaxe::{anotacoes, chamada, nome_da_funcao, receptor_txt, tem_modificador};
use crate::lang::{eh_funcao, filhos, linha, simbolo_dono, texto, visitar};
use crate::modelo::Violacao;
use std::collections::{HashMap, HashSet};
use tree_sitter::Node;

/// Anotações que só funcionam quando a chamada passa pelo proxy do Spring.
const PROXY: &[&str] = &[
    "Async",
    "CacheEvict",
    "CachePut",
    "Cacheable",
    "PostAuthorize",
    "PreAuthorize",
    "Retryable",
    "Secured",
    "Transactional",
];

fn add(
    ctx: &Contexto,
    v: &mut Vec<Violacao>,
    codigo: &'static str,
    n: Node,
    msg: String,
    dica: &str,
    trecho: &str,
) {
    v.push(Violacao::nova(
        codigo,
        ctx.arquivo,
        linha(n),
        msg,
        dica,
        simbolo_dono(n, ctx.fonte),
        trecho,
        None,
    ));
}

fn privado(n: Node, fonte: &str) -> bool {
    tem_modificador(n, fonte, "private")
}

fn eh_classe(k: &str) -> bool {
    matches!(
        k,
        "class_declaration" | "object_declaration" | "enum_declaration" | "record_declaration"
    )
}

/// Métodos declarados direto no corpo da classe.
fn metodos<'t>(classe: Node<'t>) -> Vec<Node<'t>> {
    let corpo = classe.child_by_field_name("body").or_else(|| {
        filhos(classe)
            .into_iter()
            .find(|c| c.kind() == "class_body")
    });
    corpo.map_or_else(Vec::new, |c| {
        filhos(c)
            .into_iter()
            .filter(|m| eh_funcao(m.kind()))
            .collect()
    })
}

fn proxies<'t>(m: Node<'t>, fonte: &'t str) -> Vec<&'t str> {
    anotacoes(m, fonte)
        .into_iter()
        .map(|(a, _)| a)
        .filter(|a| PROXY.contains(a))
        .collect()
}

/// SPR003: método da classe chama (sem passar pelo proxy) outro com @Transactional/@Async...
fn auto_chamada(ctx: &Contexto, classe: Node, v: &mut Vec<Violacao>) {
    let f = ctx.fonte;
    let ms = metodos(classe);
    let alvos: HashMap<&str, Vec<&str>> = ms
        .iter()
        .filter_map(|m| {
            let p = proxies(*m, f);
            (!p.is_empty()).then(|| (nome_da_funcao(*m, f), p))
        })
        .collect();
    if alvos.is_empty() {
        return;
    }
    for m in &ms {
        let proprio = nome_da_funcao(*m, f);
        let do_chamador: HashSet<&str> = proxies(*m, f).into_iter().collect();
        visitar(*m, |n| {
            let Some(c) = chamada(n, f) else { return };
            let r = receptor_txt(&c, f);
            if !(r.is_empty() || r == "this") || c.nome == proprio || c.construtor {
                return;
            }
            let Some(anots) = alvos.get(c.nome) else {
                return;
            };
            // Se quem chama já tem a mesma anotação, o efeito (transação aberta) já vale.
            let perdidas: Vec<&str> = anots
                .iter()
                .filter(|a| !do_chamador.contains(*a))
                .copied()
                .collect();
            if let Some(a) = perdidas.first() {
                add(
                    ctx,
                    v,
                    "SPR003",
                    n,
                    format!("chamada interna a {}() ignora o @{a}", c.nome),
                    "a chamada pelo this não passa pelo proxy: mova o método para outro bean ou anote quem chama",
                    c.nome,
                );
            }
        });
    }
}

fn eh_controlador(classe: Node, fonte: &str) -> bool {
    anotacoes(classe, fonte)
        .iter()
        .any(|(a, _)| matches!(*a, "RestController" | "Controller"))
}

/// SPR006: @Transactional na classe ou em métodos de um controlador HTTP.
fn transacional_em_controlador(ctx: &Contexto, classe: Node, v: &mut Vec<Violacao>) {
    let f = ctx.fonte;
    if !eh_controlador(classe, f) {
        return;
    }
    let msg = "@Transactional em controlador HTTP";
    let dica = "mova @Transactional para o serviço/caso de uso; transação na camada web segura conexão durante o HTTP";
    if let Some((_, no)) = anotacoes(classe, f)
        .into_iter()
        .find(|(a, _)| *a == "Transactional")
    {
        add(ctx, v, "SPR006", no, msg.into(), dica, "Transactional");
    }
    for m in metodos(classe) {
        if let Some((_, no)) = anotacoes(m, f)
            .into_iter()
            .find(|(a, _)| *a == "Transactional")
        {
            add(ctx, v, "SPR006", no, msg.into(), dica, "Transactional");
        }
    }
}

pub fn verificar(ctx: &Contexto, v: &mut Vec<Violacao>) {
    let f = ctx.fonte;
    if !f.contains('@') {
        return;
    }
    visitar(ctx.raiz, |n| {
        let k = n.kind();
        if eh_classe(k) {
            auto_chamada(ctx, n, v);
            transacional_em_controlador(ctx, n, v);
            return;
        }
        let anots = || anotacoes(n, f);
        match k {
            "field_declaration" | "property_declaration" => {
                if let Some((a, no)) = anots()
                    .into_iter()
                    .find(|(a, _)| matches!(*a, "Autowired" | "Inject"))
                {
                    add(
                        ctx,
                        v,
                        "SPR001",
                        no,
                        format!("injeção por campo (@{a})"),
                        "receba a dependência pelo construtor (campo final/val)",
                        a,
                    );
                }
            }
            _ if eh_funcao(k) => {
                let lista = anots();
                if privado(n, f)
                    && let Some((a, no)) = lista.iter().find(|(a, _)| PROXY.contains(a))
                {
                    add(
                        ctx,
                        v,
                        "SPR002",
                        *no,
                        format!("@{a} em método private não tem efeito"),
                        "o proxy do Spring só intercepta método público chamado de fora do bean",
                        a,
                    );
                }
                if let Some((_, no)) = lista.iter().find(|(a, _)| *a == "RequestMapping")
                    && !texto(*no, f).contains("method")
                {
                    add(
                        ctx,
                        v,
                        "SPR005",
                        *no,
                        "@RequestMapping sem método HTTP".into(),
                        "use @GetMapping, @PostMapping... (ou method = RequestMethod.X)",
                        "RequestMapping",
                    );
                }
            }
            _ => {}
        }
    });
}

#[cfg(test)]
mod casos {
    use super::*;
    use crate::config::Limites;
    use crate::lang::{Linguagem, Parsers};

    fn achar(fonte: &str, ling: Linguagem) -> Vec<(String, usize)> {
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
        let mut r: Vec<_> = v
            .into_iter()
            .map(|x| (x.codigo.to_string(), x.linha))
            .collect();
        r.sort();
        r
    }

    #[test]
    fn java() {
        let j = r#"@Service
class Pedidos {
  @Autowired private Repo repo;
  public void fechar() { salvar(); this.avisar(); }
  @Transactional public void salvar() { }
  @Async void avisar() { }
  @Transactional private void interno() { }
  @Transactional public void lote() { salvar(); }
  @RequestMapping("/x") public String x() { return "x"; }
  @RequestMapping(value = "/y", method = RequestMethod.GET) public String y() { return "y"; }
}"#;
        assert_eq!(
            achar(j, Linguagem::Java),
            [
                ("SPR001".to_string(), 3),
                ("SPR002".into(), 7),
                ("SPR003".into(), 4),
                ("SPR003".into(), 4),
                ("SPR005".into(), 9),
            ]
        );
    }

    #[test]
    fn kotlin() {
        let k = "@Service\nclass Pedidos {\n    @Autowired\n    lateinit var repo: Repo\n    @Transactional\n    private fun interno() {}\n}";
        assert_eq!(
            achar(k, Linguagem::Kotlin),
            [("SPR001".to_string(), 3), ("SPR002".into(), 5)]
        );
    }

    #[test]
    fn transacional_em_controller() {
        let j = "@RestController\nclass PedidoController {\n  @PostMapping\n  @Transactional\n  public void criar() {}\n}\n@Controller\n@Transactional\nclass OutroController {}";
        assert_eq!(
            achar(j, Linguagem::Java),
            [("SPR006".to_string(), 4), ("SPR006".into(), 8)]
        );
    }
}
