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

import gfx_patch
import kv_merge
import mdl_anim
import mdl_v45_to_v44
from model_kit import TEXTURE_PARAMS, model_materials

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'content'))
import class_costumes  # noqa: E402

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


JUMP_ACTIVATION = re.compile(rb'^[ \t]*"(?!event")[^"\r\n]*"[ \t]+"AE_ACTIVATE_STATE_CHANGE_CONDITION[ \t]+(Jump\w*|SuperPower)"[^\r\n]*(?:\r?\n)', re.M)


def with_jump_activations(script: bytes, donor_script: bytes) -> tuple[bytes, list[str]]:
    """The class's events script with the donor's jump and XE activations (AE_ACTIVATE_STATE_CHANGE_CONDITION
    Jump*, SuperPower) in each block both scripts have and the class's lacks: the NA had no jump, so the
    Miri's own idle, run and rest never opened JumpBegin and Space did nothing in battle (C4 MI14); the XE
    move opens from the same 71 blocks as the donor's (idle, run, attacks, light damage...)."""
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


# Activations of the donor's $ref_group$ lists the class lacks: the NA Miri's common_idle_run_active_states
# has no SearchEvilCore, so E on what a monster dropped did nothing (C4 MI27; M7g only added the
# search_evil_core events).
REF_GROUP_ACTIVATIONS = {'common_idle_run_active_states': ('SearchEvilCore',)}
REF_GROUP = '$ref_group$'


def with_ref_group_activations(script: bytes, donor_script: bytes,
                               wanted: dict = REF_GROUP_ACTIVATIONS) -> tuple[bytes, list[str]]:
    """The class's events script with the donor's `wanted` activations added to the same $ref_group$ list."""
    mine, theirs = event_blocks(script).get(REF_GROUP.lower()), event_blocks(donor_script).get(REF_GROUP.lower())
    if mine is None or theirs is None:
        return script, []
    def groups(text, span):
        root = next((n for n in kv_merge.parse(text[span[0]:span[1]]).children if n.block), None)
        return {g.key: g for g in root.children if g.block} if root else {}
    my_groups, their_groups = groups(script, mine), groups(donor_script, theirs)
    edits, added = [], []
    for name, states in wanted.items():
        group, donor = my_groups.get(name), their_groups.get(name)
        if group is None or donor is None:
            continue
        have = set(re.findall(rb'AE_ACTIVATE_STATE_CHANGE_CONDITION[ \t]+(\w+)', script[mine[0] + group.start:mine[0] + group.end]))
        for state in states:
            line = re.search(rb'^[^\r\n]*AE_ACTIVATE_STATE_CHANGE_CONDITION[ \t]+' + state.encode() + rb'"[^\r\n]*\r?\n',
                             donor_script[theirs[0] + donor.start:theirs[0] + donor.end], re.M)
            if line and state.encode() not in have:
                at = script.rfind(b'\n', 0, mine[0] + group.close) + 1
                edits.append((at, line.group(0)))
                added.append(f'{name}/{state}')
    for at, text in sorted(edits, key=lambda e: e[0], reverse=True):
        script = script[:at] + text + script[at:]
    return script, added


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
# The character list portrait (gfx_patch): the 1.69 select movie gets a frame for the class, showing an
# atlas made from the NA's own portrait of the class (C4 MI4-4: the list showed Lann's face).
LIST_PORTRAIT = 'scaleform/player_portrait/c_info_{klass}.tga'
LIST_PORTRAIT_ATLAS = 'scaleform/selectcharacterdialog_v2_i{klass}.dds'
# 1.69 scripts that ExtremeMode reads in place of a shared one, mapped to the NA script whose class lines
# they get: with ExtremeMode the 1.69 loads scripts/action_table_player_ex.txt (client and server), and
# the NA's _ex table has no Miri line, so her AE_SET_CUR_ACTION_INFO miri_* found no action: 37 damage a
# hit at 13k ATK and no SP from attacks (C4 MI15).
EXTREME_MERGES = {'scripts/action_table_player_ex.txt': 'scripts/action_table_player.txt'}
# States the 1.69's ExtremeMode gives every class and the NA's class files lack (the NA had no jump): the
# jump attacks of the "XE common" block, copied from the 1.69 donor's _ex state file (C4 MI15), and the XE
# move (SuperPower), which asks for the class's own XE skill (class_imports.donor_xe_rows) and weapon
# entity in place of the donor's (donor_renames).
DONOR_STATES = {'female': ('scripts/actionstate/player_action_state_delia_ex.txt',
                           ('JumpAttack', 'JumpAttackStrong', 'SuperPower'))}


