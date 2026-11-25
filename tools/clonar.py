"""Clona (raso) os repositórios Java/Kotlin de uma conta do GitHub para montar o corpus.

Uso:
    python tools/clonar.py DONO DESTINO [--limite 200] [--com-forks]

Precisa do `gh` autenticado. DESTINO deve ficar fora deste repositório (ex.: a pasta
temporária da sessão). Repositório já clonado é pulado; nada é enviado para o GitHub.
"""

import argparse
import json
import os
import subprocess


def main():
    a = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    a.add_argument("dono")
    a.add_argument("destino")
    a.add_argument("--limite", type=int, default=200)
    a.add_argument("--com-forks", action="store_true")
    o = a.parse_args()
    r = subprocess.run(
        ["gh", "repo", "list", o.dono, "--limit", str(o.limite),
         "--json", "name,primaryLanguage,isFork,url"],
        capture_output=True, text=True, encoding="utf-8", check=True,
    )
    os.makedirs(o.destino, exist_ok=True)
    for repo in json.loads(r.stdout):
        ling = (repo.get("primaryLanguage") or {}).get("name")
        if ling not in ("Java", "Kotlin") or (repo["isFork"] and not o.com_forks):
            continue
        d = os.path.join(o.destino, repo["name"])
        if os.path.isdir(d):
            print("já existe", repo["name"])
            continue
        print("clonando", repo["name"], ling)
        subprocess.run(["git", "clone", "--depth", "1", "-q", repo["url"], d], check=False)


if __name__ == "__main__":
    main()
