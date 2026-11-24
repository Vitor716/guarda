//! guarda: linter determinístico para Kotlin e Java, com saída enxuta para IA.

mod arquitetura;
mod baseline;
mod build;
mod cache;
mod cobertura;
mod codigo_novo;
mod config;
mod descoberta;
mod duplicacao;
mod explicar;
mod gate;
mod hook;
mod integracao;
mod lang;
mod mcp;
mod modelo;
mod motor;
mod recursos;
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
        /// Também integra com agentes (repita: --ia claude --ia codex --ia git)
        #[arg(long, value_enum)]
        ia: Vec<AgenteArg>,
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
    /// Integra com um agente: instruções, hooks e MCP (ou o pre-commit do git)
    Integrar {
        #[arg(value_enum)]
        agente: AgenteArg,
        #[arg(default_value = ".")]
        raiz: PathBuf,
    },
    /// Hook de agente: lê o evento no stdin e responde no protocolo do agente
    Hook {
        /// proteger | pos-edicao | final
        evento: String,
        #[arg(long, value_enum, default_value = "claude")]
        agente: AgenteHook,
    },
    /// Explica uma regra (por quê, errado, certo); sem código, lista as regras
    Explicar { codigo: Option<String> },
    /// Servidor MCP por stdio (verificar, explicar, arquitetura) para qualquer agente
    Mcp {
        /// Raiz do projeto (padrão: o diretório com guarda.yml acima do atual)
        #[arg(long)]
        raiz: Option<PathBuf>,
    },
    /// Imprime o JSON Schema do guarda.yml
    Schema,
    /// Imprime a árvore sintática de um arquivo (para escrever regras)
    #[command(hide = true)]
    Arvore { arquivo: PathBuf },
}

