"""Collaboration events of the content (Sword Art Online and Fairy Tail) for the EU laboratory.

Not in the original game: the EU service (EN-EU-X-GM) never ran these events. The content keeps
them as FeatureMatrix keys of other services (Event_sao_npc_jajp, Event_SAO_AllUserGoal_us...).
The operator asked (03/11/2026) for both collabs complete on the EU laboratory:

- the panel lists their keys in a "Collabs" filter, including the other regions' keys
  (marked "adaptado de JP/KR") and the keys only the NPC scripts use (NpcTalkInfo defaults and
  <feature> button conditions), which the SQL tables never name;
- a pack per collab switches on, in one published version, every key the whole event needs and
  switches off the other regions' copies of the same content (one set of collective goals);
- the collab outfits are wearable by every ported class whose body has a model for them
  (server and client copy). All 28 bits (05/11 first cut) left the tooltip without classes and
  the preview without a model on the other body (operator report of 05/11/2026): a piece now
  admits the classes whose body category (male, female, giant, tall) its player_costume.txt
  label has, and the accessories (no costume model) every ported class;
- every text the collabs show has English (collab_texts.json): the NA client's official English
  where it has the key, else a laboratory translation; applied only where the EU text has no
  line or a Korean/Japanese/Chinese one (client copy, panel names, cash shop, item names).

Evidence for the packs (heroesContents.db3 and Data/XML of the server, 03/11/2026):
- Event_sao_npc_jajp: NpcTalkInfo defaults Default_SAO_Kirito, _Asuna (t01_inn), _Lisbet (t01_bs).
- Event_SAO_AllUserGoal_enus: StoryLineInfo Event_SAO_allusergoal_enus (Kirito) and Kirito's button
  sao_allusergoal_enus_shop (storyline_cleared + feature) -> SAO_KIRITO_ENUS_Shop (no Feature).
- Event_SAO_AllUserGoal_Cat_Shop_enus: STRANGECAT button EVENT_SAO_ENUS_CAT_Shop (Default_Cat).
- Event_SAO_AllUserGoal_us: the 8 AllUserGoalInfo goals with the enUS texts (_aus repeats them,
  _jajp/_kokr/_cn/_cn_1 are the other services' copies); AllUserGoalEvent switches the system on.
- Event_SAO_asuna_cooking_jajp, Event_SAO_lisbet_blacksmith_jajp: the Asuna and Lisbeth stories
  (the _jajp ones have English text in the EU client; _kokr repeats Lisbeth's with Korean text).
- Fairy Tail: Event_Fairytail_npc (NPCs), _natsu/_rucy/_gray/_elsa/_makarov (stories) and
  Event_monsterbingo_fairytail (Makarov's bingo shop and story, monster drops).
"""
from __future__ import annotations

import json
import pathlib
import re
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / 'client'))
import kv_merge  # noqa: E402

# Collab name -> FeatureMatrix keys and item classes of the collab.
COLLABS = {
    'SAO': re.compile(r'(?:^|_)sao(?:_|$)', re.I),
    'Fairy Tail': re.compile(r'fairytail', re.I),
}
# Item classes of the collabs (outfits, Yui's heads, Lisbeth/Asuna inners...).
ITEMS = re.compile(r'sao|fairytail|kirito|asuna|lisbet|(?:^|_)yui_', re.I)
# Region tokens of the other services -> label shown by the panel ("adaptado de JP").
REGIONS = {'jajp': 'JP', 'kokr': 'KR', 'cn': 'CN', 'zhcn': 'CN', 'zhtw': 'TW', 'cntw': 'TW', 'thth': 'TH'}
# Equipment categories made wearable by every class (ItemClassInfo.Category).
WEARABLE = ('TUNIC', 'PANTS', 'GLOVES', 'BOOTS', 'HELM', 'AVATAR_TUNIC', 'AVATAR_PANTS', 'AVATAR_GLOVES',
            'AVATAR_BOOTS', 'AVATAR_HELM', 'INNERARMOR', 'NECKLACE', 'EPAULET', 'BODYPAINTING')