def donor_renames(class_name: str, weapon: str) -> tuple[tuple[bytes, bytes], ...]:
    donor = class_costumes.XE_DONOR
    entity = class_costumes.LAB_WEAPON_ENTITY.get(weapon, f'weapon_{weapon}')
    return ((class_costumes.XE_SKILL.format(klass=donor).encode(), class_costumes.XE_SKILL.format(klass=class_name).encode()),
            (f'"{class_costumes.DONOR_WEAPON_ENTITY[donor]}"'.encode(), f'"{entity}"'.encode()))


def with_donor_states(script: bytes, donor_script: bytes, names: tuple[str, ...],
                      renames: tuple[tuple[bytes, bytes], ...] = ()) -> tuple[bytes, list[str]]:
    """The class's action-state script with the donor's `names` states, in each section
    (StateChangeConditionDefine, StateDefine) where the donor has them and the class does not, with the
    `renames` (donor text, class text) made in the copied states."""
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
                text = donor_script[state.start:state.end]
                for old, new in renames:
                    text = text.replace(old, new)
                edits.append((target.close, b'\t\t' + text + newline + b'\t'))
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


# Effects name their materials, models and sub-effects; without following them the Miri's skill effects
# drew nothing (Blazing Spine's blue fire, effects/fire_ani_03; Slice and Dice, effects/dark_02; C4 MI20).
EFX_REFERENCE = re.compile(rb'"(material|matname|sprite|model|effect|efx|sub_effect)"\s+"([^"]+)"', re.I)
# NA physically based materials (a shader the 1.69 lacks) drew as a white wireframe (the Dragon Knight
# armour, C4 MI20; the later Draker weapons): they become Heroes_Armor with the texture the player sees,
# the parameters the 1.69 shader knows, and nothing of the PBS set.
PBS_SHADERS = ('heroes_pbs', 'heroes_pbs_texture_transition')
ARMOR_PARAMS = ('$bumpmap', '$envmap', '$envmapcontrast', '$envmaptint', '$normalmapalphaenvmapmask', '$nocull',
                '$nodecal', '$alphatest', '$translucent', '$halflambert', '$rimlight_enable', '$rimlight_exp',
                '$rimlight_boost', '$selfillum', '$selfillummask', '$phong_enable', '$phongexponent', '$phongboost',
                '$surfaceprop')
VMT_PARAM = re.compile(r'^\s*"(\$[A-Za-z0-9_]+)"\s+("[^"]*"|\S+)', re.M)


def pbs_as_armor(path: str, data: bytes) -> bytes:
    """A material of a PBS shader as Heroes_Armor; any other data unchanged."""
    if not path.endswith('.vmt'):
        return data
    text = data.decode('latin-1')
    head = re.match(r'\s*"?([A-Za-z0-9_]+)"?', text)
    if not head or head.group(1).lower() not in PBS_SHADERS:
        return data
    live = '\n'.join(line for line in text.splitlines() if not line.lstrip().startswith('//'))
    params = {k.lower(): v for k, v in VMT_PARAM.findall(live)}
    base = params.get('$transition_map') or params.get('$basetexture')
    lines = ['"Heroes_Armor"', '{']
    if base:
        lines.append(f'\t"$basetexture" {base if base.startswith(chr(34)) else chr(34) + base + chr(34)}')
    for key in ARMOR_PARAMS:
        if key in params:
            value = params[key]
            lines.append(f'\t"{key}" {value if value.startswith(chr(34)) else chr(34) + value + chr(34)}')
    lines.append('}')
    return ('\r\n'.join(lines) + '\r\n').encode('latin-1')


# The NA's Dragon Knight look names the NA weapon entity (weapon_draker, NA sub-type), which the 1.69 copy
# does not have (the Draker is a weapon_greatsword there): the look keeps the weapon she holds.
CSF_WITHOUT_WEAPON = ('csf/metamorphosis/{klass}_dragon_knight.csf',)


