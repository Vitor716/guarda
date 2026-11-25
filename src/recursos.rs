//! Arquivos que não são código: configuração do Spring (`application*.properties|yml`) e
//! arquivos de segredo versionáveis (`.env`, chave privada, keystore).
//!
//! Regras: SEC001 credencial fixa na configuração (Sonar S2068/S6418), SEC017 configuração
//! insegura (Sonar S4507; actuator exposto, console H2 remoto, stack trace na resposta) e
//! SEC018 arquivo de segredo fora do .gitignore (skill security-audit da Cloudflare,
//! "obvious things"). A mensagem nunca repete o valor do segredo.

use crate::modelo::Violacao;
use crate::regras::sintaxe::{formato_de_segredo, nome_de_segredo, valor_de_segredo};
use std::collections::HashSet;
use std::path::Path;

/// Tipo de recurso pelo nome do arquivo.
pub enum Recurso {
    Configuracao,
    Segredo,
}

pub fn tipo(rel: &str) -> Option<Recurso> {
    let nome = rel.rsplit('/').next().unwrap_or(rel);
    let l = nome.to_lowercase();
    let teste =
        rel.contains("/test/") || rel.starts_with("test/") || rel.contains("/testFixtures/");
    let config = (l.starts_with("application") || l.starts_with("bootstrap"))
        && [".properties", ".yml", ".yaml"]
            .iter()
            .any(|e| l.ends_with(e));
    if config && !teste {
        return Some(Recurso::Configuracao);
    }
    let exemplo = ["example", "sample", "template", "dist", "modelo", "exemplo"]
        .iter()
        .any(|e| l.contains(e));
    let env = l == ".env" || (l.starts_with(".env.") && !exemplo);
    let chave = [".pem", ".key", ".p12", ".pfx", ".jks", ".keystore"]
        .iter()
        .any(|e| l.ends_with(e))
        || matches!(
            l.as_str(),
            "id_rsa" | "id_dsa" | "id_ecdsa" | "id_ed25519" | "credentials.json"
        )
        || (l.starts_with("service-account") && l.ends_with(".json"));
    if env || (chave && !teste && !exemplo) {
        return Some(Recurso::Segredo);
    }
    None
}

/// Perfil de desenvolvimento ou teste: debug ligado ali é o esperado.
fn perfil_local(rel: &str) -> bool {
    let nome = rel.rsplit('/').next().unwrap_or(rel).to_lowercase();
    ["-dev", "-local", "-test", "-desenv", "-h2"]
        .iter()
        .any(|p| nome.contains(p))
}

/// (chave completa, valor, linha) de um .properties.
fn properties(t: &str) -> Vec<(String, String, usize)> {
    let mut v = Vec::new();
    for (i, l) in t.lines().enumerate() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with('!') {
            continue;
        }
        let Some(p) = l.find(['=', ':']) else {
            continue;
        };
        v.push((
            l[..p].trim().to_string(),
            l[p + 1..].trim().to_string(),
            i + 1,
        ));
    }
    v
}

/// (chave completa, valor, linha) de um YAML simples (mapas aninhados por recuo).
fn yaml(t: &str) -> Vec<(String, String, usize)> {
    let mut v = Vec::new();
    let mut pilha: Vec<(usize, String)> = Vec::new();
    for (i, linha) in t.lines().enumerate() {
        let sem_comentario = linha.split(" #").next().unwrap_or(linha);
        let conteudo = sem_comentario.trim_end();
        let c = conteudo.trim_start();
        if c == "---" {
            pilha.clear();
            continue;
        }
        if c.is_empty() || c.starts_with('#') || c.starts_with('-') {
            continue;
        }
        let recuo = conteudo.len() - c.len();
        let Some(p) = c.find(':') else { continue };
        let chave = c[..p].trim().trim_matches(['"', '\'']).to_string();
        let valor = c[p + 1..].trim().to_string();
        while pilha.last().is_some_and(|(r, _)| *r >= recuo) {
            pilha.pop();
        }
        let completa = pilha
            .iter()
            .map(|(_, k)| k.as_str())
            .chain(std::iter::once(chave.as_str()))
            .collect::<Vec<_>>()
            .join(".");
        if valor.is_empty() || valor == "|" || valor == ">" {
            pilha.push((recuo, chave));
        } else {
            v.push((completa, valor.trim_matches(['"', '\'']).to_string(), i + 1));
        }
    }
    v
}