# Body category of player_costume.txt per ported class (BaseCharacter value): the category of the
# 1.69 single-class tunics (Lann/Kai/Hagie male, Karok giant, Hurk tall, the others female) and the
# Miri's NA entries (female).
CLASS_BODIES = {0: 'male', 1: 'female', 2: 'female', 3: 'giant', 4: 'male', 5: 'female', 6: 'tall', 7: 'female',
                8: 'female', 9: 'male', 10: 'female', 11: 'female'}
PORTED = sum(1 << c for c in CLASS_BODIES)
# ItemClassInfo.Category (AVATAR_ prefix removed) -> player_costume.txt section of its model.
COSTUME_SECTIONS = {'TUNIC': 'upper_body', 'PANTS': 'lower_body', 'GLOVES': 'hand', 'BOOTS': 'foot', 'HELM': 'head',
                    'INNERARMOR': 'inner_armor'}

PACKS = [
    dict(id='sao_eu', name='SAO completo (EU)', collab='SAO',
         enable=['AllUserGoalEvent', 'Event_sao_npc_jajp', 'Event_SAO_AllUserGoal_enus',
                 'Event_SAO_AllUserGoal_Cat_Shop_enus', 'Event_SAO_AllUserGoal_us',
                 'Event_SAO_asuna_cooking_jajp', 'Event_SAO_lisbet_blacksmith_jajp'],
         disable=['Event_SAO_AllUserGoal_aus', 'Event_SAO_AllUserGoal_jajp', 'Event_SAO_AllUserGoal_kokr',
                  'Event_SAO_AllUserGoal_cn', 'Event_SAO_AllUserGoal_cn_1', 'Event_SAO_AllUserGoal_Cat_Shop_jajp',
                  'Event_SAO_AllUserGoal_catshop_kokr', 'Event_SAO_lisbet_blacksmith_kokr']),
    dict(id='fairytail_eu', name='Fairy Tail completo (EU)', collab='Fairy Tail',
         enable=['Event_Fairytail_npc', 'Event_monsterbingo_fairytail', 'Event_Fairytail_makarov',
                 'Event_Fairytail_rucy', 'Event_Fairytail_natsu', 'Event_Fairytail_gray', 'Event_Fairytail_elsa'],
         disable=[]),
]


def collab(key: str) -> str | None:
    """The collab a FeatureMatrix key belongs to."""
    for name, pattern in COLLABS.items():
        if pattern.search(key):
            return name
    return None


def adapted(key: str) -> str | None:
    """The other service a collab key comes from (its region token), or None for an EU/US key."""
    for token in key.lower().split('_'):
        if token in REGIONS:
            return REGIONS[token]
    return None


def costume_bodies(costume: bytes) -> dict[tuple[str, str], set[str]]:
    """(section, entry key) -> body categories of the entry's label group in player_costume.txt."""
    groups: dict[tuple[str, str], set[str]] = {}
    labels: dict[tuple[str, str], str] = {}
    for section in kv_merge.parse(costume).children:
        if section.key not in COSTUME_SECTIONS.values() or not section.children:
            continue
        for entry in section.children:
            values = {c.key.lower(): c.value for c in entry.children or [] if c.value is not None}
            label = values.get('label', entry.key)
            labels[(section.key, entry.key)] = label
            if values.get('category'):
                groups.setdefault((section.key, label), set()).add(values['category'].lower())
    return {k: groups.get((k[0], label), set()) for k, label in labels.items()}


