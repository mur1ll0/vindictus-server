"""Read-only probe of the NA x64 client.dll for the Draker weapon's own client code (Miri M7e).

The NA draws the Draker's chain in its own weapon entity, not with animation events: the client has
`C_WeaponDraker` (`DT_WeaponDraker`) next to the strings `draker_handle`, `cable/heroes_metalchain_05.vmt`,
`models/weapon/draker/draker_chain.mdl` and `anim_attachment_TOWN_WP`, and the generic weapon script
parser reads `TownAttachData`/`ExceptOwnerSequence`, `chain_material` and `chain_width`. The 1.69 has
none of them. This probe finds the RIP-relative references to those strings by a byte pattern (no
disassembly of the whole binary), names the function holding each one (exception table, as in
na_x64_delia_entity.py) and lists the strings and calls of those functions only.

Usage: python -I scripts/reverse/na_x64_draker_weapon.py [bin_x64 dir] [--dump RVA ...]
"""
import json
import re
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import na_x64_delia_entity as delia  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / 'docs/evidence/client/na-x64-draker-weapon.json'
STRINGS = ('CWeaponDraker', 'C_WeaponDraker', 'DT_WeaponDraker', 'draker_handle', 'cable/heroes_metalchain_05.vmt',
           'models/weapon/draker/draker_chain.mdl', 'anim_attachment_TOWN_WP', 'TownAttachData', 'ExceptOwnerSequence',
           'chain_material', 'chain_width', 'chain_01', 'chain_town', 'battle_rest')
# lea r64, [rip + disp32]
LEA = re.compile(rb'[\x48\x4c]\x8d[\x05\x0d\x15\x1d\x25\x2d\x35\x3d]', re.S)


def string_rvas(image, text):
    needle = text.encode() + b'\0'
    out, at = [], 0
    data = bytes(image.data)
    while True:
        at = data.find(needle, at)
        if at < 0:
            return out
        if at == 0 or data[at - 1] == 0:
            out.append(at)
        at += 1


def lea_references(image, targets):
    """rva of each `lea r64, [rip + X]` in code whose X is one of targets -> target."""
    data = bytes(image.data)
    found = {}
    for name, (va, size) in image.sections.items():
        if name not in ('.text', '.vmp0'):
            continue
        for m in LEA.finditer(data, va, va + size):
            at = m.start()
            target = at + 7 + struct.unpack_from('<i', data, at + 3)[0]
            if target in targets:
                found[at] = target
    return found


def function_of(image, rva):
    best = None
    for begin, end in image.functions.items():
        if begin <= rva < end and (best is None or begin > best):
            best = begin
    return best


def listing(image, rva, limit=400):
    out = []
    for instruction in image.body(rva)[:limit]:
        text = f'{instruction.address - image.base:#x} {instruction.mnemonic} {instruction.op_str}'
        target = image.rip_target(instruction)
        if target is not None and image.cstring(target):
            text += f'    ; "{image.cstring(target)}"'
        out.append(text)
    return out


def probe(directory, dumps=()):
    image = delia.Image(directory / 'client.dll')
    strings = {text: string_rvas(image, text) for text in STRINGS}
    targets = {rva: text for text, rvas in strings.items() for rva in rvas}
    refs = lea_references(image, targets)
    functions = {}
    for at, target in sorted(refs.items()):
        begin = function_of(image, at)
        key = hex(begin) if begin is not None else 'none'
        entry = functions.setdefault(key, {'references': []})
        entry['references'].append({'at': hex(at), 'string': targets[target]})
    for key, entry in functions.items():
        if key != 'none':
            entry.update(image.summary(int(key, 16)))
    report = {'client_sha256': delia.sha256(directory / 'client.dll'),
              'strings': {t: [hex(r) for r in rvas] for t, rvas in strings.items()},
              'functions': functions}
    for rva in dumps:
        report.setdefault('listings', {})[hex(rva)] = listing(image, rva)
    return report


def main():
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    dumps = []
    if '--dump' in sys.argv:
        dumps = [int(a, 16) for a in sys.argv[sys.argv.index('--dump') + 1:]]
        args = args[:len(args) - len(dumps)]
    directory = Path(args[0]) if args else ROOT / '.cache/bin_x64'
    report = probe(directory, dumps)
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(report, indent=1), encoding='utf-8')
    print(json.dumps({k: v for k, v in report.items() if k != 'listings'}, indent=1)[:6000])
    for rva, lines in report.get('listings', {}).items():
        print(f'--- {rva}')
        print('\n'.join(lines))


if __name__ == '__main__':
    main()
