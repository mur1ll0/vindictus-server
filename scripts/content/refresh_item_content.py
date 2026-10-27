"""Bring the game server's item exports up to the published item editor edits (docs/future/item-editor.md,
cut 2). The panel runs inside the game container, which has no Python and reads .cache/private-content
read-only, so the host does this step: start-lab.bat runs it before the client copy is updated.

1. read the overlay the game wrote (.cache/content-state/content-edits.json) and validate its items
   with content_additions (template, columns, costumes, assets, text);
2. stop when the stamp (.cache/private-content/item-content.json) already has the same item digest;
3. back up the 13 item exports, then re-run their exporters with fixed arguments (the overlay is
   copied to .cache/private-content/content-edits.json, which the shop exporters read);
4. write the stamp (overlay version and digest) and restart the game container, which loads the
   exports and records the version as applied;
5. if an exporter fails or the game does not become healthy, put the backup back and restart again.

Never runs anything taken from the overlay: the exporters and their arguments are the list below."""
from __future__ import annotations
import argparse
import json
import os
import pathlib
import shutil
import subprocess
import sys
import time

import content_additions
import content_edits

ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPTS = pathlib.Path(__file__).resolve().parent
PRIVATE = ROOT / '.cache/private-content'
OVERLAY = content_additions.EDITS
STAMP = PRIVATE / 'item-content.json'
BACKUP = ROOT / '.cache/private-content-items-backup/last'
# (exporter, output, fixed arguments): every export that projects ItemClassInfo/EquipItemInfo rows.
EXPORTS = (
    ('export_item_catalog.py', 'item-catalog.json', ()),
    ('export_item_stats.py', 'item-stats.json', ()),
    ('export_item_colors.py', 'item-colors.json', ()),
    ('export_item_economy.py', 'item-economy.json', ()),
    ('export_item_enhance.py', 'item-enhance.json', ()),
    ('export_npc_scripts.py', 'npc-scripts.json', ()),
    ('export_random_item.py', 'random-item.json', ()),
    ('export_cash_shop.py', 'cash-shop.json', ()),
    ('export_item_names.py', 'item-names.json', ()),
    ('export_panel_catalog.py', 'panel-catalog.json', ()),
    ('export_equipment_profile.py', 'normal-equipment-profile-v4.json', ('--all-classes',)),
    ('export_fiona_appearance.py', 'fiona-appearance-inventory.json', ()),
    ('export_fiona_inventory.py', 'fiona-inventory.json', ()),
)
EXPORT_TIMEOUT = 900
LOG = ROOT / '.cache/verification/refresh-item-content.log'


def state(overlay_path: pathlib.Path = OVERLAY) -> dict:
    """The overlay's version and the digest of its item content (validated)."""
    edits = content_edits.load(overlay_path)
    items = content_additions.load(edits=edits)
    overrides = content_additions.load_overrides(edits)
    return {'format_version': 1, 'version': edits.get('version', 0),
            'items_sha256': content_additions.digest(items, overrides),
            'items': len(items), 'overrides': len(overrides), 'hidden': len(content_additions.hidden_items(edits))}


def current_stamp(stamp: pathlib.Path = STAMP) -> dict:
    try:
        return json.loads(stamp.read_text(encoding='utf-8'))
    except (OSError, ValueError):
        return {}


def run_exports(log) -> None:
    for script, output, arguments in EXPORTS:
        partial = PRIVATE / (output + '.partial')
        partial.unlink(missing_ok=True)
        command = [sys.executable, str(SCRIPTS / script), '--output', str(partial), *arguments]
        log.write(f'$ {script} {" ".join(arguments)}\n')
        log.flush()
        done = subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, timeout=EXPORT_TIMEOUT)
        if done.returncode != 0 or not partial.is_file():
            raise RuntimeError(f'{script} failed (see {LOG})')
    for _, output, _ in EXPORTS:
        os.replace(PRIVATE / (output + '.partial'), PRIVATE / output)


def restart_game(log) -> bool:
    for command in (['docker', 'compose', 'restart', 'game'], ['docker', 'compose', 'up', '-d', '--wait', 'game']):
        done = subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, timeout=600)
        if done.returncode != 0:
            return False
    return True


def backup() -> None:
    if BACKUP.exists():
        shutil.rmtree(BACKUP)
    BACKUP.mkdir(parents=True)
    for _, output, _ in EXPORTS:
        if (PRIVATE / output).is_file():
            shutil.copy2(PRIVATE / output, BACKUP / output)
    for extra in (STAMP, PRIVATE / 'content-edits.json'):
        if extra.is_file():
            shutil.copy2(extra, BACKUP / extra.name)


def restore() -> None:
    for path in BACKUP.iterdir():
        shutil.copy2(path, PRIVATE / path.name)


def refresh(force: bool = False, restart: bool = True) -> dict:
    wanted = state()
    if not force and current_stamp().get('items_sha256') == wanted['items_sha256']:
        return {'refreshed': False, 'reason': 'up to date', **wanted}
    LOG.parent.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    with LOG.open('w', encoding='utf-8') as log:
        backup()
        try:
            if OVERLAY.is_file():
                shutil.copy2(OVERLAY, PRIVATE / 'content-edits.json')
            run_exports(log)
            STAMP.write_text(json.dumps(wanted), encoding='utf-8')
            if restart and not restart_game(log):
                raise RuntimeError('the game container did not become healthy')
        except Exception as error:
            log.write(f'refresh failed: {error}\n')
            restore()
            if restart:
                restart_game(log)
            raise
    return {'refreshed': True, 'seconds': round(time.monotonic() - started), **wanted}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--force', action='store_true', help='re-export even when the stamp matches')
    parser.add_argument('--no-restart', action='store_true', help='leave the game container running')
    parser.add_argument('--check', action='store_true', help='only validate and compare with the stamp')
    args = parser.parse_args()
    try:
        if args.check:
            wanted = state()
            print(json.dumps({'up_to_date': current_stamp().get('items_sha256') == wanted['items_sha256'], **wanted}))
            return 0
        print(json.dumps(refresh(args.force, not args.no_restart)))
    except (ValueError, RuntimeError, OSError, subprocess.SubprocessError) as error:
        print(json.dumps({'refreshed': False, 'error': str(error)}))
        return 2
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