/// Endpoints do Actuator que expõem segredos, memória ou controle da JVM.
fn actuator_sensivel(valor: &str) -> bool {
    const SENSIVEIS: &[&str] = &[
        "env",
        "heapdump",
        "shutdown",
        "configprops",
        "beans",
        "mappings",
        "threaddump",
        "logfile",
        "httptrace",
        "httpexchanges",
    ];
    valor
        .split([',', ' ', '[', ']', '"', '\''])
        .map(str::trim)
        .any(|ep| SENSIVEIS.contains(&ep))
}

fn violacao(
    codigo: &'static str,
    rel: &str,
    linha: usize,
    msg: String,
    dica: &str,
    chave: &str,
) -> Violacao {
    Violacao::nova(codigo, rel, linha, msg, dica, chave, chave, None)
}

fn configuracao(rel: &str, texto: &str, v: &mut Vec<Violacao>) {
    let pares = if rel.ends_with(".properties") {
        properties(texto)
    } else {
        yaml(texto)
    };
    let local = perfil_local(rel);
    // Banco H2 em memória: a senha só vale dentro do próprio processo.
    let h2_memoria = pares
        .iter()
        .any(|(k, v, _)| k.ends_with("datasource.url") && v.starts_with("jdbc:h2:mem"));
    for (chave, valor, linha) in pares {
        if h2_memoria && chave.ends_with("datasource.password") {
            continue;
        }
        let ultimo = chave.rsplit('.').next().unwrap_or(&chave);
        // `${VAR:padrão}`: o valor padrão também fica no git.
        let fixo = match valor.strip_prefix("${") {
            Some(resto) => resto
                .split_once(':')
                .map(|(_, padrao)| padrao.trim_end_matches('}').to_string()),
            None => Some(valor.clone()),
        };
        if let Some(fixo) = fixo {
            if let Some(tipo) = formato_de_segredo(&fixo) {
                v.push(violacao(
                    "SEC001",
                    rel,
                    linha,
                    format!("{tipo} na configuração ({chave})"),
                    "revogue a chave e leia de variável de ambiente: ${NOME}",
                    &chave,
                ));
            } else if nome_de_segredo(ultimo) && valor_de_segredo(&fixo) {
                let msg = if valor.starts_with("${") {
                    format!("credencial com valor padrão fixo ({chave})")
                } else {
                    format!("credencial fixa na configuração ({chave})")
                };
                v.push(violacao(
                    "SEC001",
                    rel,
                    linha,
                    msg,
                    "use ${NOME_DA_VARIAVEL} sem valor padrão e injete no deploy",
                    &chave,
                ));
            }
        }
        if local {
            continue;
        }
        let l = valor.to_lowercase();
        let inseguro = match chave.as_str() {
            "management.endpoints.web.exposure.include" if l.contains('*') => {
                Some("todos os endpoints do actuator expostos")
            }
            "management.endpoints.web.exposure.include" if actuator_sensivel(&l) => {
                Some("endpoint sensível do actuator exposto")
            }
            "management.endpoint.health.show-details" if l == "always" => {
                Some("detalhes do health expostos para qualquer cliente")
            }
            "spring.h2.console.enabled" if l == "true" => {
                Some("console do H2 habilitado fora de perfil local")
            }
            "spring.h2.console.settings.web-allow-others" if l == "true" => {
                Some("console do H2 aberto para a rede")
            }
            "debug" if l == "true" => Some("modo debug ligado"),
            "server.error.include-stacktrace"
                if matches!(l.as_str(), "always" | "on_param" | "on-param") =>
            {
                Some("stack trace na resposta de erro")
            }
            "server.error.include-exception" if l == "true" => {
                Some("classe da exceção exposta na resposta de erro")
            }
            "management.endpoint.env.show-values"
            | "management.endpoint.configprops.show-values"
                if l == "always" =>
            {
                Some("actuator mostra valores de configuração")
            }
            _ => None,
        };
        if let Some(m) = inseguro {
            v.push(violacao(
                "SEC017",
                rel,
                linha,
                format!("{m} ({chave})"),
                "mova para um perfil de desenvolvimento (application-dev) ou restrinja",
                &chave,
            ));
        }
    }
}