#[derive(Subcommand)]
enum BaselineCmd {
    /// Regrava o baseline com as violações atuais, preservando os motivos já aceitos
    Gerar {
        #[arg(default_value = ".")]
        raiz: PathBuf,
        /// Atualiza só estas regras (ex.: --regras SEC001 SPR006), sem tocar nas demais
        #[arg(long, num_args = 1..)]
        regras: Vec<String>,
    },
    /// Congela violações atuais de uma regra num arquivo (ou sobe catraca), com motivo
    Aceitar {
        /// Código da regra (ex.: CPX001)
        codigo: String,
        /// Arquivo, relativo à raiz
        arquivo: PathBuf,
        /// Por que a dívida é aceita (fica gravado no baseline)
        #[arg(long)]
        motivo: String,
        /// Aceita só a(s) violação(ões) desta linha
        #[arg(long)]
        linha: Option<usize>,
        /// Aceita só a(s) violação(ões) deste símbolo (ex.: Classe.metodo)
        #[arg(long)]
        simbolo: Option<String>,
        #[arg(long, default_value = ".")]
        raiz: PathBuf,
    },
    /// Mostra a saúde do baseline: entradas por regra, motivos, corrigidas e catracas
    Status {
        #[arg(default_value = ".")]
        raiz: PathBuf,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum AgenteArg {
    /// Claude Code: CLAUDE.md, hooks em .claude/settings.json, MCP em .mcp.json
    Claude,
    /// Codex (ChatGPT): AGENTS.md e hooks em .codex/hooks.json
    Codex,
    /// Antigravity (Gemini): AGENTS.md, .agents/hooks.json e .agents/mcp_config.json
    Antigravity,
    /// Qualquer agente que leia AGENTS.md (Cursor, Copilot...)
    Agents,
    /// Hook de pre-commit: barra o commit de qualquer IA ou pessoa
    Git,
    /// Gradle: tarefa guardaCheck ligada ao `check`
    Gradle,
    /// Maven: imprime o trecho do exec-maven-plugin para o `verify`
    Maven,
}

impl From<AgenteArg> for integracao::Agente {
    fn from(a: AgenteArg) -> Self {
        match a {
            AgenteArg::Claude => Self::Claude,
            AgenteArg::Codex => Self::Codex,
            AgenteArg::Antigravity => Self::Antigravity,
            AgenteArg::Agents => Self::Agents,
            AgenteArg::Git => Self::Git,
            AgenteArg::Gradle => Self::Gradle,
            AgenteArg::Maven => Self::Maven,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum AgenteHook {
    Claude,
    Codex,
    Antigravity,
}

impl From<AgenteHook> for hook::Agente {
    fn from(a: AgenteHook) -> Self {
        match a {
            AgenteHook::Claude => Self::Claude,
            AgenteHook::Codex => Self::Codex,
            AgenteHook::Antigravity => Self::Antigravity,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum Formato {
    /// Uma linha por violação (padrão fora do terminal: feito para IA)
    Compacto,
    /// Um objeto JSON por linha
    Json,
    /// SARIF 2.1.0 (code scanning, IDE)
    Sarif,
    /// Com cor, para pessoas (padrão no terminal)
    Texto,
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
    /// Padrão: texto no terminal, compacto quando a saída vai para outro programa (IA, CI)
    #[arg(long, value_enum)]
    formato: Option<Formato>,
    /// Teto de linhas da saída compacta
    #[arg(long, default_value_t = 30)]
    max: usize,
    /// Ignora o baseline (mostra o legado também)
    #[arg(long)]
    sem_baseline: bool,
    /// Tira do baseline o que foi corrigido e aperta a catraca (nunca acrescenta)
    #[arg(long)]
    atualizar_baseline: bool,
    /// Roda os testes antes, para a cobertura refletir o código atual
    #[arg(long)]
    cobertura: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let ok = |r: Result<()>| r.map(|_| ExitCode::SUCCESS);
    let r = match cli.cmd {
        Cmd::Check(a) => check(a),
        Cmd::Init { raiz, forcar, ia } => ok(init(&raiz, forcar, ia)),
        Cmd::Baseline {
            acao: BaselineCmd::Gerar { raiz, regras },
        } => ok(gerar_baseline(&raiz, &regras)),
        Cmd::Baseline {
            acao:
                BaselineCmd::Aceitar {
                    codigo,
                    arquivo,
                    motivo,
                    linha,
                    simbolo,
                    raiz,
                },
        } => ok(aceitar_no_baseline(
            &raiz,
            &codigo,
            &arquivo,
            &motivo,
            linha,
            simbolo.as_deref(),
        )),
        Cmd::Baseline {
            acao: BaselineCmd::Status { raiz },
        } => ok(status_baseline(&raiz)),
        Cmd::Medir { raiz } => ok(medir(&raiz)),
        Cmd::Contexto { raiz } => ok(Config::carregar(&raiz)
            .and_then(|cfg| integracao::contexto(&raiz, &cfg))
            .map(|t| print!("{t}"))),
        Cmd::Integrar { agente, raiz } => ok(integrar(&raiz, agente)),
        Cmd::Hook { evento, agente } => return rodar_hook(&evento, agente.into()),
        Cmd::Explicar { codigo } => match codigo {
            None => {
                print!("{}", explicar::lista());
                Ok(ExitCode::SUCCESS)
            }
            Some(c) => match explicar::explicar(&c) {
                Some(t) => {
                    print!("{t}");
                    Ok(ExitCode::SUCCESS)
                }
                None => Err(anyhow::anyhow!(
                    "regra desconhecida: {c} (veja `guarda explicar`)"
                )),
            },
        },
        Cmd::Mcp { raiz } => {
            let raiz = match raiz {
                Some(r) => r,
                None => mcp::achar_raiz(&std::env::current_dir().unwrap_or_else(|_| ".".into())),
            };
            ok(mcp::servir(&raiz))
        }
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

/// Hooks nunca bloqueiam por erro próprio: só por decisão.
fn rodar_hook(evento: &str, agente: hook::Agente) -> ExitCode {
    let saida = match hook::Fase::de_texto(evento) {
        Err(e) => {
            eprintln!("guarda hook: {e:#}");
            return ExitCode::from(1);
        }
        Ok(fase) => match hook::executar(agente, fase) {
            Ok(d) => hook::responder(agente, fase, &d),
            Err(e) => hook::responder_erro(agente, fase, &format!("guarda hook {evento}: {e:#}")),
        },
    };
    if let Some(o) = saida.stdout {
        println!("{o}");
    }
    if let Some(e) = saida.stderr {
        eprintln!("{e}");
    }
    ExitCode::from(saida.codigo)
}

fn integrar(raiz: &Path, agente: AgenteArg) -> Result<()> {
    let cfg = Config::carregar(raiz)?;
    let r = integracao::integrar(raiz, &cfg, agente.into())?;
    if !r.escritos.is_empty() {
        println!("integrado: {}", r.escritos.join(", "));
    }
    for n in r.notas {
        println!("- {n}");
    }
    match agente {
        AgenteArg::Agents => {}
        AgenteArg::Gradle | AgenteArg::Maven => {
            println!(
                "- o comando `guarda` precisa estar no PATH de quem roda o build (Gradle também aceita GUARDA_BIN ou -Pguarda.bin)"
            )
        }
        _ => println!("- o comando `guarda` precisa estar no PATH do agente"),
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
        cobertura: a.cobertura,
        atualizar_baseline: a.atualizar_baseline,
    };
    let r = gate::verificar(raiz, &cfg, pedido, &op)?;
    if a.atualizar_baseline && (r.corrigidas > 0 || r.catracas_apertadas > 0) {
        let mut partes = Vec::new();
        if r.corrigidas > 0 {
            partes.push(format!("{} corrigida(s) removida(s)", r.corrigidas));
        }
        if r.catracas_apertadas > 0 {
            partes.push(format!("{} catraca(s) apertada(s)", r.catracas_apertadas));
        }
        eprintln!("baseline: {}", partes.join(", "));
    }
    let falhou = r.falhou;
    let formato = a.formato.unwrap_or_else(|| {
        use std::io::IsTerminal;
        if std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none() {
            Formato::Texto
        } else {
            Formato::Compacto
        }
    });
    let resumo = saida::Resumo {
        congeladas: r.congeladas,
        tempo: r.tempo,
        falhou,
        nota: r.nota.clone(),
    };
    match formato {
        Formato::Compacto => print!("{}", saida::compacta(&r.novas, a.max, &resumo)),
        Formato::Texto => print!("{}", saida::texto(&r.novas, a.max, &resumo)),
        Formato::Json => print!("{}", saida::json(&r.novas)),
        Formato::Sarif => print!("{}", saida::sarif(&r.novas)),
    }
    Ok(if falhou {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn gerar_baseline(raiz: &Path, regras: &[String]) -> Result<()> {
    let cfg = Config::carregar(raiz)?;
    let filtro: Option<Vec<String>> = if regras.is_empty() {
        None
    } else {
        let mut cods = Vec::new();
        for r in regras {
            let c = r.trim().to_uppercase();
            if modelo::regra(&c).is_none() {
                bail!("regra desconhecida: {c} (veja `guarda explicar`)");
            }
            cods.push(c);
        }
        Some(cods)
    };
    let c = gate::coletar(raiz, &cfg, Pedido::Tudo, &Opcoes::default())?;
    let caminho = gate::caminho_baseline(raiz, &cfg);
    let anterior = Baseline::carregar(&caminho)?;
    let (b, delta) = match &anterior {
        Some(ant) => ant.regerar(&c.congelaveis, filtro.as_deref()),
        None => {
            let alvo: Vec<_> = match &filtro {
                Some(fs) => c
                    .congelaveis
                    .into_iter()
                    .filter(|v| fs.iter().any(|f| f == v.codigo))
                    .collect(),
                None => c.congelaveis,
            };
            let b = Baseline::de_violacoes(&alvo);
            let n = b.violacoes.len();
            (
                b,
                baseline::Regeracao {
                    novas: n,
                    removidas: 0,
                    motivos_preservados: 0,
                },
            )
        }
    };
    b.salvar(&caminho)?;
    if anterior.is_some() {
        println!(
            "baseline: {} violações congeladas em {} regras (+{} novas, -{} removidas, {} motivo(s) preservado(s)) ({})",
            b.violacoes.len(),
            b.regras(),
            delta.novas,
            delta.removidas,
            delta.motivos_preservados,
            cfg.gate.baseline
        );
    } else {
        println!(
            "baseline: {} violações congeladas em {} regras ({})",
            b.violacoes.len(),
            b.regras(),
            cfg.gate.baseline
        );
    }
    Ok(())
}

fn aceitar_no_baseline(
    raiz: &Path,
    codigo: &str,
    arquivo: &Path,
    motivo: &str,
    linha_alvo: Option<usize>,
    simbolo_alvo: Option<&str>,
) -> Result<()> {
    let motivo = motivo.trim();
    if motivo.chars().count() < 10 {
        anyhow::bail!("--motivo precisa explicar a decisão (10 caracteres ou mais)");
    }
    let codigo = codigo.trim().to_uppercase();
    if modelo::regra(&codigo).is_none() {
        anyhow::bail!("regra desconhecida: {codigo} (veja `guarda explicar`)");
    }
    let base = std::path::absolute(raiz)?;
    let abs = std::path::absolute(raiz.join(arquivo))
        .ok()
        .filter(|p| p.exists())
        .map_or_else(|| std::path::absolute(arquivo), Ok)?;
    let rel = descoberta::relativo(&base, &abs)
        .with_context(|| format!("{} está fora de {}", arquivo.display(), raiz.display()))?;
    let cfg = Config::carregar(raiz)?;
    let c = gate::coletar(
        raiz,
        &cfg,
        Pedido::Arquivos(vec![rel.clone()]),
        &Opcoes::default(),
    )?;
    let alvo: Vec<_> = c
        .congelaveis
        .into_iter()
        .filter(|v| v.codigo == codigo && v.arquivo == rel)
        .filter(|v| linha_alvo.is_none_or(|l| v.linha == l))
        .filter(|v| simbolo_alvo.is_none_or(|s| v.simbolo == s))
        .collect();
    let caminho = gate::caminho_baseline(raiz, &cfg);
    let mut b = Baseline::carregar(&caminho)?.unwrap_or_default();
    let r = b.aceitar(&alvo, motivo);
    b.salvar(&caminho)?;
    let mut extra = String::new();
    if r.atualizadas > 0 {
        extra.push_str(&format!(", {} catraca(s) atualizada(s)", r.atualizadas));
    }
    println!(
        "baseline: {} violação(ões) de {codigo} em {rel} aceita(s){extra}; {} já estava(m) congelada(s)",
        r.novas, r.ja_congeladas
    );
    Ok(())
}

fn status_baseline(raiz: &Path) -> Result<()> {
    let cfg = Config::carregar(raiz)?;
    let caminho = gate::caminho_baseline(raiz, &cfg);
    let Some(b) = Baseline::carregar(&caminho)? else {
        println!(
            "baseline: nenhum arquivo encontrado em {} (rode `guarda init` ou `guarda baseline gerar`)",
            cfg.gate.baseline
        );
        return Ok(());
    };
    let coleta = gate::coletar(raiz, &cfg, Pedido::Tudo, &Opcoes::default())?;
    let analisados: std::collections::HashSet<&str> =
        coleta.analisados.iter().map(String::as_str).collect();
    let comp = baseline::comparar(&b, coleta.congelaveis, &analisados, true);

    let com_motivo = b.com_motivo();
    let legado = b.violacoes.len() - com_motivo;
    println!(
        "baseline: {} entradas em {} regras ({com_motivo} com motivo, {legado} legado) ({})",
        b.violacoes.len(),
        b.regras(),
        cfg.gate.baseline
    );
    println!(
        "  corrigidas (podem sair): {}{}",
        comp.corrigidas,
        if comp.arquivos_removidos > 0 {
            format!(" ({} em arquivos removidos)", comp.arquivos_removidos)
        } else {
            String::new()
        }
    );
    println!("  catracas que podem apertar: {}", comp.catracas_apertadas);
    println!("  novas fora do baseline: {}", comp.novas.len());

    let mut por_regra: BTreeMap<&str, usize> = BTreeMap::new();
    for e in &b.violacoes {
        *por_regra.entry(e.codigo.as_str()).or_default() += 1;
    }
    let mut ordenado: Vec<(&str, usize)> = por_regra.into_iter().collect();
    ordenado.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    if !ordenado.is_empty() {
        println!("por regra:");
        for (c, n) in ordenado {
            let nome = modelo::regra(c).map_or("", |r| r.nome);
            println!("  {c} {n:>5}  {nome}");
        }
    }
    if comp.corrigidas > 0 || comp.catracas_apertadas > 0 {
        println!("dica: rode `guarda check --atualizar-baseline` para enxugar o baseline");
    }
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

fn init(raiz: &Path, forcar: bool, ia: Vec<AgenteArg>) -> Result<()> {
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
    let referencia = referencia_padrao(raiz);
    let cobertura = match build.cobertura {
        Some(_) => format!(
            "\ncobertura:  # Sonar way: 80% do código novo coberto\n  relatorio: auto\n  codigo-novo: {{ referencia: {referencia}, minimo: 80 }}\n"
        ),
        None => String::new(),
    };
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
         {cobertura}\
         \n\
         duplicacao:  # Sonar way: no máximo 3% de linhas duplicadas no código novo\n  tokens-minimos: 100\n  codigo-novo-maximo: 3\n\
         \n\
         gate:\n  falhar-em: erro\n  baseline: .guarda/baseline.json\n  supressao: exigir-motivo\n",
        config::ARQUIVO_SCHEMA,
        env!("CARGO_PKG_VERSION")
    );
    let cfg = Config::de_texto(&yml).context("guarda.yml gerado é inválido (bug)")?;
    std::fs::write(&caminho_cfg, &yml)?;
    integracao::gravar_schema(raiz)?;

    // Com o preset e a base fixados, a análise é a mesma que o check vai fazer.
    let coleta = gate::coletar(raiz, &cfg, Pedido::Tudo, &Opcoes::default())?;
    let rel = coleta.rel;
    let b = Baseline::de_violacoes(&coleta.congelaveis);
    b.salvar(&gate::caminho_baseline(raiz, &cfg))?;
    let mut escritos = vec![
        "guarda.yml".to_string(),
        cfg.gate.baseline.clone(),
        config::ARQUIVO_SCHEMA.to_string(),
    ];
    let mut notas = Vec::new();
    for agente in ia {
        let r = integracao::integrar(raiz, &cfg, agente.into())?;
        escritos.extend(r.escritos);
        notas.extend(r.notas);
    }
    escritos.dedup();

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
        Some(c) => println!("cobertura: {c} no build; código novo contra {referencia}"),
        None => println!("cobertura: nem JaCoCo nem Kover no build (seção cobertura não escrita)"),
    }
    println!(
        "escrito: {} · {}",
        escritos.join(", "),
        saida::segundos(rel.tempo)
    );
    for n in notas {
        println!("- {n}");
    }
    Ok(())
}

/// A referência do código novo: o branch principal remoto, se existir.
fn referencia_padrao(raiz: &Path) -> String {
    let existe = |r: &str| {
        std::process::Command::new("git")
            .args(["rev-parse", "--verify", "--quiet", r])
            .current_dir(raiz)
            .output()
            .is_ok_and(|o| o.status.success())
    };
    ["origin/main", "origin/master", "main", "master"]
        .into_iter()
        .find(|r| existe(r))
        .unwrap_or("origin/main")
        .to_string()
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
            "  {}: {}/{} pacotes e {}/{} arquivos em camadas ({} camadas), {} com import proibido, {} marca(s) do estilo",
            p.preset,
            p.pacotes_cobertos,
            p.pacotes_total,
            p.cobertos,
            p.total,
            p.camadas_usadas,
            p.com_violacao,
            p.marcas
        );
    }
    let cc_padrao = config::Cobertura::default();
    let cc = cfg.cobertura.as_ref().unwrap_or(&cc_padrao);
    match cobertura::carregar(raiz, cc)? {
        Some(r) => println!(
            "cobertura: geral {} em {}{}",
            cobertura::resumo_geral(&r).map_or("-".into(), cobertura::pct),
            r.caminhos.join(", "),
            if cfg.cobertura.is_none() {
                " (seção cobertura ausente: não vale no gate)"
            } else {
                ""
            }
        ),
        None => println!("cobertura: nenhum relatório JaCoCo/Kover encontrado"),
    }
    match codigo_novo::calcular(raiz, cfg.referencia(), None) {
        Ok(n) => println!(
            "código novo: {} linha(s) em {} arquivo(s) contra {}",
            n.por_arquivo.values().map(|l| l.len()).sum::<usize>(),
            n.por_arquivo.len(),
            n.referencia
        ),
        Err(e) => println!("código novo: indisponível ({e})"),
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