def unlock_masks(db, costume: bytes | None, classes: tuple[int, ...] | None = None) -> dict[str, int]:
    """Collab equipment -> ClassRestriction admitting every ported class whose body the piece has a
    model for (accessories: every ported class); only items whose mask grows. `classes` limits the ported
    classes to a client version's (scripts/client/class_layers.py: 0-9 and its new classes)."""
    class_bodies = {c: body for c, body in CLASS_BODIES.items() if classes is None or c in classes}
    ported = sum(1 << c for c in class_bodies)
    columns = {r[1] for r in db.execute('PRAGMA main.table_info([ItemClassInfo])')}
    if not {'ItemClass', 'Category', 'ClassRestriction'} <= columns:
        return {}
    bodies = costume_bodies(costume) if costume else None
    costume_types = {}
    if {'ItemClass', 'CostumeType'} <= {r[1] for r in db.execute('PRAGMA main.table_info([EquipItemInfo])')}:
        for item, kind in db.execute('SELECT ItemClass, CostumeType FROM main.[EquipItemInfo]'):
            costume_types.setdefault(item, kind)
    out: dict[str, int] = {}
    for item, category, mask in db.execute('SELECT ItemClass, Category, ClassRestriction FROM main.[ItemClassInfo]'):
        if not (isinstance(item, str) and ITEMS.search(item) and category in WEARABLE and type(mask) is int):
            continue
        section = COSTUME_SECTIONS.get(category.removeprefix('AVATAR_'))
        if section is None:
            allowed = ported
        elif bodies is None:
            continue
        else:
            kinds = bodies.get((section, str(costume_types.get(item))), set())
            allowed = sum(1 << c for c, body in class_bodies.items() if body in kinds)
        if mask | allowed != mask:
            out[item] = max(out.get(item, 0), mask | allowed)
    return out


def check_packs(keys: set[str]) -> list[dict]:
    """The packs of the collabs the content has, each key known to the events export (a content
    without the collab has no pack; one missing only some keys is a content change)."""
    out = []
    for pack in PACKS:
        named = pack['enable'] + pack['disable']
        missing = [k for k in named if k not in keys]
        if len(missing) == len(named):
            continue
        if missing or set(pack['enable']) & set(pack['disable']):
            raise ValueError(f'Collab pack {pack["id"]} names keys outside the events: {missing}')
        out.append(dict(pack))
    return out


TEXTS = pathlib.Path(__file__).resolve().parent / 'collab_texts.json'
TEXT_KEY = re.compile(r'^[A-Z][A-Z0-9_]{0,191}$')
# Hangul, CJK and kana (export_cash_shop.UNTRANSLATED).
UNTRANSLATED = re.compile('[ᄀ-ᇿ぀-ヿ㄰-㆏一-鿿가-힣]')
# Characters a token line cannot hold: the value is one quoted line of the localization file.
FORBIDDEN = ('"', '\t', '\r', '\n')


def texts(path: pathlib.Path = TEXTS) -> dict[str, str]:
    """collab_texts.json: the official NA English (`official_na`) and the laboratory translations
    (`lab`) by localization key, as the client file writes them (line breaks as the two characters
    backslash-n)."""
    data = json.loads(path.read_text(encoding='utf-8'))
    out: dict[str, str] = {}
    for section in ('official_na', 'lab'):
        for key, value in data.get(section, {}).items():
            if (not TEXT_KEY.match(key) or not isinstance(value, str) or not value.strip() or len(value) > 4096
                    or any(c in value for c in FORBIDDEN) or UNTRANSLATED.search(value) or key in out):
                raise ValueError(f'Collab text {key!r} must be one bounded English line without quotes')
            out[key] = value
    if len(out) > 5000:
        raise ValueError('Collab text bound')
    return out


def missing_in(entries: dict[str, str], collab_texts: dict[str, str] | None = None) -> dict[str, str]:
    """The collab texts whose key the localization `entries` lacks or keeps untranslated
    (keys compared without case, as the client looks them up)."""
    collab_texts = texts() if collab_texts is None else collab_texts
    english = {k.upper() for k, v in entries.items() if v and v.strip() and not UNTRANSLATED.search(v)}
    return {k: v for k, v in collab_texts.items() if k.upper() not in english}
