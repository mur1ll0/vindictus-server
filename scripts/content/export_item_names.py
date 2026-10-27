"""English names for the admin panel (B4.3): items (name and description), skills and status
effects, from the client's english_eu localization (heroes_text_english_eu.txt), keyed like the
client looks them up: HEROES_ITEM_NAME_/HEROES_ITEM_DESC_<CLASS>, HEROES_SKILL_NAME_<SKILL> and
HEROES_STATUS_NAME_<TYPE>, with the key in upper case. Text the EU localization never translated
(Korean/CJK) is left out, as in export_cash_shop.py. Read-only; no game state."""
import argparse, json, pathlib, re
from export_cash_shop import LOCALIZED, UNTRANSLATED
from export_item_catalog import digest
import content_additions

ROOT = pathlib.Path(__file__).resolve().parents[2]
KEY = re.compile(r'^[A-Z0-9_]{1,96}$')
ENTRY = re.compile(r'^\s*"(HEROES_(?:ITEM_NAME|ITEM_DESC|SKILL_NAME|STATUS_NAME)_[^"]+)"\s+"(.*)"\s*$', re.M)
PREFIXES = {'items': 'HEROES_ITEM_NAME_', 'skills': 'HEROES_SKILL_NAME_', 'status_effects': 'HEROES_STATUS_NAME_'}


def usable(text, limit):
    return text is not None and len(text) <= limit and not UNTRANSLATED.search(text)


def localized_entries(path):
    data = path.read_bytes()
    if len(data) > 64 * 1024 * 1024 or data[:2] != b'\xff\xfe':
        raise ValueError('Localized text must be bounded UTF-16LE')
    return dict(ENTRY.findall(data.decode('utf-16')))


def read_package(localized_path=LOCALIZED, additions=None, overrides=None):
    # The laboratory and panel items' text (item_additions.json, the item editor's overlay) is what
    # the client copy gets too; it goes with the client's own localization only.
    if additions is None:
        additions = content_additions.load() if localized_path == LOCALIZED else []
    if overrides is None:
        overrides = content_additions.load_overrides() if localized_path == LOCALIZED else {}
    entries = {**localized_entries(localized_path), **content_additions.text_entries(additions, overrides)}
    package = dict(format_version=1, source='heroes_text_english_eu', localized_sha256=digest(localized_path))
    for kind, prefix in PREFIXES.items():
        names = {}
        for key, name in entries.items():
            if not key.startswith(prefix):
                continue
            short = key[len(prefix):]
            if not KEY.match(short) or not usable(name, 256):
                continue
            if kind == 'items':
                desc = entries.get('HEROES_ITEM_DESC_' + short)
                desc = desc.replace('\\n', '\n') if desc is not None else None
                names[short] = [name, desc if usable(desc, 4096) else None]
            else:
                names[short] = name
        package[kind] = dict(sorted(names.items()))
    if digest(localized_path) != package['localized_sha256']:
        raise ValueError('Localized text changed during export')
    return package


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    output = pathlib.Path(args.output).resolve()
    if output.parent != (ROOT / '.cache/private-content').resolve():
        raise ValueError('Private output outside ACL cache')
    package = read_package()
    output.write_text(json.dumps(package, ensure_ascii=True, separators=(',', ':')), encoding='utf-8', newline='\n')
    print('English names exported: %d items, %d skills, %d status effects'
          % (len(package['items']), len(package['skills']), len(package['status_effects'])))


if __name__ == '__main__':
    main()
