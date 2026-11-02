"""Where the NA x64 DLLs keep the Miri's Flamebreath (read only; Miri M7h).

Flamebreath (skill_miri_dragon_energy) is an 8-level gauge her Fiery skills spend (USE_DRAGON_ENERGY,
As_Cond_Miri_Can_DragonSmash), shown by the scaleform movie MiriSkillDialog.gfx (SetGaugeLevel,
ShowEffect) and DragonEnergy.gfx (8 slots). The 1.69 has none of it. This probe lists, in the NA
client.dll and server.dll, the functions that reference those names (RIP-relative lea, as in
na_x64_draker_weapon.py), so the gauge's storage, recharge and HUD calls can be read without
disassembling the whole binary.

Usage: python -I scripts/reverse/na_x64_miri_dragon_energy.py [bin_x64 dir] [--dll client.dll|server.dll]
       [--dump RVA ...]
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import na_x64_delia_entity as delia  # noqa: E402
import na_x64_draker_weapon as draker  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / 'docs/evidence/client/na-x64-miri-dragon-energy-{dll}.json'
STRINGS = ('MiriSkillDialog', 'DragonEnergy', 'SetGaugeLevel', 'ShowEffect', 'USE_DRAGON_ENERGY',
           'REDUCE_DRAGON_KNIGHT_SP_GAUGE', 'skill_miri_dragon_energy', 'As_Cond_Miri_Can_DragonSmash',
           'As_Cond_Miri_DragonKnight', 'As_Cond_Miri_Can_Transform_DragonKnight', 'skill_miri_dragon_mark',
           'dragon_energy', 'DragonEnergyLevel', 'm_nDragonEnergy', 'm_iDragonEnergy', 'm_DragonEnergy',
           # Dragon Knight (Ancestral Garb, skill_miri_sp4): status effect type and look
           'dragonknight_transform', 'skill_miri_dragon_transform', 'EQUIP_DRAGON_KNIGHT_LOOK', 'PURGE_DRAGON_KNIGHT_LOOK',
           'EQUIP_DRAGON_KNIGHT_ARMOR', 'PURGE_DRAGON_KNIGHT_ARMOR', 'csf/metamorphosis/miri_dragon_knight.csf',
           'DragonEnergy_transform')


def probe(directory: Path, dll: str, dumps=()) -> dict:
    image = delia.Image(directory / dll)
    strings = {text: draker.string_rvas(image, text) for text in STRINGS}
    targets = {rva: text for text, rvas in strings.items() for rva in rvas}
    refs = draker.lea_references(image, targets)
    functions = {}
    for at, target in sorted(refs.items()):
        begin = draker.function_of(image, at)
        key = hex(begin) if begin is not None else 'none'
        entry = functions.setdefault(key, {'references': []})
        entry['references'].append({'at': hex(at), 'string': targets[target]})
    for key, entry in functions.items():
        if key != 'none':
            entry.update(image.summary(int(key, 16)))
    report = {'dll': dll, 'sha256': delia.sha256(directory / dll),
              'strings': {t: [hex(r) for r in rvas] for t, rvas in strings.items() if rvas}, 'functions': functions}
    for rva in dumps:
        report.setdefault('listings', {})[hex(rva)] = draker.listing(image, rva)
    return report


def main():
    argv = sys.argv[1:]
    dll = 'client.dll'
    if '--dll' in argv:
        dll = argv[argv.index('--dll') + 1]
        del argv[argv.index('--dll'):argv.index('--dll') + 2]
    dumps = []
    if '--dump' in argv:
        dumps = [int(a, 16) for a in argv[argv.index('--dump') + 1:]]
        argv = argv[:argv.index('--dump')]
    directory = Path(argv[0]) if argv else ROOT / '.cache/bin_x64'
    report = probe(directory, dll, dumps)
    out = Path(str(OUTPUT).format(dll=dll.split('.')[0]))
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=1), encoding='utf-8')
    print(json.dumps({k: v for k, v in report.items() if k != 'listings'}, indent=1)[:6000])
    for rva, lines in report.get('listings', {}).items():
        print(f'--- {rva}')
        print('\n'.join(lines))


if __name__ == '__main__':
    main()