# The Dragon Knight's black wings (M8j/M8k, operator). The NA client wears them from code; the look's .csf
# leaves the back slot empty ("rear" sn -1), but the NA start menu look (csf/<class>_dragon_knight_startmenu.csf)
# names them: rear 500139, an entry the class merge already brings (the needles name the class). The 1.69
# loads the whole .csf (M7n), so the look takes that back slot from the start menu one.
CSF_REAR = {'csf/metamorphosis/{klass}_dragon_knight.csf': 'csf/{klass}_dragon_knight_startmenu.csf'}
# That entry draws models/rear/dragonknight/<class>_dragonknight_wing.mdl (29 bones, from Spine2), and with it
# worn the transformation's camera (AE_ENABLE_CAMERA_ORIGIN/TARGET_BONE) went under the boat (C4 MI40): the
# camera bones ValveBiped.Anim_Attachment_CAM(_Origin) of every body model are not in it. The NA also has
# the wings alone on the whole skeleton, camera bones included (models/player/<class>/dragonknight/
# <class>_dragonknight_wing.mdl, one mesh, 2546 vertices against the body's 12632), so the merge's copy of
# the entry draws that one; the body (upper_body 500152) keeps its hair and bust jiggle bones.
KNIGHT_WINGS = ('models/rear/dragonknight/{klass}_dragonknight_wing.mdl',
                'models/player/{klass}/dragonknight/{klass}_dragonknight_wing.mdl')


def _costume_child(data: bytes, key: str):
    import kv_merge as kv
    costume = next((n for n in kv.parse(data).children if n.block), None)
    return next((c for c in costume.children if c.block and c.key.lower() == key), None) if costume else None


def with_csf_rear(data: bytes, look: bytes) -> bytes:
    rear, worn = _costume_child(data, 'rear'), _costume_child(look, 'rear')
    if rear is None or worn is None:
        return data
    return data[:rear.start] + look[worn.start:worn.end] + data[rear.end:]


def with_knight_wings(costume_script: bytes, klass: str) -> bytes:
    back, whole = (m.format(klass=klass).encode() for m in KNIGHT_WINGS)
    return costume_script.replace(b'"' + back + b'"', b'"' + whole + b'"')


def without_csf_weapon(data: bytes) -> bytes:
    import kv_merge as kv
    root = kv.parse(data)
    costume = next((n for n in root.children if n.block), None)
    weapon = next((c for c in costume.children if c.block and c.key.lower() == 'weapon'), None) if costume else None
    if weapon is None:
        return data
    start = data.rfind(b'\n', 0, weapon.start) + 1
    end = data.find(b'\n', weapon.end)
    return data[:start] + data[end + 1 if end >= 0 else weapon.end:]


# Item icons of the class's imported items (class_imports, .cache/private-content/class-imports/<class>.db3):
# their names seldom carry the class's (Dragon Rise: eq_dragon_rises_female_*), so the sets showed no icon.
CLASS_IMPORT = ROOT / '.cache/private-content/class-imports/{klass}.db3'
ICON_COLUMNS = ('Icon', 'IconFG', 'IconBG')
ICON_FOLDER = 'scaleform/item_icon/'


def imported_icons(class_name: str, na: dict, path: Path | None = None) -> set[str]:
    import sqlite3
    db_path = path or Path(str(CLASS_IMPORT).format(klass=class_name))
    if not db_path.is_file():
        return set()
    db = sqlite3.connect(f'file:{db_path.as_posix()}?mode=ro', uri=True)
    try:
        cols = {r[1] for r in db.execute('PRAGMA table_info(ItemClassInfo)')}
        names = set()
        for col in (c for c in ICON_COLUMNS if c in cols):
            names |= {r[0].lower() for r in db.execute(f'SELECT [{col}] FROM ItemClassInfo') if r[0]}
    finally:
        db.close()
    return {p for p in na if p.startswith(ICON_FOLDER) and p[len(ICON_FOLDER):].rsplit('.', 1)[0] in names}


# The NA plays most of the Miri's skill effects through AE_CL_PLAY_CUSTOM_EFFECT <effect> <Dragon Knight
# effect> (37 events: Blazing Spine, Slice and Dice, the Fiery smashes...). They stay as they are: the
# Miri's client FireEvent picks the path by her state (patch_class_routing.CUSTOM_EFFECT, M8i).


