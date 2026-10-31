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

import kv_merge
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


def extreme_scripts(model_path: str) -> dict[str, str]:
    """The _ex scripts that go with a battle motion _ex model copied from the full one (extreme_motion_sibling),
    each mapped to the full sibling it is staged from. The NA kept the class's _ex scripts as copies of
    another class's (the Miri's: 368 mentions of the Delia, none of herself), so with ExtremeMode the Miri
    played the Delia's moveset on her own animations: no SP skill, no Tab (Fiery Evasion), no Fiery
    skill (C4 MI12). Her full scripts are the moveset her model, her skill table and translated_spskill
    were made for."""
    parts = model_path.split('/')
    folder, name = '/'.join(parts[:-1]), parts[-1]
    klass = name[len('pc_'):-len('_battle_motion_ex.mdl')]
    return {f'scripts/actionstate/player_action_state_{klass}_ex.txt': f'scripts/actionstate/player_action_state_{klass}.txt',
            f'{folder}/pc_{klass}_motion_ex.txt': f'{folder}/pc_{klass}_motion.txt'}


def with_extreme_includes(script: bytes, known=lambda p: False) -> bytes:
    """An action-state script whose includes take the 1.69's _ex variant when there is one, as every 1.69
    _ex state file does: the 1.69's common_state_movement.txt has all its jump states commented out
    (they live in common_state_movement_ex.txt), and the Miri's full script, staged as her _ex
    (extreme_scripts), lost the town and battle jumps (C4 MI13)."""
    def swap(m):
        path = m.group(1).decode('latin-1')
        extreme = path[:-len('.txt')] + '_ex.txt'
        if not path.endswith('.txt') or path.endswith('_ex.txt') or not known(extreme.lower()):
            return m.group(0)
        return m.group(0).replace(m.group(1), extreme.encode('latin-1'))
    return SCRIPT_INCLUDE.sub(swap, script)


def event_blocks(script: bytes) -> dict[str, tuple[int, int]]:
    """Top-level blocks of a motion events script: lower-case sequence name -> (start, end) of
    `"name" { ... }`, braces counted outside quotes and // comments."""
    blocks, at, depth, start, name = {}, 0, 0, None, None
    while at < len(script):
        c = script[at:at + 1]
        if script.startswith(b'//', at):
            end = script.find(b'\n', at)
            at = len(script) if end < 0 else end
            continue
        if c == b'"':
            end = script.find(b'"', at + 1)
            if end < 0:
                break
            if depth == 0:
                start, name = at, script[at + 1:end].decode('latin-1').lower()
            at = end + 1
            continue
        if c == b'{':
            depth += 1
        elif c == b'}':
            depth -= 1
            if depth == 0 and name is not None:
                blocks.setdefault(name, (start, at + 1))
                name = None
        at += 1
    return blocks


JUMP_ACTIVATION = re.compile(rb'^[ \t]*"(?!event")[^"\r\n]*"[ \t]+"AE_ACTIVATE_STATE_CHANGE_CONDITION[ \t]+(Jump\w*)"[^\r\n]*(?:\r?\n)', re.M)


def with_jump_activations(script: bytes, donor_script: bytes) -> tuple[bytes, list[str]]:
    """The class's events script with the donor's jump activations (AE_ACTIVATE_STATE_CHANGE_CONDITION
    Jump*) in each block both scripts have and the class's lacks: the NA had no jump, so the Miri's own
    idle, run and rest never opened JumpBegin and Space did nothing in battle (C4 MI14)."""
    mine, theirs = event_blocks(script), event_blocks(donor_script)
    edits = []
    for name, (start, end) in mine.items():
        if name not in theirs:
            continue
        have = {m.group(1) for m in JUMP_ACTIVATION.finditer(script, start, end)}
        lines = [m.group(0) for m in JUMP_ACTIVATION.finditer(donor_script, *theirs[name]) if m.group(1) not in have]
        if lines:
            close = script.rfind(b'}', start, end)
            at = script.rfind(b'\n', start, close) + 1
            edits.append((at, b''.join(lines), name))
    for at, text, _ in sorted(edits, reverse=True):
        script = script[:at] + text + script[at:]
    return script, sorted(name for _, _, name in edits)


