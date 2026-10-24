"""Original gameplay values over the server's modified heroesContents.db3.

The server content (ServidorVindictus/Server/zh-tw-x-gm/Bin/heroesContents.db3) was
edited by its previous owners: every quest drops 20 items, every title goal needs a
single action, quests open at level 1, stacks hold 1000. The TutorialRagezone server
database keeps the original values for the same rows (same keys, same other columns).
Exporters keep the server database as their bound source (its hash stays in every
manifest) and read these columns from the original database instead.

The table/column list below is also the list of tuning points for the admin panel
(docs/content-tuning.md).
"""
import contextlib, hashlib, pathlib, sqlite3

ROOT = pathlib.Path(__file__).resolve().parents[2]
ORIGINAL_DATABASE = ROOT.parent / 'TutorialRagezone/DB/server/heroesContents.db3'
ORIGINAL_SHA256 = '5d367666fd006ffd670e1fc85cfeb087006c3fa89522d108fb60a95925055781'

# table -> (key columns, original columns)
COLUMNS = {
    'QuestInfo': (('ID',), ('DropItemCountMin', 'DropItemCountMax', 'MaxPlayCount', 'QuestSet', 'DisableUserShip', 'EnableUserDSMode')),
    'QuestDifficultyInfo': (('QuestID', 'Difficulty'), ('MinLevel',)),
    'StoryLineInfo': (('StoryLineID',), ('RewardExp',)),
    'TitleGoalInfo': (('ID',), ('TargetCount',)),
    'ItemClassInfo': (('ItemClass', 'Feature'), ('MaxStack', 'RequiredLevel', 'ExpireIn')),
    # The server content sets every enhance success ratio to 1.0.
    'EnhanceInfo': (('EnhanceType', 'EnhanceLevel', 'Feature'), ('SuccessRatio',)),
    'EnhanceServerInfo': (('EnhanceType', 'EnhanceLevel', 'Feature'), ('SuccessRatio',)),
}

_cache = {}


def _digest(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for block in iter(lambda: f.read(1 << 20), b''):
            h.update(block)
    return h.hexdigest()


def _original(table):
    if table not in _cache:
        if _digest(ORIGINAL_DATABASE) != ORIGINAL_SHA256:
            raise ValueError('Original content database changed')
        keys, columns = COLUMNS[table]
        with contextlib.closing(sqlite3.connect(ORIGINAL_DATABASE.resolve().as_uri() + '?mode=ro', uri=True)) as db:
            db.execute('PRAGMA query_only=ON')
            db.row_factory = sqlite3.Row
            values = {}
            for r in db.execute('Select * From [' + table + ']'):
                key = tuple(r[k] for k in keys)
                if key in values:
                    raise ValueError('Original content key repeated')
                values[key] = {c: r[c] for c in columns}
        _cache[table] = values
    return _cache[table]


def select(db, table):
    """`Select * From [table]` on the server database, in its provider order, as dict rows
    with the original columns applied (a row without an original counterpart is kept)."""
    keys, _ = COLUMNS[table]
    original = _original(table)
    cursor = db.execute('Select * From [' + table + ']')
    names = [c[0] for c in cursor.description]
    for row in cursor:
        r = dict(zip(names, row))
        r.update(original.get(tuple(r[k] for k in keys), {}))
        yield r
