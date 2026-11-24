//! guarda: linter determinístico para Kotlin e Java, com saída enxuta para IA.

mod arquitetura;
mod baseline;
mod cache;
mod config;
mod descoberta;
mod gate;
mod hook;
mod integracao;
mod lang;
mod modelo;
mod motor;
mod regras;
mod saida;

use anyhow::{Context, Result, bail};
use baseline::Baseline;
use clap::{Parser, Subcommand, ValueEnum};
use config::Config;
use gate::{Opcoes, Pedido};
use motor::{Escopo, Origem, Relatorio};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "guarda",
    version,
    about = "Linter determinístico para Kotlin e Java, com saída para IA"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Verifica o projeto e aplica o gate (sai com 1 se houver violação nova)
    Check(CheckArgs),
    /// Detecta a estrutura, escreve o guarda.yml e congela o legado no baseline
    Init {
        #[arg(default_value = ".")]
        raiz: PathBuf,
        /// Sobrescreve um guarda.yml existente
        #[arg(long)]
        forcar: bool,
        /// Também integra com o agente (contexto no CLAUDE.md/AGENTS.md e hooks)
        #[arg(long, value_enum)]
        ia: Option<AgenteArg>,
    },
    /// Baseline do legado
    Baseline {
        #[command(subcommand)]
        acao: BaselineCmd,
    },
    /// Mede parse, tempo e violações por regra (portão da Fase 0)
    Medir {
        #[arg(default_value = ".")]
        raiz: PathBuf,
    },
    /// Imprime as regras do projeto em poucas linhas, para o prompt da IA
    Contexto {
        #[arg(default_value = ".")]
        raiz: PathBuf,
    },
    /// Escreve o contexto no CLAUDE.md (e os hooks no .claude/settings.json) ou no AGENTS.md
    Integrar {
        #[arg(value_enum)]
        agente: AgenteArg,
        #[arg(default_value = ".")]
        raiz: PathBuf,
    },
    /// Hook do Claude Code: lê o evento no stdin (proteger | pos-edicao | final)
    Hook { evento: String },
    /// Imprime o JSON Schema do guarda.yml
    Schema,
    /// Imprime a árvore sintática de um arquivo (para escrever regras)
    #[command(hide = true)]
    Arvore { arquivo: PathBuf },
}