def with_included_events(script: bytes, donor_script: bytes, sequences: set[str]) -> tuple[bytes, list[str]]:
    """The class's events script plus the donor's event blocks of `sequences` (the ones an included model
    supplies) that the script lacks: the engine plays an included sequence with the events of the
    model's own script, so without them the battle jump would have no landing and the core search
    (E on a monster's drop, "GET") no Item_Loot (C4 MI12)."""
    mine, theirs = event_blocks(script), event_blocks(donor_script)
    added = sorted(s for s in sequences if s not in mine and s in theirs)
    if not added:
        return script, []
    newline = b'\r\n' if b'\r\n' in script else b'\n'
    pieces = [donor_script[theirs[s][0]:theirs[s][1]] for s in added]
    return script.rstrip() + newline + newline + (newline + newline).join(pieces) + newline, added


SPSKILL_TABLE = 'scripts/translated_spskill.txt'
# 1.69 scripts that ExtremeMode reads in place of a shared one, mapped to the NA script whose class lines
# they get: with ExtremeMode the 1.69 loads scripts/action_table_player_ex.txt (client and server), and
# the NA's _ex table has no Miri line, so her AE_SET_CUR_ACTION_INFO miri_* found no action: 37 damage a
# hit at 13k ATK and no SP from attacks (C4 MI15).
EXTREME_MERGES = {'scripts/action_table_player_ex.txt': 'scripts/action_table_player.txt'}
# States the 1.69's ExtremeMode gives every class and the NA's class files lack (the NA had no jump): the
# jump attacks of the "XE common" block, copied from the 1.69 donor's _ex state file (C4 MI15). The XE
# move itself (SuperPower) needs an XE skill the NA Miri does not have.
DONOR_STATES = {'female': ('scripts/actionstate/player_action_state_delia_ex.txt', ('JumpAttack', 'JumpAttackStrong'))}


def with_donor_states(script: bytes, donor_script: bytes, names: tuple[str, ...]) -> tuple[bytes, list[str]]:
    """The class's action-state script with the donor's `names` states, in each section
    (StateChangeConditionDefine, StateDefine) where the donor has them and the class does not."""
    mine = next(n for n in kv_merge.parse(script).children if n.block)
    theirs = next(n for n in kv_merge.parse(donor_script).children if n.block)
    newline = b'\r\n' if b'\r\n' in script else b'\n'
    edits, added = [], []
    for section in theirs.children:
        target = next((c for c in mine.children if c.block and c.key == section.key), None)
        if target is None or not section.block:
            continue
        have = {c.key for c in target.children}
        for state in section.children:
            if state.block and state.key in names and state.key not in have:
                edits.append((target.close, b'\t\t' + donor_script[state.start:state.end] + newline + b'\t'))
                added.append(f'{section.key}/{state.key}')
    for at, text in sorted(edits, key=lambda e: e[0], reverse=True):
        script = script[:at] + text + script[at:]
    return script, added


def spskill_for_169(na: bytes, lab: bytes, class_name: str) -> bytes:
    """The NA translated_spskill.txt with the class's section in the 1.69 shape. The NA nests each class's
    SP skills under a weapon block ("miri" { "0" { "skill_miri_sp1" {...} } }); the 1.69 lists them right
    under the class, so the merged Miri section held no skill and none of her SP skills could be used
    (C4 MI12). The section keeps the first weapon's skills whose key the 1.69's own table uses (the NA
    vocation entries take IN_SPSKILL12-17, keys the 1.69 lacks)."""
    keys = {n.value for n in _walk(kv_merge.parse(lab)) if not n.block and n.key.lower() == 'key'}
    section = next((n for n in kv_merge.parse(na).children if n.block and n.key.lower() == class_name), None)
    if section is None or not section.children or not all(c.block and c.key.isdigit() for c in section.children):
        return na
    weapon = next(c for c in section.children if c.key == '0') if any(c.key == '0' for c in section.children) else section.children[0]
    kept = [c for c in weapon.children if c.block
            and any(not k.block and k.key.lower() == 'key' and k.value in keys for k in c.children)]
    newline = b'\r\n' if b'\r\n' in na else b'\n'
    body = b''.join(b'\t' + na[c.start:c.end] + newline + newline for c in kept)
    flat = b'"' + section.key.encode('latin-1') + b'"' + newline + b'{' + newline + body + b'}'
    return na[:section.start] + flat + na[section.end:]


