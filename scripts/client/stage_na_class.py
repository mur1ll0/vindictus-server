"""Stage the files of an NA class for the laboratory client (skill new-class-port, step 1).

Copies, from the NA extraction (read-only), every file of the class and its weapon into a staging
folder laid out like the game, never versioned (.cache/private-assets/classes/<class>):
  - files under models/, materials/, effects/, expressions/, csf/, scaleform/, sound/ and scripts/
    with the class or weapon name in a folder or in the file name, minus systems the 1.69 lacks
    (relationship, NA start menu, hero path, Chinese/Japanese variants);
  - the materials and textures those models use that neither the staging nor the 1.69 has
    (closure over cdmaterials, texture names, VMT texture parameters and VMT includes);
  - the NA copy of each shared 1.69 script in SHARED_SCRIPTS (merge/<path>), from which the client
    copy gets only the class's lines (kv_merge.py);
NA studio models of version 45 are converted to 1.69's version 44 (mdl_v45_to_v44.py). Files that the
1.69 already has at the same path are never staged (the 1.69 copy wins). class-assets.json records
each file's source, the SHA-256 of the source and of the staged file, and what stayed unresolved;
scripts/client/apply_lab_content_additions.py packs it into the client copy.

  stage_na_class.py --class miri --weapon draker [--na-root F:/Games/vindictus/hfsExtracted]
                    [--lab-root .cache/play-client/hfs_extracted] [--out .cache/private-assets/classes/miri]
"""
from __future__ import annotations
import argparse
import hashlib
import json
import re
import struct
import sys
from pathlib import Path

import mdl_anim
import mdl_v45_to_v44
from model_kit import TEXTURE_PARAMS, model_materials

ROOT = Path(__file__).resolve().parents[2]
NA_ROOT = Path('F:/Games/vindictus/hfsExtracted')
LAB_ROOT = ROOT / '.cache/play-client/hfs_extracted'
STAGING = ROOT / '.cache/private-assets/classes'
MANIFEST = 'class-assets.json'
PREFIXES = ('models/', 'materials/', 'effects/', 'expressions/', 'csf/', 'scaleform/', 'sound/', 'scripts/')
EXCLUDE = re.compile(r'relationship|_externzhcn/|(^|/)jajp_|_cn\.bsh$|^models/npc/npc_start/|^models/objects/start/'
                     r'|^scripts/heropath/|^sound/voice/npc/|startmenu|_teaser')
INCLUDE_VMT = re.compile(r'"include"\s+"([^"]+)"', re.IGNORECASE)
NAME = re.compile(r'^[a-z]{2,16}$')
# Shared 1.69 scripts that get the class's lines (kv_merge.py), measured for Miri in
# docs/future/miri-class-port.md 2.5. Left out: NPC, pet and relationship scripts, the voice emote
# block of chat_commands.txt (every NA class) and the relationship precache of game_sounds_manifest.txt.
# English text of the NA: the class's tokens (names, descriptions, UI) go into the 1.69 text packs.
TEXT_SOURCE = 'resource/localized_text/english/heroes_text_english.txt'
TEXT_TARGET = 'text/heroes_text_english.txt'
SHARED_SCRIPTS = (
    'csf/bodyshape_fig_list.txt', 'scripts/action_table_player.txt', 'scripts/action_table_player_deathmatch.txt',
    'scripts/action_table_player_pmatch.txt', 'scripts/actionstate/common_state_etc.txt',
    'scripts/animating_area_attack.txt', 'scripts/body_layer.txt', 'scripts/face_layer.txt',
    'scripts/game_sounds_heroes_effect.txt', 'scripts/game_sounds_heroes_env.txt', 'scripts/game_sounds_heroes_monster.txt',
    'scripts/game_sounds_heroes_player.txt', 'scripts/game_sounds_heroes_weapons.txt', 'scripts/game_sounds_items.txt',
    'scripts/heroes_kb_act.lst', 'scripts/item_data.txt', 'scripts/player_costume.txt', 'scripts/projectile.txt',
    'scripts/skill/skill_actionstate_match_table.txt', 'scripts/translated_spskill.txt', 'scripts/weapon_meleebase.txt',
)