#[derive(Subcommand)]
enum BaselineCmd {
    /// Regrava o baseline com todas as violações atuais (decisão humana: congela dívida nova)
    Gerar {
        #[arg(default_value = ".")]
        raiz: PathBuf,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum AgenteArg {
    /// Claude Code: CLAUDE.md e hooks em .claude/settings.json
    Claude,
    /// Outros agentes: AGENTS.md
    Agents,
}

impl From<AgenteArg> for integracao::Agente {
    fn from(a: AgenteArg) -> Self {
        match a {
            AgenteArg::Claude => Self::Claude,
            AgenteArg::Agents => Self::Agents,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum Formato {
    Compacto,
    Json,
}

#[derive(clap::Args)]
struct CheckArgs {
    #[arg(default_value = ".")]
    raiz: PathBuf,
    /// Só esses arquivos (a arquitetura continua valendo pelo índice do projeto)
    #[arg(long, num_args = 1..)]
    arquivos: Vec<PathBuf>,
    /// Só o que mudou contra HEAD, incluindo arquivos novos
    #[arg(long)]
    alterados: bool,
    #[arg(long, value_enum, default_value = "compacto")]
    formato: Formato,
    /// Teto de linhas da saída compacta
    #[arg(long, default_value_t = 30)]
    max: usize,
    /// Ignora o baseline (mostra o legado também)
    #[arg(long)]
    sem_baseline: bool,
    /// Tira do baseline o que foi corrigido e aperta a catraca (nunca acrescenta)
    #[arg(long)]
    atualizar_baseline: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let ok = |r: Result<()>| r.map(|_| ExitCode::SUCCESS);
    let r = match cli.cmd {
        Cmd::Check(a) => check(a),
        Cmd::Init { raiz, forcar, ia } => ok(init(&raiz, forcar, ia)),
        Cmd::Baseline {
            acao: BaselineCmd::Gerar { raiz },
        } => ok(gerar_baseline(&raiz)),
        Cmd::Medir { raiz } => ok(medir(&raiz)),
        Cmd::Contexto { raiz } => ok(Config::carregar(&raiz)
            .and_then(|cfg| integracao::contexto(&raiz, &cfg))
            .map(|t| print!("{t}"))),
        Cmd::Integrar { agente, raiz } => ok(integrar(&raiz, agente)),
        Cmd::Hook { evento } => return rodar_hook(&evento),
        Cmd::Schema => {
            print!("{}", config::schema());
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Arvore { arquivo } => ok(arvore(&arquivo)),
    };
    match r {
        Ok(c) => c,
        Err(e) => {
            eprintln!("guarda: {e:#}");
            ExitCode::from(2)
        }
    }
}

/// Hooks nunca devolvem exit 2 por erro próprio: só por decisão (bloquear).
fn rodar_hook(evento: &str) -> ExitCode {
    match hook::executar(evento) {
        Ok(hook::Resposta::Liberar) => ExitCode::SUCCESS,
        Ok(hook::Resposta::LiberarComMensagem(json)) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Ok(hook::Resposta::Bloquear(texto)) => {
            eprintln!("{texto}");
            ExitCode::from(2)
        }
        Err(e) => {
            eprintln!("guarda hook {evento}: {e:#}");
            ExitCode::from(1)
        }
    }
}

fn integrar(raiz: &Path, agente: AgenteArg) -> Result<()> {
    let cfg = Config::carregar(raiz)?;
    let escritos = integracao::integrar(raiz, &cfg, agente.into())?;
    println!("integrado: {}", escritos.join(", "));
    if matches!(agente, AgenteArg::Claude) {
        println!("hooks: proteger (antes de editar), pos-edicao, final (antes de encerrar)");
        println!("o comando `guarda` precisa estar no PATH do Claude Code");
    }
    Ok(())
}

fn check(a: CheckArgs) -> Result<ExitCode> {
    let raiz = &a.raiz;
    let cfg = Config::carregar(raiz)?;
    let pedido = if a.alterados {
        Pedido::Alterados
    } else if !a.arquivos.is_empty() {
        let base = std::path::absolute(raiz)?;
        let mut rels = Vec::new();
        for f in &a.arquivos {
            let abs = std::path::absolute(f)?;
            let rel = descoberta::relativo(&base, &abs)
                .with_context(|| format!("{} está fora de {}", f.display(), raiz.display()))?;
            rels.push(rel);
        }
        Pedido::Arquivos(rels)
    } else {
        Pedido::Tudo
    };
    let op = Opcoes {
        sem_baseline: a.sem_baseline,
        atualizar_baseline: a.atualizar_baseline,
    };
    let r = gate::verificar(raiz, &cfg, pedido, &op)?;
    if a.atualizar_baseline && r.corrigidas > 0 {
        eprintln!("baseline: {} corrigida(s) removida(s)", r.corrigidas);
    }
    let falhou = r.falhou;
    match a.formato {
        Formato::Compacto => {
            let resumo = saida::Resumo {
                congeladas: r.congeladas,
                tempo: r.tempo,
                falhou,
            };
            print!("{}", saida::compacta(&r.novas, a.max, &resumo));
        }
        Formato::Json => print!("{}", saida::json(&r.novas)),
    }
    Ok(if falhou {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn gerar_baseline(raiz: &Path) -> Result<()> {
    let cfg = Config::carregar(raiz)?;
    let rel = motor::executar(raiz, &cfg, &Escopo::Tudo)?;
    let b = Baseline::de_violacoes(&rel.violacoes);
    let caminho = gate::caminho_baseline(raiz, &cfg);
    b.salvar(&caminho)?;
    println!(
        "baseline: {} violações congeladas em {} regras ({})",
        b.violacoes.len(),
        b.regras(),
        cfg.gate.baseline
    );
    Ok(())
}

struct Build {
    descricao: String,
    cobertura: Option<&'static str>,
}

fn detectar_build(raiz: &Path) -> Build {
    let mut poms = 0;
    let mut gradles = 0;
    let mut textos = String::new();
    for e in ignore::WalkBuilder::new(raiz)
        .hidden(true)
        .require_git(false)
        .build()
        .flatten()
    {
        let nome = e.file_name().to_string_lossy();
        let eh_pom = nome == "pom.xml";
        let eh_gradle = nome == "build.gradle" || nome == "build.gradle.kts";
        if eh_pom || eh_gradle {
            if eh_pom {
                poms += 1
            } else {
                gradles += 1
            }
            textos.push_str(&std::fs::read_to_string(e.path()).unwrap_or_default());
        }
    }
    let (sistema, n) = if gradles > 0 {
        ("gradle", gradles)
    } else if poms > 0 {
        ("maven", poms)
    } else {
        ("sem build", 0)
    };
    let descricao = match n {
        0 => sistema.to_string(),
        1 => format!("{sistema} (1 módulo)"),
        n => format!("{sistema} multi-módulo ({n})"),
    };
    let cobertura = if textos.contains("kover") {
        Some("kover")
    } else if textos.contains("jacoco") {
        Some("jacoco")
    } else {
        None
    };
    Build {
        descricao,
        cobertura,
    }
}

fn contar_linguagens(rel: &Relatorio) -> BTreeMap<&'static str, usize> {
    let mut m = BTreeMap::from([("java", 0), ("kotlin", 0)]);
    for a in &rel.analises {
        *m.entry(a.ling.nome()).or_default() += 1;
    }
    m
}

fn linha_arquitetura(rel: &Relatorio) -> String {
    let base = rel.projeto.base.as_deref().unwrap_or("?");
    let ranking: Vec<String> = rel
        .ranking
        .iter()
        .map(|p| format!("{} {:.0}%", p.preset, p.nota * 100.0))
        .collect();
    let escolha = match (&rel.arquitetura, &rel.origem) {
        (Some(a), Origem::Config) => format!("{} (do guarda.yml)", a.nome),
        (Some(a), _) => format!(
            "{} (confiança {:.0}%)",
            a.nome,
            rel.confianca().unwrap_or(0.0) * 100.0
        ),
        (None, _) => format!(
            "nenhuma com confiança ≥ {:.0}%: ARQ001 desligada até declarar as camadas",
            arquitetura::CONFIANCA_MINIMA * 100.0
        ),
    };
    format!(
        "arquitetura: {escolha} base={base}\n  ranking: {}",
        ranking.join(" · ")
    )
}

fn init(raiz: &Path, forcar: bool, ia: Option<AgenteArg>) -> Result<()> {
    let caminho_cfg = raiz.join(config::ARQUIVO);
    if caminho_cfg.exists() && !forcar {
        bail!(
            "{} já existe (use --forcar para sobrescrever)",
            caminho_cfg.display()
        );
    }
    let cfg_auto = Config::default();
    let rel = motor::executar(raiz, &cfg_auto, &Escopo::Tudo)?;
    let build = detectar_build(raiz);
    let linguagens = contar_linguagens(&rel);

    let (preset, comentario) = match &rel.arquitetura {
        Some(a) => (
            a.nome.clone(),
            format!("confiança {:.0}%", rel.confianca().unwrap_or(0.0) * 100.0),
        ),
        None => (
            "nenhum".to_string(),
            "nenhum preset passou de 60%: declare as camadas".to_string(),
        ),
    };
    let base = rel.projeto.base.clone().unwrap_or_default();
    let yml = format!(
        "# yaml-language-server: $schema=./{}\n\
         # gerado por guarda init {} — edite à vontade; só o que difere do padrão precisa estar aqui\n\
         versao: 1\n\
         perfil: recomendado\n\
         \n\
         fontes:\n  producao: [\"**/src/main/**\"]\n\
         \n\
         arquitetura:\n  preset: {preset}  # {comentario}\n  base: {base}\n\
         \n\
         limites:\n  linhas-funcao: 60\n  complexidade-cognitiva: 15\n\
         \n\
         gate:\n  falhar-em: erro\n  baseline: .guarda/baseline.json\n  supressao: exigir-motivo\n",
        config::ARQUIVO_SCHEMA,
        env!("CARGO_PKG_VERSION")
    );
    let cfg = Config::de_texto(&yml).context("guarda.yml gerado é inválido (bug)")?;
    std::fs::write(&caminho_cfg, &yml)?;
    integracao::gravar_schema(raiz)?;

    // Com o preset e a base fixados, a análise é a mesma que o check vai fazer.
    let rel = motor::executar(raiz, &cfg, &Escopo::Tudo)?;
    let b = Baseline::de_violacoes(&rel.violacoes);
    b.salvar(&gate::caminho_baseline(raiz, &cfg))?;
    let mut escritos = vec![
        "guarda.yml".to_string(),
        cfg.gate.baseline.clone(),
        config::ARQUIVO_SCHEMA.to_string(),
    ];
    if let Some(agente) = ia {
        escritos.extend(integracao::integrar(raiz, &cfg, agente.into())?);
    }

    let erros_parse = rel.analises.iter().filter(|a| a.erro_parse).count();
    println!(
        "guarda init: kotlin {} arq, java {} arq, {}",
        linguagens["kotlin"], linguagens["java"], build.descricao
    );
    println!("{}", linha_arquitetura(&rel));
    println!(
        "baseline: {} violações congeladas em {} regras",
        b.violacoes.len(),
        b.regras()
    );
    println!("parse: {erros_parse} arquivo(s) com erro (ERR000)");
    match build.cobertura {
        Some(c) => println!("cobertura: {c} encontrado no build (lido a partir da fase 2)"),
        None => println!("cobertura: nem JaCoCo nem Kover no build"),
    }
    println!(
        "escrito: {} · {}",
        escritos.join(", "),
        saida::segundos(rel.tempo)
    );
    Ok(())
}

fn medir(raiz: &Path) -> Result<()> {
    let cfg = Config::carregar(raiz)?;
    // A primeira execução paga disco frio e antivírus; o portão vale para o uso contínuo.
    // Sem cache mede o parser de verdade; com cache, o que a IA sente a cada edição.
    let frio = motor::executar_com(raiz, &cfg, &Escopo::Tudo, false)?.tempo;
    let rel = motor::executar_com(raiz, &cfg, &Escopo::Tudo, false)?;
    motor::executar(raiz, &cfg, &Escopo::Tudo)?;
    let com_cache = motor::executar(raiz, &cfg, &Escopo::Tudo)?.tempo;
    let total = rel.analises.len();
    let linguagens = contar_linguagens(&rel);
    let linhas: usize = rel.analises.iter().map(|a| a.linhas).sum();
    let com_erro: Vec<&motor::Analise> = rel.analises.iter().filter(|a| a.erro_parse).collect();
    let pct = if total == 0 {
        0.0
    } else {
        com_erro.len() as f64 * 100.0 / total as f64
    };
    let lento = rel.analises.iter().max_by_key(|a| a.tempo);
    let soma: f64 = rel.analises.iter().map(|a| a.tempo.as_secs_f64()).sum();

    println!(
        "arquivos: {total} (kotlin {}, java {}) · {linhas} linhas",
        linguagens["kotlin"], linguagens["java"]
    );
    println!("parse: {} com erro ({:.2}%)", com_erro.len(), pct);
    for a in &com_erro {
        if let Some(v) = a.violacoes.first() {
            println!("  {}:{} {}", a.arquivo, v.linha, v.mensagem);
        }
    }
    println!(
        "tempo: {} sem cache (a frio {}) · {} com cache · {:.2} ms/arquivo em média · mais lento {:.1} ms ({})",
        saida::segundos(rel.tempo),
        saida::segundos(frio),
        saida::segundos(com_cache),
        if total == 0 {
            0.0
        } else {
            soma * 1000.0 / total as f64
        },
        lento.map_or(0.0, |a| a.tempo.as_secs_f64() * 1000.0),
        lento.map_or("-", |a| a.arquivo.as_str())
    );
    let mut por_regra: BTreeMap<&str, usize> =
        modelo::REGRAS.iter().map(|r| (r.codigo, 0)).collect();
    for v in &rel.violacoes {
        *por_regra.entry(v.codigo).or_default() += 1;
    }
    println!("violações: {}", rel.violacoes.len());
    for (c, n) in &por_regra {
        let nome = modelo::regra(c).map_or("", |r| r.nome);
        println!("  {c} {n:>5}  {nome}");
    }
    println!("{}", linha_arquitetura(&rel));
    for p in &rel.ranking {
        println!(
            "  {}: {}/{} pacotes de 1º nível e {}/{} arquivos em camadas ({} camadas), {} com import proibido",
            p.preset,
            p.topo_cobertos,
            p.topo_total,
            p.cobertos,
            p.total,
            p.camadas_usadas,
            p.com_violacao
        );
    }
    let ok_parse = pct < 1.0;
    let ok_tempo = rel.tempo.as_secs_f64() < 1.0;
    println!(
        "portão fase 0: parse < 1% {} · projeto todo < 1s {}",
        if ok_parse { "ok" } else { "FALHOU" },
        if ok_tempo { "ok" } else { "FALHOU" }
    );
    Ok(())
}

fn arvore(arquivo: &Path) -> Result<()> {
    let nome = arquivo.to_string_lossy();
    let ling = lang::Linguagem::do_caminho(&nome).context("só .java e .kt")?;
    let fonte = std::fs::read_to_string(arquivo)?;
    let mut p = lang::Parsers::novo();
    let t = p.ler(ling, &fonte).context("parser não devolveu árvore")?;
    print!("{}", lang::despejar(t.root_node(), &fonte));
    Ok(())
}