# The NA spends the Dragon Knight transformation's SP at a frame of miri_SP4 (AE_CUSTOM_MOTION
# REDUCE_DRAGON_KNIGHT_SP_GAUGE; NA server 0x5dc470: the SP of skill_miri_sp4 taken off the gauge), while its
# state AttackSP04 only checks it (SPGauge_check_only 200). No 1.69 handler knows the command, so the
# transformation cost nothing. It becomes the 1.69's AE_GAIN_SP_GAUGE -<SP the state checks> at the same
# frame (the 1.69 scripts spend that way: AE_GAIN_SP_GAUGE -50).
SP_REDUCE = {'REDUCE_DRAGON_KNIGHT_SP_GAUGE': 'AttackSP04'}
SP_REDUCE_STATES = 'scripts/actionstate/player_action_state_{klass}.txt'


def state_sp_checks(states: bytes) -> dict[str, int]:
    """SP each state of an action-state script checks without spending (property SPGauge_check_only)."""
    out = {}
    stack = [kv_merge.parse(states)]
    while stack:
        node = stack.pop()
        for child in node.children:
            if not child.block:
                continue
            prop = next((c for c in child.children if c.block and c.key.lower() == 'property'), None)
            value = next((c.value for c in prop.children if not c.block and c.key == 'SPGauge_check_only'), None) if prop else None
            if value and value.isdigit():
                out.setdefault(child.key, int(value))
            stack.append(child)
    return out


# Stopping a battle run opened SprintStop (battle_run_sprint_stop, a short dash forward) from the class's
# battle_run_sprint, even after a tap of a move key; the 1.69 classes (Delia, Fiona) go back to BattleIdle
# there. Operator decision (C4 MI27): the class's battle sprint ends in BattleIdle, as the Delia's.
SPRINT_STOP = re.compile(rb'(AE_ACTIVATE_STATE_CHANGE_CONDITION[ \t]+)SprintStop(GamePad)?\b')
SPRINT_BLOCK = 'battle_run_sprint'


def without_sprint_stop(path: str, data: bytes) -> bytes:
    if not (path.startswith('models/player/_anim/') and path.endswith('.txt')):
        return data
    span = event_blocks(data).get(SPRINT_BLOCK)
    if span is None:
        return data
    start, end = span
    return data[:start] + SPRINT_STOP.sub(rb'\1BattleIdle', data[start:end]) + data[end:]


def with_sp_reduce(path: str, data: bytes, checks: dict[str, int]) -> bytes:
    if not (path.startswith('models/player/_anim/') and path.endswith('.txt')):
        return data
    for command, state in SP_REDUCE.items():
        if state in checks:
            data = re.sub(rb'AE_CUSTOM_MOTION([ \t]+)' + command.encode() + rb'\b',
                          rb'AE_GAIN_SP_GAUGE\g<1>-' + str(checks[state]).encode(), data)
    return data


# Call of the Dragon (C4 MI27: no dragon): the NA client draws the dragon itself (C_HeroesDrakerSystem,
# client x64 0xaa5110: a client entity of SUMMONED_MODEL; SUMMON_DRAGON plays SUMMONED_SEQUENCE at the
# player, RETURN_DRAGON hides it, PLAY_DRAGON_EFFECT plays an effect on the dragon's bones), which the 1.69
# has not. The 1.69 server spawns a model prop at the player playing a sequence (AE_CREATE_ATTACKABLE_MODEL
# <mdl> <sequence> <forward>, removed by AE_DELETE_ATTACKABLE_MODEL <mdl> <delay>; the Delia's and Evy's
# transformations use it), and plays a model's own events file (<model>.txt) on it: the summon becomes that
# pair, and the dragon's effects move to its events file (summoned_model_events).
SUMMONED_MODEL = 'models/effects/mesh/pc_{klass}_sp5_dragon.mdl'
SUMMONED_SEQUENCE = 'dragon_attack'
SUMMON = re.compile(rb'AE_CUSTOM_WEAPON_EVENT_INDP([ \t]+)SUMMON_DRAGON\b')
RETURN = re.compile(rb'AE_CUSTOM_WEAPON_EVENT_INDP([ \t]+)RETURN_DRAGON\b')
DRAGON_EFFECT = re.compile(rb'^[^\r\n]*AE_CUSTOM_MOTION_CLIENT[ \t]+PLAY_DRAGON_EFFECT[ \t]+([^\s"]+)[^\r\n]*\r?\n', re.M)


