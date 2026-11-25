# Segurança

## Versões com suporte

Só a versão mais recente recebe correções. Atualize com o binário da última release ou
`cargo install --path . --locked` a partir da `main`.

## Como relatar uma vulnerabilidade

**Não abra issue.** Fale direto com o mantenedor (Vitor716 no GitHub) por um canal privado.
Inclua a versão (`guarda --version`), o sistema operacional e os passos para reproduzir.
Achados que são só falso positivo ou falso negativo de uma regra `SEC*` não são
vulnerabilidade da guarda: use o modelo de issue *Falso positivo* ou *Nova regra*.

## O que a guarda faz (e não faz)

- **Roda offline.** Não abre conexão de rede, não envia código nem telemetria.
- **Lê** os fontes, o `guarda.yml`, os XML de cobertura e o `git diff`. **Escreve** só no
  projeto analisado: `.guarda/` (baseline, schema e cache) e, quando você pede `init` ou
  `integrar`, os arquivos de integração (hooks, `AGENTS.md`, `CLAUDE.md`, build).
- `check --cobertura` executa os testes do projeto pelo Gradle ou Maven dele: é código do
  próprio projeto, rode só em repositório confiável.
- As regras `SEC*` apontam **pontos de atenção** encontrados pela sintaxe. Elas não
  substituem revisão de segurança, SAST completo nem pentest. A guarda nunca imprime o valor de
  um segredo encontrado, só o nome da chave.