def _walk(node):
    for child in node.children:
        yield child
        yield from _walk(child)


# Files the NA's native code of a weapon names, which no name selection finds: the Draker's chain
# cable (C_WeaponDraker, ported in patch_class_routing.py CHAIN).
NATIVE_ASSETS = {'draker': ('materials/cable/heroes_metalchain_05.vmt',)}


def town_motion(path: str, data: bytes, donor: bytes | None = None) -> bytes:
    """A town motion model with the 1.69 Fiona's free-hands jump_town and the weapon kept on the back
    (mdl_anim.town_jump_on_back): the Miri's jump_town is the Delia's, in the combat pose, and carried the
    weapon into her hands (C4 MI7, MI8, MI11). The NA has no town jump (its movement states are commented
    out); the operator chose the Fiona's until an EU client can be compared."""
    if path.startswith('models/player/_anim/') and path.endswith('_town_motion.mdl'):
        return mdl_anim.town_jump_on_back(data, donor) or data
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
    lab_files = game_files(lab_root)
    lab = set(lab_files)
    donor = lab_files[mdl_anim.TOWN_JUMP_DONOR].read_bytes() if mdl_anim.TOWN_JUMP_DONOR in lab_files else None
    names = (class_name, weapon)
    queue = sorted({p for p in na if selected(p, names) and p not in lab}
                   | {p for p in NATIVE_ASSETS.get(weapon, ()) if p in na and p not in lab})
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
        data = town_motion(rel, data, donor)
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
            included = set()
            if donor:
                included = ({q.lower() for q in mdl_anim.Model(lab_files[donor].read_bytes()).sequences}
                            - {q.lower() for q in mdl_anim.Model(data).sequences})
                # the 1.69 keeps the town motion's events in the same _ex script (jump_town): without its
                # block the Miri froze after a town jump until the map changed (C4 MI14)
                town = rel.replace('_battle_motion_ex.mdl', '_town_motion.mdl')
                if town in records:
                    included |= {q.lower() for q in mdl_anim.Model((out / town).read_bytes()).sequences}
                data = mdl_anim.with_included_model(data, donor)
                record['included'] = donor
            (out / rel).write_bytes(data)
            record.update({'sha256': sha256(data), 'bytes': len(data), 'converted': True})
            for script, full in extreme_scripts(rel).items():
                if script not in records or full not in records:
                    continue
                text = (out / full).read_bytes()
                extra = {'copied_from': full}
                if script.startswith('scripts/actionstate/'):
                    text = with_extreme_includes(text, lambda q: q in lab)
                    donor_states = DONOR_STATES.get(rel.split('/')[3])
                    if donor_states and donor_states[0] in lab_files:
                        text, states = with_donor_states(text, lab_files[donor_states[0]].read_bytes(), donor_states[1])
                        extra.update({'states_from': donor_states[0], 'states_added': states})
                donor_events = donor and donor.replace('_battle_motion_ex.mdl', '_motion_ex.txt')
                if donor_events and script.endswith('_motion_ex.txt') and donor_events in lab_files:
                    donor_text = lab_files[donor_events].read_bytes()
                    text, added = with_included_events(text, donor_text, included)
                    text, opened = with_jump_activations(text, donor_text)
                    extra.update({'events_from': donor_events, 'events_added': len(added), 'jump_activations': len(opened)})
                (out / script).write_bytes(text)
                records[script].update({'sha256': sha256(text), 'bytes': len(text), 'converted': True, **extra})
    merges = []
    for rel, na_rel in [(r, r) for r in SHARED_SCRIPTS] + list(EXTREME_MERGES.items()):
        if na_rel not in na or rel not in lab:
            continue
        source = na[na_rel].read_bytes()
        target = out / 'merge' / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(spskill_for_169(source, lab_files[rel].read_bytes(), class_name) if rel == SPSKILL_TABLE else source)
        merges.append({'path': rel, 'source': na[na_rel].relative_to(na_root).as_posix(), 'source_sha256': sha256(source),
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