def with_summoned_model(path: str, data: bytes, klass: str) -> bytes:
    if not (path.startswith('models/player/_anim/') and path.endswith('.txt')):
        return data
    model = SUMMONED_MODEL.format(klass=klass).replace('/', '\\').encode()
    data = SUMMON.sub(rb'AE_CREATE_ATTACKABLE_MODEL\g<1>' + model.replace(b'\\', b'\\\\') + b' '
                      + SUMMONED_SEQUENCE.encode() + b' 0.0', data)
    data = RETURN.sub(rb'AE_DELETE_ATTACKABLE_MODEL\g<1>' + model.replace(b'\\', b'\\\\') + b' 0.0', data)
    return DRAGON_EFFECT.sub(b'', data)


# The dragon's eyes (M8b): the NA client puts a sprite (sprites/redglow1.vmt, scale 0.1, glow render mode,
# brightness 255) on each eye attachment of the model (client x64 0xaa532c), in code. The 1.69 attaches an
# effect to an attachment from the event (AE_CL_PLAY_EFFECT <efx> attachment_1 <name>, as the Kraken's
# eyeL1), so the eyes are a laboratory effect: one red glow particle renewed every 0.05 s, following the
# attachment, for the length of SUMMONED_SEQUENCE (639 frames at 60 fps); the material is the 1.69's
# particle glow effects/redeye1.vmt (the ratman's eyes; base texture sprites/redglow2), since the sprite
# shader of redglow1 is not a particle material. It dies with the model.
SUMMONED_EYES = ('LEFT_EYE', 'RIGHT_EYE')
SUMMONED_EYE_EFFECT = 'effects/player/{klass}/pc_{klass}_sp5_dragon_eye.efx'
SUMMONED_EYE_EFX = (
    '// Laboratory effect (Miri M8b): the Call of the Dragon eye glow, NA client 0xaa532c\r\n'
    '"particle"\r\n{\r\n'
    '\t"follow_parent_1"\t"1"\r\n\t"direction_axis_1"\t"2"\r\n\t"position_offset_1"\t"0 0 0"\r\n'
    '\t"emitter"\t"simple"\r\n\t"gravity"\t"0"\r\n'
    '\t"emit_interval"\t"0.05"\r\n\t"emit_duration"\t"10.7"\r\n'
    '\t"material"\t"effects/redeye1.vmt"\r\n\t"particle_count"\t"1"\r\n'
    '\t"color_min"\t"255 255 255"\r\n'
    '\t"die_time"\r\n\t{\r\n\t\t"0"\t"0.1"\r\n\t}\r\n'
    '\t"use_spread_direction"\t"1"\r\n'
    '\t"color_ramp"\r\n\t{\r\n\t\t"0"\t"1 1"\r\n\t}\r\n'
    '\t"size_delta"\r\n\t{\r\n\t\t"0"\t"12"\r\n\t}\r\n'
    '\t"alpha_delta"\r\n\t{\r\n\t\t"0"\t"255"\r\n\t}\r\n'
    '\t"roll_delta"\r\n\t{\r\n\t\t"0"\t"0"\r\n\t}\r\n'
    '\t"length"\t"0.5 2"\r\n\t"width"\t"5 5"\r\n\t"wind_blown"\t"0"\r\n\t"no_screen"\t"0"\r\n'
    '}\r\n')


def summoned_model_events(motion: bytes, klass: str | None = None) -> bytes | None:
    """The dragon model's events file: its PLAY_DRAGON_EFFECT effects at frame 0 of SUMMONED_SEQUENCE, then
    the eye glows on SUMMONED_EYES when the class is given."""
    effects = list(dict.fromkeys(m.group(1).decode('latin-1') for m in DRAGON_EFFECT.finditer(motion)))
    if not effects:
        return None
    lines = ''.join(f'\t"0"\t"AE_CL_PLAY_EFFECT\t{e}"\r\n' for e in effects)
    if klass is not None:
        eye = SUMMONED_EYE_EFFECT.format(klass=klass)
        lines += ''.join(f'\t"0"\t"AE_CL_PLAY_EFFECT\t{eye} attachment_1 {a}"\r\n' for a in SUMMONED_EYES)
    return f'"{SUMMONED_SEQUENCE}"\r\n{{\r\n{lines}}}\r\n'.encode('latin-1')


COSTUME_SCRIPT = 'scripts/player_costume.txt'
MODEL_COMPANIONS = ('.vvd', '.dx90.vtx', '.dx80.vtx', '.sw.vtx', '.xbox.vtx', '.phy', '.ani')


