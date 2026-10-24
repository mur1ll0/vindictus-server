"""Align the StoryGoalInfo rows of a COPY of the 1.69 EN client with the server. Dry-run by default.

The client marks a storyline phase as deliverable ("?") from its own StoryGoalInfo
conditions, while the server completes the phase from its content. The 1.69 EN client
database ships a different revision of 11 server storylines (A Job for Shayla (1) asks the
client for red_gnoll_skin, the server and the phase text for gold_gnoll_skin). For every
storyline the server defines whose rows differ, the copy takes the server rows; storylines
only the client has are kept. A SQLite backup of the copy is written first.
"""
from __future__ import annotations
import argparse
import json
import sqlite3
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from prepare_lab_client import PreparationError, connect, reject_original  # noqa: E402

WORKSPACE = Path(__file__).resolve().parents[3]
SERVER_DATABASE = WORKSPACE / 'ServidorVindictus/Server/zh-tw-x-gm/Bin/heroesContents.db3'


def columns(db) -> list:
    return [c[1] for c in db.execute('PRAGMA table_info([StoryGoalInfo])')]


def story_rows(db, names: list) -> dict:
    out = {}
    for r in db.execute('Select ' + ','.join('[' + n + ']' for n in names) + ' From [StoryGoalInfo]'):
        out.setdefault(r[0], []).append(tuple(r))
    return out


def plan(server_db: Path, copy_db: Path) -> tuple:
    """The server column list and, per differing storyline, the rows the copy takes."""
    with connect(server_db.resolve().as_uri() + '?mode=ro', uri=True) as server, \
            connect(copy_db.resolve().as_uri() + '?mode=ro', uri=True) as copy:
        names = columns(server)
        if not names or names[0] != 'StoryLine' or names != columns(copy):
            raise PreparationError('StoryGoalInfo columns differ between server and client copy')
        wanted = story_rows(server, names)
        present = story_rows(copy, names)
    changes = {line: rows for line, rows in wanted.items()
               if Counter(rows) != Counter(present.get(line, []))}
    return names, changes


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument('--client-copy', required=True)
    parser.add_argument('--server-database', default=str(SERVER_DATABASE))
    parser.add_argument('--apply', action='store_true')
    args = parser.parse_args()
    root = reject_original(Path(args.client_copy))
    copy_db = root / 'sql' / 'heroes.db3'
    if not copy_db.is_file():
        raise PreparationError('Client copy has no sql/heroes.db3')
    names, changes = plan(Path(args.server_database), copy_db)
    if args.apply and changes:
        backup = copy_db.with_name('heroes.db3.pre-story-goals')
        if not backup.exists():
            with connect(copy_db) as source, connect(backup) as target:
                source.backup(target)
        insert = ('Insert Into [StoryGoalInfo](' + ','.join('[' + n + ']' for n in names)
                  + ') Values(' + ','.join('?' * len(names)) + ')')
        with connect(copy_db) as db:
            for line, rows in changes.items():
                db.execute('Delete From [StoryGoalInfo] Where [StoryLine]=?', (line,))
                db.executemany(insert, rows)
    print(json.dumps({'applied': bool(args.apply), 'storylines': sorted(changes)}))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
