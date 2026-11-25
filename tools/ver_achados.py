"""Mostra cada achado de uma medição com o código em volta, para revisar falso positivo.

Uso:
    python tools/ver_achados.py achados.json SEC001,SML011 [--limite 30] [--antes 1] [--depois 2]
                                [--projeto nome]

Valores de segredo (password, senha, secret, token, key) aparecem mascarados: nunca cole
um segredo real numa doc, commit ou conversa.
"""

import argparse
import collections
import json
import os
import re
import sys

SEGREDO = re.compile(r"(?i)(password|passwd|pwd|senha|secret|token|key)(\w*\s*[=:]\s*)(\S+)")
URL = re.compile(r"(://[^:/@\s]+:)[^@\s]+@")


def ocultar(v):
    return v[:2] + "***" if len(v) > 6 else "***"


def mascarar(t):
    t = URL.sub(lambda m: m.group(1) + "***@", t)
    return SEGREDO.sub(lambda m: m.group(1) + m.group(2) + ocultar(m.group(3)), t)


def main():
    a = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    a.add_argument("achados")
    a.add_argument("regras", help="códigos separados por vírgula")
    a.add_argument("--limite", type=int, default=30, help="máximo por regra")
    a.add_argument("--antes", type=int, default=1)
    a.add_argument("--depois", type=int, default=2)
    a.add_argument("--projeto", help="só projetos cujo nome contém este texto")
    o = a.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")
    with open(o.achados, encoding="utf-8") as f:
        achados = json.load(f)

    for codigo in o.regras.split(","):
        xs = [x for x in achados if x[3] == codigo]
        if o.projeto:
            xs = [x for x in xs if o.projeto in os.path.basename(x[0])]
        por_projeto = collections.Counter(os.path.basename(x[0]) for x in xs)
        print(f"##### {codigo}: {len(xs)} achados em {len(por_projeto)} projetos "
              + ", ".join(f"{p}={n}" for p, n in por_projeto.most_common()))
        # Um de cada projeto primeiro, para a amostra cobrir o corpus todo.
        vistos, amostra, resto = set(), [], []
        for x in xs:
            (resto if x[0] in vistos else amostra).append(x)
            vistos.add(x[0])
        for projeto, arquivo, linha, cod, msg in (amostra + resto)[: o.limite]:
            print(f"===== {cod} {os.path.basename(projeto)} {arquivo}:{linha} | {msg}")
            try:
                with open(os.path.join(projeto, arquivo), encoding="utf-8", errors="replace") as f:
                    ls = f.read().splitlines()
            except OSError as e:
                print("  ", e)
                continue
            for i in range(max(0, linha - 1 - o.antes), min(len(ls), linha + o.depois)):
                print(f"{i + 1:5} {mascarar(ls[i].rstrip()[:160])}")


if __name__ == "__main__":
    main()