def imported_costumes(class_name: str, costume_script: bytes, path: Path | None = None) -> set[str]:
    """Model paths (lower case) of the NA player_costume.txt entries the class's imported non-weapon items
    wear (EquipItemInfo.CostumeType in the class import), male entries left out. Neither the models nor the
    entries name the class (Dragon Rise: models/armor/dragon_rises/...), so the pieces equipped but drew
    nothing (C4 MI21); the paths stage the models and are the merge's needles for those entries.
    An entry's "onepiece" (the 1.69 reads it too: the body model drawn in place of the upper and lower
    pieces when both are worn) names another entry of the same section, which comes with it: the Dragon
    Rise tunic pointed at a one-piece entry the lab did not have, the tunic and the pants hid each other
    and equipping them in turn crashed the client (C4 MI24)."""
    import sqlite3
    db_path = path or Path(str(CLASS_IMPORT).format(klass=class_name))
    if not db_path.is_file():
        return set()
    db = sqlite3.connect(f'file:{db_path.as_posix()}?mode=ro', uri=True)
    try:
        if not db.execute("SELECT 1 FROM sqlite_master WHERE name='EquipItemInfo'").fetchone():
            return set()
        numbers = {str(r[0]) for r in db.execute(
            'SELECT DISTINCT CostumeType FROM EquipItemInfo WHERE CostumeType IS NOT NULL '
            'AND upper(EquipClass) NOT IN (SELECT DISTINCT upper(EquipClass) FROM EquipItemInfo e '
            "JOIN ItemClassInfo i ON i.ItemClass = e.ItemClass WHERE i.Category = 'WEAPON')")}
    finally:
        db.close()
    models = set()
    for section in kv_merge.parse(costume_script).children:
        if not section.block:
            continue
        entries = {e.key: e for e in section.children if e.block}
        wanted, seen = [n for n in numbers if n in entries], set()
        while wanted:
            number = wanted.pop()
            if number in seen:
                continue
            seen.add(number)
            values = {c.key.lower(): (c.value or '') for c in entries[number].children if not c.block}
            if values.get('category', '').lower() == 'male':
                continue
            for key in ('normal', 'broken'):
                model = values.get(key, '').replace(chr(92), '/').lower()
                if model.endswith('.mdl'):
                    models.add(model)
            onepiece = values.get('onepiece', '').split(';')[0].strip()
            if onepiece in entries:
                wanted.append(onepiece)
    return models


