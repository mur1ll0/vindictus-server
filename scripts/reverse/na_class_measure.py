"""Passo 0 do porte de classe (skill new-class-port): mede uma classe do NA contra o 1.69.

Só leitura. Relatórios:
  db      linhas com o nome da classe em cada tabela do heroes.db3 do NA e do conteúdo do servidor 1.69;
  models  versão IDST dos .mdl da classe no NA e censo das versões no 1.69;
  tokens  chaves de quadro, eventos AE_*, condições e comandos AE_CUSTOM_MOTION dos scripts da classe
          que não aparecem nos DLLs 1.69 (*.pre-routing.bak) nem nos scripts do 1.69.

Uso: python -s -E scripts/reverse/na_class_measure.py --class miri --weapon draker {db,models,tokens}
"""
from __future__ import annotations

import argparse
import collections
import glob
import os
import re
import sqlite3
import struct
import sys

NA_DB = ".cache/na-db/heroes-plain.db3"
SERVER_DB = "../ServidorVindictus/Server/zh-tw-x-gm/Bin/heroesContents.db3"
NA_ROOT = "F:/Games/vindictus/hfsExtracted"
LAB_ROOT = ".cache/play-client/hfs_extracted"
LAB_BIN = ".cache/play-client/bin"

TOKEN_PATTERNS = {
    "frame_key": re.compile(r"\$[A-Za-z_][A-Za-z0-9_]*\$"),
    "ae": re.compile(r"\bAE_[A-Z0-9_]+\b"),
    "cond": re.compile(r"\bA[Ss]_Cond_[A-Za-z0-9_]+\b"),
    "act": re.compile(r"\bACT_[A-Z0-9_]+\b"),
    "custom_cmd": re.compile(r"AE_CUSTOM_MOTION\s+\"?([A-Za-z_][A-Za-z0-9_]*)"),
}


def read_only(path: str) -> sqlite3.Connection:
    return sqlite3.connect(f"file:{path}?mode=ro", uri=True)


def table_counts(con: sqlite3.Connection, needle: str) -> dict[str, int]:
    counts = {}
    tables = [r[0] for r in con.execute("select name from sqlite_master where type='table' order by name")]
    for table in tables:
        columns = [r[1] for r in con.execute(f'pragma table_info("{table}")')]
        if not columns:
            continue
        cond = " or ".join(f'lower(cast("{c}" as text)) like ?' for c in columns)
        count = con.execute(f'select count(*) from "{table}" where {cond}', [f"%{needle}%"] * len(columns)).fetchone()[0]
        if count:
            counts[table] = count
    return counts


def report_db(args: argparse.Namespace) -> None:
    for label, path in (("NA", NA_DB), ("1.69", SERVER_DB)):
        counts = table_counts(read_only(path), args.class_name)
        print(f"== {label}: {len(counts)} tabelas, {sum(counts.values())} linhas com '{args.class_name}'")
        for table, count in counts.items():
            print(f"   {table}: {count}")
    na = read_only(NA_DB)
    rows = na.execute(
        "select e.EquipClass, count(*) from EquipItemInfo e join ItemClassInfo i on i.ItemClass = e.ItemClass "
        "where lower(i.ClassRestriction) like ? and i.Category = 'WEAPON' group by 1",
        [f"%{args.class_name}%"],
    ).fetchall()
    print("== NA tipos de arma:", rows)


def mdl_version(path: str):
    try:
        with open(path, "rb") as f:
            head = f.read(8)
    except OSError:
        return None
    if len(head) == 8 and head[:4] == b"IDST":
        return struct.unpack_from("<i", head, 4)[0]
    return head[:4].hex()


def report_models(args: argparse.Namespace) -> None:
    names = (args.class_name, args.weapon)
    per_dir = collections.Counter()
    for dirpath, _dirs, files in os.walk(NA_ROOT):
        for name in files:
            rel = os.path.relpath(os.path.join(dirpath, name), NA_ROOT).replace("\\", "/")
            if name.lower().endswith(".mdl") and any(n in rel.lower() for n in names):
                per_dir[("/".join(rel.split("/")[:3]), mdl_version(os.path.join(dirpath, name)))] += 1
    print(f"== NA .mdl de {names}:")
    for key, count in sorted(per_dir.items(), key=lambda kv: str(kv[0])):
        print(f"   {key[0]} v{key[1]}: {count}")
    lab = collections.Counter()
    for dirpath, _dirs, files in os.walk(LAB_ROOT):
        for name in files:
            if name.lower().endswith(".mdl"):
                lab[mdl_version(os.path.join(dirpath, name))] += 1
    print("== 1.69 censo das versões:", dict(lab))


def read_text(path: str) -> str:
    with open(path, "rb") as f:
        return f.read().decode("latin-1")


def report_tokens(args: argparse.Namespace) -> None:
    c, w = args.class_name, args.weapon
    scripts = [
        f"models/player/_anim/{args.body}/{c}/pc_{c}_motion.txt",
        f"models/player/_anim/{args.body}/{c}/pc_{c}_motion_ex.txt",
        f"models/player/_anim/{args.body}/{c}/pc_{c}_town.txt",
        f"models/player/_anim/{args.body}/{c}/pc_{c}_motion_seqGroup.txt",
        f"models/player/_anim/{args.body}/{c}/pc_{c}_motion_ex_seqGroup.txt",
        f"scripts/actionstate/player_action_state_{c}.txt",
        f"scripts/actionstate/player_action_state_{c}_ex.txt",
        f"scripts/weapon_{w}.txt",
    ]
    blobs = []
    for dll in ("client.dll", "server.dll", "engine.dll"):
        backups = sorted(glob.glob(os.path.join(LAB_BIN, f"{dll}.*.pre-routing.bak")))
        blobs.append(open(backups[0] if backups else os.path.join(LAB_BIN, dll), "rb").read().lower())
    lab_text = "\n".join(
        read_text(p)
        for pattern in ("scripts/**/*.txt", "models/player/_anim/**/*.txt")
        for p in glob.glob(os.path.join(LAB_ROOT, pattern), recursive=True)
    )
    found = {k: set() for k in TOKEN_PATTERNS}
    for rel in scripts:
        path = os.path.join(NA_ROOT, rel)
        if not os.path.exists(path):
            print("ausente:", rel)
            continue
        text = read_text(path)
        for kind, rx in TOKEN_PATTERNS.items():
            found[kind].update(m.group(1) if m.groups() else m.group(0) for m in rx.finditer(text))
    for kind, tokens in found.items():
        missing = []
        for token in sorted(tokens):
            if any(token.lower().encode() in blob for blob in blobs):
                continue
            missing.append(token + (" (só em scripts 1.69)" if token in lab_text else ""))
        print(f"== {kind}: {len(tokens)} distintos; ausentes nos DLLs 1.69: {len(missing)}")
        for token in missing:
            print("   ", token)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--class", dest="class_name", required=True)
    parser.add_argument("--weapon", required=True)
    parser.add_argument("--body", default="female")
    parser.add_argument("report", choices=("db", "models", "tokens"))
    args = parser.parse_args()
    {"db": report_db, "models": report_models, "tokens": report_tokens}[args.report](args)
    sys.stdout.flush()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