class StageError(ValueError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def selected(rel: str, names: tuple[str, ...]) -> bool:
    """rel: lower-case game path."""
    if not rel.startswith(PREFIXES) or EXCLUDE.search(rel):
        return False
    folders, _, file_name = rel.rpartition('/')
    return any(n in folders.split('/') or n in file_name for n in names)


def game_files(root: Path) -> dict[str, Path]:
    """Lower-case game path -> file, for every file under root."""
    return {p.relative_to(root).as_posix().lower(): p for p in root.rglob('*') if p.is_file()}


def include_models(mdl: bytes) -> list[str]:
    count, index = struct.unpack_from('<ii', mdl, 336)
    out = []
    for k in range(count if 0 < index < len(mdl) else 0):
        at = index + 8 * k
        name_at = at + struct.unpack_from('<i', mdl, at + 4)[0]
        out.append(mdl[name_at:mdl.index(b'\0', name_at)].decode('latin-1').replace('\\', '/').lower())
    return out


SCRIPT_INCLUDE = re.compile(rb'^[ \t]*"include"[ \t]+"([^"\r\n]+)"[^\r\n]*(?:\r?\n|$)', re.M)


def without_missing_includes(path: str, data: bytes, known=lambda p: False) -> bytes:
    """An action-state script without the "include" lines of files that neither the 1.69 nor the staging
    has (NA systems such as rope and interactive objects): the 1.69 loader drops the whole class file
    when an include is missing, and the class stays in its idle pose in battle (MI5)."""
    if not (path.startswith('scripts/actionstate/') and path.endswith('.txt')):
        return data
    return SCRIPT_INCLUDE.sub(lambda m: m.group(0) if known(m.group(1).decode('latin-1').lower()) else b'', data)


def sequence_count(mdl: bytes) -> int:
    return struct.unpack_from('<i', mdl, 0xbc)[0] if mdl[:4] == b'IDST' and len(mdl) >= 0xc0 else -1


def extreme_motion_sibling(path: str, data: bytes) -> str | None:
    """The full battle motion model an NA *_battle_motion_ex.mdl stub stands for. The 1.69 lab runs
    with the ExtremeMode feature, which loads the _ex motion model (every 1.69 class has a full one,
    475-639 sequences); the NA dropped the mode and left a one-sequence stub, so the class froze in
    its reference pose in battle (MI7)."""
    if path.endswith('_battle_motion_ex.mdl') and 0 <= sequence_count(data) <= 1:
        return path[:-len('_ex.mdl')] + '.mdl'
    return None


# 1.69 battle motion model of the same skeleton that the copied _ex includes when it has no battle
# jump: the Miri's NA battle model has none (jump_begin/during/end, jump_attack), and she slid in the air
# without animation (C4 MI8). The Delia's is the closest female set; her events file already names the
# sequences. The engine takes from an included model only the sequences the model lacks.
JUMP_SEQUENCE = 'jump_begin'
JUMP_DONORS = {'female': 'models/player/_anim/female/delia/pc_delia_battle_motion_ex.mdl'}


def jump_donor(path: str, data: bytes, known=lambda p: False) -> str | None:
    """The 1.69 model a battle motion _ex model must include for the battle jump, or None."""
    if not path.endswith('_battle_motion_ex.mdl') or not path.startswith('models/player/_anim/'):
        return None
    donor = JUMP_DONORS.get(path.split('/')[3])
    if donor is None or not known(donor):
        return None
    try:
        if JUMP_SEQUENCE in {s.lower() for s in mdl_anim.Model(data).sequences}:
            return None
    except mdl_anim.ModelError:
        return None
    return donor


# Battle rest sequences in which the NA hangs the Draker from the chain on the left arm (C_WeaponDraker:
# draker_chain.mdl and a cable to the weapon's chain_01, native code the 1.69 lacks): without the chain
# the weapon floated beside her (C4 MI8). Until the chain is ported, the copied _ex keeps the weapon in
# the hand there, as every 1.69 class does (the hand offset of battle_idle_lower). The attacks and the
# throw still swing it on the (invisible) chain.
REST_SEQUENCES = ('battle_idle2rest', 'battle_idle2restb', 'battle_rest', 'battle_restb')
HAND = 'ValveBiped.Bip01_R_Hand'
HAND_REFERENCE = 'battle_idle_lower'


def rest_in_hand(data: bytes) -> tuple[bytes, list[str]]:
    """data with the weapon bone pinned to the hand in each REST_SEQUENCES that takes it off the hand."""
    pinned = []
    for sequence in REST_SEQUENCES:
        try:
            if max(mdl_anim.max_drift(data, sequence, mdl_anim.WEAPON_BONE, HAND, HAND_REFERENCE)) < mdl_anim.PINNED:
                continue
        except mdl_anim.ModelError:
            continue
        data = mdl_anim.pin_bone(data, sequence, mdl_anim.WEAPON_BONE, HAND, HAND_REFERENCE)
        pinned.append(sequence)
    return data, pinned


def town_motion(path: str, data: bytes) -> bytes:
    """A town motion model with the weapon kept on the back in jump_town (mdl_anim.town_jump_on_back):
    the Miri's, like the Delia's, carried it into her hand (C4 MI7, MI8)."""
    if path.startswith('models/player/_anim/') and path.endswith('_town_motion.mdl'):
        return mdl_anim.town_jump_on_back(data) or data
    return data


def dependencies(path: str, data: bytes, known=lambda p: False) -> list[str]:
    """Game paths a staged model or material needs. A model's material is the first cdmaterials folder
    that has it, as the engine searches them (`known`: the path exists in the 1.69, the NA or the
    staging); a material no folder has is reported in its first folder."""
    if path.endswith('.mdl') and data[:4] == b'IDST':
        textures, folders = model_materials(data)
        wanted = []
        for texture in textures:
            paths = ['materials/' + (folder + texture).replace('\\', '/').lower().strip('/') + '.vmt' for folder in folders]
            wanted.append(next((p for p in paths if known(p)), paths[0] if paths else None))
        return [w for w in wanted if w] + [m if m.startswith('models/') else 'models/' + m for m in include_models(data)]
    if path.endswith('.vmt'):
        text = data.decode('latin-1')
        wanted = ['materials/' + name.replace('\\', '/').lower().strip('/') + '.vtf'
                  for _, name in TEXTURE_PARAMS.findall(text) if name]
        return wanted + [name.replace('\\', '/').lower() for name in INCLUDE_VMT.findall(text)]
    return []


def stage(class_name: str, weapon: str, na_root: Path, lab_root: Path, out: Path) -> dict:
    for name in (class_name, weapon):
        if not NAME.match(name):
            raise StageError(f'Class and weapon names must match {NAME.pattern}.')
    na = game_files(na_root)
    lab = set(game_files(lab_root))
    names = (class_name, weapon)
    queue = sorted(p for p in na if selected(p, names) and p not in lab)
    records: dict[str, dict] = {}
    missing: set[str] = set()
    while queue:
        rel = queue.pop()
        if rel in records:
            continue
        source = na[rel].read_bytes()
        data = source
        converted = mdl_v45_to_v44.is_v45(source)
        if converted:
            data = mdl_v45_to_v44.convert(source)
        data = without_missing_includes(rel, data, lambda p: p in lab or (p in na and selected(p, names)))
        data = town_motion(rel, data)
        converted = converted or data != source
        target = out / rel
        if not target.is_file() or sha256(target.read_bytes()) != sha256(data):
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        records[rel] = {'path': rel, 'source': na[rel].relative_to(na_root).as_posix(), 'source_sha256': sha256(source),
                        'sha256': sha256(data), 'converted': converted, 'bytes': len(data)}
        for need in dependencies(rel, data, lambda p: p in lab or p in na):
            if need in records or need in lab:
                continue
            if need in na:
                queue.append(need)
            else:
                missing.add(need)
    for rel, record in list(records.items()):
        sibling = extreme_motion_sibling(rel, (out / rel).read_bytes())
        if sibling and sibling in records:
            data = (out / sibling).read_bytes()
            record.update({'copied_from': sibling})
            donor = jump_donor(rel, data, lambda p: p in lab)
            if donor:
                data = mdl_anim.with_included_model(data, donor)
                record['included'] = donor
            data, pinned = rest_in_hand(data)
            if pinned:
                record['weapon_in_hand'] = pinned
            (out / rel).write_bytes(data)
            record.update({'sha256': sha256(data), 'bytes': len(data), 'converted': True})
    merges = []
    for rel in SHARED_SCRIPTS:
        if rel not in na or rel not in lab:
            continue
        source = na[rel].read_bytes()
        target = out / 'merge' / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(source)
        merges.append({'path': rel, 'source': na[rel].relative_to(na_root).as_posix(), 'source_sha256': sha256(source),
                       'needles': list(names)})
    text = None
    if TEXT_SOURCE in na:
        source = na[TEXT_SOURCE].read_bytes()
        (out / 'merge/text').mkdir(parents=True, exist_ok=True)
        (out / 'merge' / TEXT_TARGET).write_bytes(source)
        text = {'path': TEXT_TARGET, 'source': na[TEXT_SOURCE].relative_to(na_root).as_posix(), 'source_sha256': sha256(source)}
    manifest = {'format_version': 1, 'class': class_name, 'weapon': weapon, 'source_root': str(na_root),
                'files': [records[k] for k in sorted(records)], 'merge': merges, 'text': text, 'missing': sorted(missing)}
    (out / MANIFEST).write_text(json.dumps(manifest, indent=1), encoding='utf-8')
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--class', dest='class_name', required=True)
    parser.add_argument('--weapon', required=True)
    parser.add_argument('--na-root', type=Path, default=NA_ROOT)
    parser.add_argument('--lab-root', type=Path, default=LAB_ROOT)
    parser.add_argument('--out', type=Path)
    args = parser.parse_args()
    out = args.out or STAGING / args.class_name
    try:
        manifest = stage(args.class_name, args.weapon, args.na_root, args.lab_root, out)
    except (OSError, StageError, mdl_v45_to_v44.ConversionError) as error:
        print(f'error: {error}', file=sys.stderr)
        return 1
    files = manifest['files']
    print(json.dumps({'out': str(out), 'files': len(files), 'converted': sum(f['converted'] for f in files),
                      'bytes': sum(f['bytes'] for f in files), 'merge': len(manifest['merge']),
                      'missing': len(manifest['missing'])}))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