/// Verifica os recursos (só os do escopo, quando houver).
pub fn verificar(
    raiz: &Path,
    recursos: &[String],
    escopo: Option<&HashSet<&str>>,
) -> Vec<Violacao> {
    let mut v = Vec::new();
    for rel in recursos {
        if escopo.is_some_and(|e| !e.contains(rel.as_str())) {
            continue;
        }
        match tipo(rel) {
            Some(Recurso::Configuracao) => {
                if let Ok(t) = std::fs::read_to_string(raiz.join(rel)) {
                    configuracao(rel, &t, &mut v);
                }
            }
            Some(Recurso::Segredo) => {
                let nome = rel.rsplit('/').next().unwrap_or(rel);
                v.push(violacao(
                    "SEC018",
                    rel,
                    1,
                    format!("arquivo de segredo fora do .gitignore ({nome})"),
                    "acrescente ao .gitignore, tire do git (git rm --cached) e troque o segredo",
                    nome,
                ));
            }
            None => {}
        }
    }
    v
}

#[cfg(test)]
mod casos {
    use super::*;

    fn achar(rel: &str, t: &str) -> Vec<(String, usize, String)> {
        let mut v = Vec::new();
        configuracao(rel, t, &mut v);
        v.into_iter()
            .map(|x| (x.codigo.to_string(), x.linha, x.mensagem))
            .collect()
    }

    #[test]
    fn properties_e_yaml() {
        let p = "spring.datasource.password=root1234\njwt.secret=${JWT_SECRET:minha-chave-123}\napi.token.expiration=3600\nspring.datasource.username=app\nmanagement.endpoints.web.exposure.include=*\nsenha.admin=${SENHA}\n";
        let v = achar("src/main/resources/application.properties", p);
        assert_eq!(
            v.iter().map(|x| (x.0.as_str(), x.1)).collect::<Vec<_>>(),
            [("SEC001", 1), ("SEC001", 2), ("SEC017", 5)],
            "{v:?}"
        );
        assert!(v[1].2.contains("valor padrão"), "{v:?}");
        assert!(
            !v.iter().any(|x| x.2.contains("root1234")),
            "nunca repete o segredo"
        );

        let y = "spring:\n  datasource:\n    url: jdbc:postgresql://db/app\n    password: \"s3nh4forte\"\n  h2:\n    console:\n      settings:\n        web-allow-others: true\ndebug: true\n";
        let v = achar("src/main/resources/application.yml", y);
        assert_eq!(
            v.iter().map(|x| (x.0.as_str(), x.1)).collect::<Vec<_>>(),
            [("SEC001", 4), ("SEC017", 8), ("SEC017", 9)],
            "{v:?}"
        );
        // No perfil de desenvolvimento, debug ligado é o esperado.
        let v = achar("src/main/resources/application-dev.yml", "debug: true\n");
        assert!(v.is_empty(), "{v:?}");

        // Novos cenários de SEC017 (on_param, health.show-details, actuator sensível, H2, include-exception).
        let y2 = "server:\n  error:\n    include-stacktrace: on_param\n    include-exception: true\nmanagement:\n  endpoints:\n    web:\n      exposure:\n        include: health,info,env\n  endpoint:\n    health:\n      show-details: always\nspring:\n  h2:\n    console:\n      enabled: true\n";
        let v = achar("src/main/resources/application.yml", y2);
        assert_eq!(
            v.iter().map(|x| (x.0.as_str(), x.1)).collect::<Vec<_>>(),
            [
                ("SEC017", 3),
                ("SEC017", 4),
                ("SEC017", 9),
                ("SEC017", 12),
                ("SEC017", 16),
            ],
            "{v:?}"
        );
    }

    #[test]
    fn senha_de_h2_em_memoria_nao_conta() {
        let p = "spring.datasource.url=jdbc:h2:mem:testdb
spring.datasource.password=password
";
        assert!(achar("src/main/resources/application.properties", p).is_empty());
        let p = "spring.datasource.url=jdbc:h2:file:./dados
spring.datasource.password=password
";
        assert_eq!(
            achar("src/main/resources/application.properties", p).len(),
            1
        );
    }

    #[test]
    fn tipos_de_arquivo() {
        assert!(matches!(tipo(".env"), Some(Recurso::Segredo)));
        assert!(tipo(".env.example").is_none());
        assert!(matches!(tipo("certs/servidor.key"), Some(Recurso::Segredo)));
        assert!(tipo("src/test/resources/teste.jks").is_none());
        assert!(matches!(
            tipo("src/main/resources/application-prod.yml"),
            Some(Recurso::Configuracao)
        ));
        assert!(tipo("src/test/resources/application.yml").is_none());
    }
}
