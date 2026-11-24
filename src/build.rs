//! Integração com o build: a guarda roda junto do `check` do Gradle e do `verify` do Maven.
//!
//! Gradle: um script (`gradle/guarda.gradle.kts` ou `.gradle`) aplicado pelo build da raiz.
//! Maven: o trecho do exec-maven-plugin é impresso para colar no pom (editar XML dos outros
//! às cegas quebra formatação e comentários).

use anyhow::{Result, bail};
use std::path::Path;

const INICIO: &str = "// guarda:inicio (gerado por `guarda integrar gradle`; não edite à mão)";
const FIM: &str = "// guarda:fim";

const GRADLE_KTS: &str = r#"// Gerado por `guarda integrar gradle`: roda a guarda no `check` de qualquer módulo.
// Binário: -Pguarda.bin=caminho, variável GUARDA_BIN ou `guarda` no PATH.
val guardaBin = providers.gradleProperty("guarda.bin")
    .orElse(providers.environmentVariable("GUARDA_BIN"))
    .orElse("guarda")

tasks.register<Exec>("guardaCheck") {
    group = "verification"
    description = "Arquitetura, complexidade e smells pela guarda (guarda check)."
    workingDir = rootDir
    commandLine(guardaBin.get(), "check")
}

allprojects {
    tasks.matching { it.name == "check" }.configureEach {
        dependsOn(rootProject.tasks.named("guardaCheck"))
    }
}
"#;

const GRADLE_GROOVY: &str = r#"// Gerado por `guarda integrar gradle`: roda a guarda no `check` de qualquer módulo.
// Binário: -Pguarda.bin=caminho, variável GUARDA_BIN ou `guarda` no PATH.
def guardaBin = providers.gradleProperty('guarda.bin')
    .orElse(providers.environmentVariable('GUARDA_BIN'))
    .orElse('guarda')

tasks.register('guardaCheck', Exec) {
    group = 'verification'
    description = 'Arquitetura, complexidade e smells pela guarda (guarda check).'
    workingDir = rootDir
    commandLine guardaBin.get(), 'check'
}

allprojects {
    tasks.matching { it.name == 'check' }.configureEach {
        dependsOn rootProject.tasks.named('guardaCheck')
    }
}
"#;

/// Versão conferida no Maven Central em 2026-11-24.
pub const MAVEN: &str = r#"<!-- guarda: roda no `mvn verify`, só no projeto raiz (gerado por `guarda integrar maven`) -->
<plugin>
  <groupId>org.codehaus.mojo</groupId>
  <artifactId>exec-maven-plugin</artifactId>
  <version>3.7.0</version>
  <inherited>false</inherited>
  <executions>
    <execution>
      <id>guarda</id>
      <phase>verify</phase>
      <goals><goal>exec</goal></goals>
      <configuration>
        <executable>guarda</executable>
        <workingDirectory>${maven.multiModuleProjectDirectory}</workingDirectory>
        <arguments><argument>check</argument></arguments>
      </configuration>
    </execution>
  </executions>
</plugin>
"#;

/// Escreve o script e aplica no build da raiz. Devolve os arquivos escritos.
pub fn gradle(raiz: &Path) -> Result<Vec<String>> {
    let existe = |n: &str| raiz.join(n).is_file();
    let kts = existe("build.gradle.kts") || existe("settings.gradle.kts");
    let groovy = existe("build.gradle") || existe("settings.gradle");
    if !kts && !groovy {
        bail!("sem build.gradle(.kts) nem settings.gradle(.kts) na raiz");
    }
    let (script, conteudo, build, linha) = if kts {
        (
            "gradle/guarda.gradle.kts",
            GRADLE_KTS,
            "build.gradle.kts",
            "apply(from = \"gradle/guarda.gradle.kts\")",
        )
    } else {
        (
            "gradle/guarda.gradle",
            GRADLE_GROOVY,
            "build.gradle",
            "apply from: 'gradle/guarda.gradle'",
        )
    };
    std::fs::create_dir_all(raiz.join("gradle"))?;
    std::fs::write(raiz.join(script), conteudo)?;
    aplicar(&raiz.join(build), linha)?;
    Ok(vec![script.to_string(), build.to_string()])
}

/// Troca (ou acrescenta no fim) o bloco da guarda no build da raiz.
fn aplicar(caminho: &Path, linha: &str) -> Result<()> {
    let atual = std::fs::read_to_string(caminho).unwrap_or_default();
    let bloco = format!("{INICIO}\n{linha}\n{FIM}\n");
    let novo = match (atual.find(INICIO), atual.find(FIM)) {
        (Some(i), Some(f)) if f > i => {
            let fim = f + FIM.len();
            let resto = atual[fim..].strip_prefix('\n').unwrap_or(&atual[fim..]);
            format!("{}{bloco}{resto}", &atual[..i])
        }
        (None, None) if atual.trim().is_empty() => bloco,
        (None, None) => format!("{}\n\n{bloco}", atual.trim_end()),
        _ => bail!(
            "{}: marcador da guarda incompleto; conserte à mão",
            caminho.display()
        ),
    };
    std::fs::write(caminho, novo)?;
    Ok(())
}

#[cfg(test)]
mod casos {
    use super::*;

    #[test]
    fn gradle_kts_aplica_uma_vez() {
        let d = tempfile::tempdir().unwrap();
        let build = d.path().join("build.gradle.kts");
        std::fs::write(&build, "plugins { java }\n").unwrap();
        let escritos = gradle(d.path()).unwrap();
        assert_eq!(escritos, ["gradle/guarda.gradle.kts", "build.gradle.kts"]);
        gradle(d.path()).unwrap();
        let t = std::fs::read_to_string(&build).unwrap();
        assert!(t.starts_with("plugins { java }\n\n// guarda:inicio"), "{t}");
        assert_eq!(t.matches("apply(from").count(), 1, "{t}");
        let s = std::fs::read_to_string(d.path().join("gradle/guarda.gradle.kts")).unwrap();
        assert!(s.contains("tasks.register<Exec>(\"guardaCheck\")"));
    }

    #[test]
    fn gradle_groovy_e_sem_build() {
        let d = tempfile::tempdir().unwrap();
        assert!(gradle(d.path()).is_err());
        std::fs::write(d.path().join("settings.gradle"), "rootProject.name = 'x'\n").unwrap();
        gradle(d.path()).unwrap();
        let t = std::fs::read_to_string(d.path().join("build.gradle")).unwrap();
        assert!(t.contains("apply from: 'gradle/guarda.gradle'"), "{t}");
    }
}
