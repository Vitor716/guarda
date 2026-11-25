"""Roda a guarda num corpus de projetos e agrega os achados por regra.

Uso:
    python tools/medir.py CORPUS [CORPUS...] [--exe CAMINHO] [--regras SEC001,SML011]
                          [--saida achados.json] [--comparar antes.json]

CORPUS é uma pasta cujas subpastas são projetos (ex.: os clones de tools/clonar.py ou
cópias dos fontes). Cada projeto roda com `check --sem-baseline --formato json`, então
nada é escrito nele além do cache em `.guarda/cache/`. Use sempre cópias, nunca o original.

A saída JSON é uma lista de [projeto, arquivo, linha, codigo, mensagem], onde `projeto` é o
caminho absoluto: é a entrada do tools/ver_achados.py. Com --comparar, imprime a tabela
"Regra | Antes | Depois" para colar no docs/fase-N.md.
"""

import argparse
import collections
import json
import os
import subprocess
import sys

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXE = os.path.join(RAIZ, "target", "release", "guarda" + (".exe" if os.name == "nt" else ""))


def projetos(corpus):
    for c in corpus:
        for n in sorted(os.listdir(c)):
            d = os.path.join(c, n)
            if os.path.isdir(d) and not n.startswith("."):
                yield os.path.abspath(d)


def medir(exe, d):
    r = subprocess.run(
        [exe, "check", "--sem-baseline", "--formato", "json", "--max", "100000"],
        cwd=d, capture_output=True, text=True, encoding="utf-8",
    )
    if r.returncode == 2:
        print(f"  erro em {d}: {r.stderr.strip()[:200]}", file=sys.stderr)
    for linha in r.stdout.splitlines():
        try:
            v = json.loads(linha)
        except ValueError:
            continue
        if "codigo" in v:
            yield v


def main():
    a = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    a.add_argument("corpus", nargs="+")
    a.add_argument("--exe", default=EXE)
    a.add_argument("--regras", help="códigos separados por vírgula (padrão: todos)")
    a.add_argument("--saida", default="achados.json")
    a.add_argument("--comparar", help="JSON de uma medição anterior")
    o = a.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")
    filtro = set(o.regras.split(",")) if o.regras else None

    total = collections.Counter()
    achados = []
    for d in projetos(o.corpus):
        c = collections.Counter()
        for v in medir(o.exe, d):
            if filtro and v["codigo"] not in filtro:
                continue
            c[v["codigo"]] += 1
            achados.append([d, v["arquivo"], v["linha"], v["codigo"], v["mensagem"]])
        total.update(c)
        if c:
            print(f"{os.path.basename(d):45} " + " ".join(f"{k}={n}" for k, n in sorted(c.items())))
    print("TOTAL", " ".join(f"{k}={n}" for k, n in sorted(total.items())))
    if total.get("ERR000"):
        print("atenção: ERR000 > 0 (arquivo que o parser não leu)")
    with open(o.saida, "w", encoding="utf-8") as f:
        json.dump(achados, f, ensure_ascii=False)

    if o.comparar:
        with open(o.comparar, encoding="utf-8") as f:
            antes = collections.Counter(x[3] for x in json.load(f))
        print("\n| Regra | Antes | Depois |\n| --- | --- | --- |")
        for k in sorted(set(antes) | set(total)):
            if not filtro or k in filtro:
                marca = "**" if antes[k] != total[k] else ""
                print(f"| {k} | {antes[k]} | {marca}{total[k]}{marca} |")


if __name__ == "__main__":
    main()