def dependencies(path: str, data: bytes, known=lambda p: False) -> list[str]:
    """Game paths a staged model or material needs. A model's material is the first cdmaterials folder
    that has it, as the engine searches them (`known`: the path exists in the 1.69, the NA or the
    staging); a material no folder has is reported in its first folder."""
    if path.endswith('.mdl') and data[:4] == b'IDST':
        # the mesh and physics files beside the .mdl; a model staged without them (the imported costume
        # models, whose names do not carry the class's) loaded with no mesh (C4 MI22)
        companions = [path[:-4] + ext for ext in MODEL_COMPANIONS if known(path[:-4] + ext)]
        textures, folders = model_materials(data)
        wanted = []
        for texture in textures:
            paths = ['materials/' + (folder + texture).replace('\\', '/').lower().strip('/') + '.vmt' for folder in folders]
            wanted.append(next((p for p in paths if known(p)), paths[0] if paths else None))
        return [w for w in wanted if w] + [m if m.startswith('models/') else 'models/' + m for m in include_models(data)] + companions
    if path.endswith('.efx'):
        wanted = []
        for key, value in EFX_REFERENCE.findall(data):
            name = value.decode('latin-1').replace(chr(92), '/').lower().strip('/')
            if key.lower() in (b'material', b'matname', b'sprite'):
                wanted.append('materials/' + (name[:-4] if name.endswith('.vmt') else name) + '.vmt')
            elif name.endswith(('.mdl', '.efx')):
                wanted.append(name)
        return wanted
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
                   | {p for p in NATIVE_ASSETS.get(weapon, ()) if p in na and p not in lab}
                   | {p for p in imported_icons(class_name, na) if p not in lab})
    costume_models = imported_costumes(class_name, na[COSTUME_SCRIPT].read_bytes()) if COSTUME_SCRIPT in na else set()
    queue = sorted(set(queue) | {m for m in costume_models if m in na and m not in lab})
    csf_without_weapon = {c.format(klass=class_name) for c in CSF_WITHOUT_WEAPON}
    sp_states = SP_REDUCE_STATES.format(klass=class_name)
    sp_checks = state_sp_checks(na[sp_states].read_bytes()) if sp_states in na else {}
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
        data = pbs_as_armor(rel, data)
        data = with_sp_reduce(rel, data, sp_checks)
        data = without_sprint_stop(rel, data)
        data = with_summoned_model(rel, data, class_name)
        if rel in csf_without_weapon:
            data = without_csf_weapon(data)
        rear_look = CSF_REAR.get(rel.replace(class_name, '{klass}'), '').format(klass=class_name)
        if rear_look in na:
            data = with_csf_rear(data, na[rear_look].read_bytes())
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
                        text, states = with_donor_states(text, lab_files[donor_states[0]].read_bytes(), donor_states[1],
                                                         donor_renames(class_name, weapon))
                        extra.update({'states_from': donor_states[0], 'states_added': states})
                donor_events = donor and donor.replace('_battle_motion_ex.mdl', '_motion_ex.txt')
                if donor_events and script.endswith('_motion_ex.txt') and donor_events in lab_files:
                    donor_text = lab_files[donor_events].read_bytes()
                    text, added = with_included_events(text, donor_text, included)
                    text, opened = with_jump_activations(text, donor_text)
                    text, grouped = with_ref_group_activations(text, donor_text)
                    extra['ref_group_activations'] = grouped
                    extra.update({'events_from': donor_events, 'events_added': len(added), 'jump_activations': len(opened)})
                (out / script).write_bytes(text)
                records[script].update({'sha256': sha256(text), 'bytes': len(text), 'converted': True, **extra})
    portrait = LIST_PORTRAIT.format(klass=class_name)
    if portrait in records:
        atlas = LIST_PORTRAIT_ATLAS.format(klass=class_name)
        data = gfx_patch.class_portrait_dds((out / portrait).read_bytes())
        (out / atlas).parent.mkdir(parents=True, exist_ok=True)
        (out / atlas).write_bytes(data)
        records[atlas] = {'path': atlas, 'source': records[portrait]['source'], 'source_sha256': records[portrait]['source_sha256'],
                          'sha256': sha256(data), 'converted': True, 'bytes': len(data), 'copied_from': portrait}
    dragon = SUMMONED_MODEL.format(klass=class_name)
    for motion in [r for r in records if r.startswith('models/player/_anim/') and r.endswith(f'pc_{class_name}_motion.txt')]:
        events = summoned_model_events(na[motion].read_bytes(), class_name)
        if events is not None and dragon in records:
            target = dragon[:-len('.mdl')] + '.txt'
            (out / target).write_bytes(events)
            records[target] = {'path': target, 'source': records[motion]['source'], 'source_sha256': records[motion]['source_sha256'],
                               'sha256': sha256(events), 'converted': True, 'bytes': len(events), 'copied_from': motion}
            eye = SUMMONED_EYE_EFFECT.format(klass=class_name)
            efx = SUMMONED_EYE_EFX.encode('latin-1')
            (out / eye).parent.mkdir(parents=True, exist_ok=True)
            (out / eye).write_bytes(efx)
            records[eye] = {'path': eye, 'source': 'laboratory (stage_na_class.SUMMONED_EYE_EFX)', 'source_sha256': sha256(efx),
                            'sha256': sha256(efx), 'converted': True, 'bytes': len(efx), 'copied_from': None}
    merges = []
    for rel, na_rel in [(r, r) for r in SHARED_SCRIPTS] + list(EXTREME_MERGES.items()):
        if na_rel not in na or rel not in lab:
            continue
        source = na[na_rel].read_bytes()
        target = out / 'merge' / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        data = spskill_for_169(source, lab_files[rel].read_bytes(), class_name) if rel == SPSKILL_TABLE else source
        if rel == COSTUME_SCRIPT and KNIGHT_WINGS[1].format(klass=class_name) in records:
            data = with_knight_wings(data, class_name)
        target.write_bytes(data)
        merges.append({'path': rel, 'source': na[na_rel].relative_to(na_root).as_posix(), 'source_sha256': sha256(source),
                       'needles': list(names) + (sorted(costume_models) if rel == COSTUME_SCRIPT else [])})
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
