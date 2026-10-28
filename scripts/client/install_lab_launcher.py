"""Write start-lab.bat in a COPY of the client (B4.4c), the laboratory launcher: before starting
the client like start.bat, it re-exports the server's items when the item editor published a change
(scripts/content/refresh_item_content.py, which restarts the game container), and it applies the published admin panel content edits to the copy's
ShopInfo (the game server writes the overlay to .cache/content-state/content-edits.json when it
starts), as the original launcher patched the client before the game. The copy's start.bat is
left as the original."""
from __future__ import annotations
import argparse
import json
from pathlib import Path

from prepare_lab_client import PreparationError, reject_original

ROOT = Path(__file__).resolve().parents[2]
EDITS = ROOT / '.cache/content-state/content-edits.json'
START = 'start vindictus.exe -stage -dev -lang en-eu-x-gm -noupdate -threads 4'
# The launcher drops extra arguments (-condebug had no effect in D8h). engine.dll runs
# `exec config_pendingkey_concmd.cfg` at start; a plain one asks the console for a log file, the
# only place that names refused precaches, missing models and unknown entity classes.
CONSOLE_CFG = 'cfg/config_pendingkey_concmd.cfg'
CONSOLE = 'developer 1\r\ncon_logfile lab_console.log\r\n'


def launcher(project: Path = ROOT, edits: Path = EDITS) -> str:
    script = project / 'scripts/client/apply_lab_content_additions.py'
    refresh = project / 'scripts/content/refresh_item_content.py'
    return '\r\n'.join([
        '@echo off',
        'rem Laboratory launcher (scripts/client/install_lab_launcher.py): brings the game server item',
        'rem exports up to the published item editor edits, updates this copy with the published panel',
        'rem content, then starts the client like start.bat.',
        'cd /d "%~dp0"',
        f'py -3.13 "{refresh}" > "%~dp0start-lab-items.log" 2>&1',
        'if errorlevel 1 (',
        '  echo Could not refresh the server item content; see start-lab-items.log. The server keeps its last items.',
        '  pause',
        ')',
        f'py -3.13 "{script}" --client-copy "%~dp0." --apply --edits "{edits}" > "%~dp0start-lab.log" 2>&1',
        'if errorlevel 1 (',
        '  echo Could not update the client content; see start-lab.log. The client starts without it.',
        '  pause',
        ')',
        START,
        '',
    ])


def install(client_copy: Path, project: Path = ROOT, edits: Path = EDITS) -> dict:
    database = reject_original(client_copy / 'sql/heroes.db3')
    if not database.is_file() or not (client_copy / 'vindictus.exe').is_file():
        raise PreparationError('The client copy must contain vindictus.exe and sql/heroes.db3.')
    target = client_copy / 'start-lab.bat'
    text = launcher(project, edits)
    data = text.encode('ascii')
    changed = not target.exists() or target.read_bytes() != data
    if changed:
        target.write_bytes(data)
    console = client_copy / CONSOLE_CFG
    if not console.exists():
        console.parent.mkdir(exist_ok=True)
        console.write_bytes(CONSOLE.encode('ascii'))
    return {'launcher': str(target), 'changed': changed, 'console_cfg': str(console)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--client-copy', type=Path, required=True)
    args = parser.parse_args()
    try:
        print(json.dumps(install(args.client_copy.resolve())))
    except (PreparationError, UnicodeEncodeError) as error:
        print(json.dumps({'installed': False, 'error': str(error)}))
        return 2
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
