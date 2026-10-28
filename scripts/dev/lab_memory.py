"""Read-only inspection of the running laboratory client (32-bit vindictus/heroes process).

Attaches with PROCESS_QUERY_INFORMATION | PROCESS_VM_READ only: nothing is written, injected or
suspended. Addresses are the DLLs' preferred ones (the lab DLLs have no ASLR); the tool reports
when a module is loaded elsewhere and rebases the named symbols.

Usage (python -s -E scripts/dev/lab_memory.py COMMAND ...):
  ps                          processes that loaded client.dll
  modules                     base and size of client.dll, server.dll and engine.dll
  dump EXPR [LEN]             hex dump (LEN default 0x40, at most 0x1000)
  dword EXPR [COUNT]          signed dwords
  str EXPR                    zero-terminated ASCII string
  symbols                     named addresses from the patch report (bin/class-routing.json)
  costume [CLASS]             costume manager column of CLASS (default 10): default and
                              default_ex part per slot and town, in the client and the server
  ptrstr EXPR LEN [FILTER]    dwords in a range that point to printable strings
  rtti EXPR LEN [FILTER]      dwords in a range that point to objects with an RTTI class
  class EXPR                  RTTI class of the object at EXPR
  save EXPR LEN NAME          raw bytes to .cache/lab-memory/NAME.bin
  diff NAME1 NAME2            dwords that differ between two saved dumps (no game needed)
  shell                       read commands from stdin, one per line
  serve                       run commands from .cache/lab-memory/request.txt (one per line) and
                              write the output to response.txt, until a `quit` request or the
                              game exits; lets a non-elevated session use one elevated reader

EXPR: a number (0x...), a symbol name, `[EXPR]` (dword at EXPR) and `+`/`-` offsets,
for example `[client.costume.real]+0xbb8`.
"""
from __future__ import annotations
import ctypes
import ctypes.wintypes as wt
import json
import re
import shlex
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REPORT = ROOT / '.cache/play-client/bin/class-routing.json'
MODULES = ('client.dll', 'server.dll', 'engine.dll')
PREFERRED = {'client.dll': 0x38000000, 'server.dll': 0x40000000, 'engine.dll': 0x42000000}
MAX_READ = 0x1000
GAME_EXES = ('vindictus.exe', 'heroes.exe')
# Costume manager layout (scripts/client/patch_class_routing.py, COSTUME).
# Index 10 is the motion part (its default/default_ex/town are motion ids, D8j); 9 and 11 are not
# confirmed and keep their number.
SLOTS = ['base', 'inner_armor', 'face', 'hair', 'upper_body', 'lower_body', 'head', 'foot', 'hand',
         'part 9', 'motion', 'part 11']

PROCESS_QUERY_INFORMATION, PROCESS_VM_READ = 0x0400, 0x0010
TH32CS_SNAPPROCESS, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32 = 0x2, 0x8, 0x10
kernel32 = ctypes.WinDLL('kernel32', use_last_error=True)


class PROCESSENTRY32W(ctypes.Structure):
    _fields_ = [('dwSize', wt.DWORD), ('cntUsage', wt.DWORD), ('th32ProcessID', wt.DWORD),
                ('th32DefaultHeapID', ctypes.c_void_p), ('th32ModuleID', wt.DWORD), ('cntThreads', wt.DWORD),
                ('th32ParentProcessID', wt.DWORD), ('pcPriClassBase', wt.LONG), ('dwFlags', wt.DWORD),
                ('szExeFile', wt.WCHAR * 260)]


class MODULEENTRY32W(ctypes.Structure):
    _fields_ = [('dwSize', wt.DWORD), ('th32ModuleID', wt.DWORD), ('th32ProcessID', wt.DWORD),
                ('GlblcntUsage', wt.DWORD), ('ProccntUsage', wt.DWORD), ('modBaseAddr', ctypes.c_void_p),
                ('modBaseSize', wt.DWORD), ('hModule', wt.HMODULE), ('szModule', wt.WCHAR * 256),
                ('szExePath', wt.WCHAR * 260)]


kernel32.CreateToolhelp32Snapshot.restype = wt.HANDLE
kernel32.OpenProcess.restype = wt.HANDLE
kernel32.ReadProcessMemory.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t,
                                       ctypes.POINTER(ctypes.c_size_t)]


class InspectError(Exception):
    pass


def processes() -> list[tuple[int, str]]:
    snap = kernel32.CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
    entry, out = PROCESSENTRY32W(), []
    entry.dwSize = ctypes.sizeof(entry)
    ok = kernel32.Process32FirstW(snap, ctypes.byref(entry))
    while ok:
        out.append((entry.th32ProcessID, entry.szExeFile))
        ok = kernel32.Process32NextW(snap, ctypes.byref(entry))
    kernel32.CloseHandle(snap)
    return out


def modules(pid: int) -> dict[str, tuple[int, int, str]]:
    snap = kernel32.CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid)
    if snap in (None, wt.HANDLE(-1).value):
        return {}
    entry, out = MODULEENTRY32W(), {}
    entry.dwSize = ctypes.sizeof(entry)
    ok = kernel32.Module32FirstW(snap, ctypes.byref(entry))
    while ok:
        out[entry.szModule.lower()] = (entry.modBaseAddr or 0, entry.modBaseSize, entry.szExePath)
        ok = kernel32.Module32NextW(snap, ctypes.byref(entry))
    kernel32.CloseHandle(snap)
    return out


def elevated(pid: int) -> bool | None:
    """Whether a process runs elevated (None when its token cannot be read)."""
    advapi32 = ctypes.WinDLL('advapi32', use_last_error=True)
    handle = kernel32.OpenProcess(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
    if not handle:
        return None
    token, value, size = wt.HANDLE(), wt.DWORD(), wt.DWORD()
    try:
        if not advapi32.OpenProcessToken(handle, 0x0008, ctypes.byref(token)):  # TOKEN_QUERY
            return None
        advapi32.GetTokenInformation(token, 20, ctypes.byref(value), 4, ctypes.byref(size))  # TokenElevation
        kernel32.CloseHandle(token)
        return bool(value.value)
    finally:
        kernel32.CloseHandle(handle)


class Game:
    def __init__(self):
        found = [(pid, name, modules(pid)) for pid, name in processes()]
        loaded = [f for f in found if 'client.dll' in f[2]]
        # Without access, the module list is empty: fall back to the game's executable name.
        named = [f for f in found if f[1].lower() in GAME_EXES]
        if not loaded and not named:
            raise InspectError('The game is not running; start the client first.')
        self.pid, self.name, self.mods = (loaded or named)[0]
        self.handle = kernel32.OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, False, self.pid)
        if not self.handle:
            error = ctypes.get_last_error()
            if error == 5 and elevated(self.pid) and not elevated(kernel32.GetCurrentProcessId()):
                raise InspectError(f'{self.name} ({self.pid}) runs as administrator; run this tool from an '
                                   'administrator terminal.')
            raise InspectError(f'OpenProcess({self.pid}) failed: error {error} (the anti-cheat may deny read access).')
        self.symbols = load_symbols(self.mods)

    def read(self, address: int, size: int) -> bytes:
        if not 0 < size <= MAX_READ:
            raise InspectError(f'Read size must be 1..{MAX_READ:#x}.')
        buffer, done = ctypes.create_string_buffer(size), ctypes.c_size_t()
        if not kernel32.ReadProcessMemory(self.handle, ctypes.c_void_p(address), buffer, size, ctypes.byref(done)):
            raise InspectError(f'ReadProcessMemory({address:#x}, {size}) failed: error {ctypes.get_last_error()}.')
        return buffer.raw[:done.value]

    def dword(self, address: int) -> int:
        return struct.unpack('<i', self.read(address, 4))[0]

    def string(self, address: int) -> str:
        raw = self.read(address, 0x100)
        return raw.split(b'\0', 1)[0].decode('latin1')

    def eval(self, text: str) -> int:
        tokens = re.findall(r'\[|\]|[+-]|[A-Za-z_][\w.]*|0x[0-9a-fA-F]+|\d+', text.replace(' ', ''))
        if ''.join(tokens) != text.replace(' ', ''):
            raise InspectError(f'Cannot parse expression {text!r}.')
        position = 0

        def term() -> int:
            nonlocal position
            token = tokens[position]
            position += 1
            if token == '[':
                value = expression()
                if position >= len(tokens) or tokens[position] != ']':
                    raise InspectError(f'Missing ] in {text!r}.')
                position += 1
                return self.dword(value) & 0xFFFFFFFF
            if token in self.symbols:
                return self.symbols[token]
            if re.fullmatch(r'0x[0-9a-fA-F]+|\d+', token):
                return int(token, 0)
            raise InspectError(f'Unknown symbol {token!r}; see `symbols`.')

        def expression() -> int:
            nonlocal position
            value = term()
            while position < len(tokens) and tokens[position] in '+-':
                sign = tokens[position]
                position += 1
                value = value + term() if sign == '+' else value - term()
            return value

        value = expression()
        if position != len(tokens):
            raise InspectError(f'Unexpected {tokens[position]!r} in {text!r}.')
        return value


def load_symbols(mods: dict) -> dict[str, int]:
    """Named addresses from the patch report, rebased if a module moved."""
    symbols = {}
    if not REPORT.is_file():
        return symbols
    for dll in json.loads(REPORT.read_text(encoding='utf-8')).get('dlls', []):
        name = Path(dll['dll']).name.lower()
        delta = mods.get(name, (PREFERRED.get(name, 0),))[0] - PREFERRED.get(name, 0)
        prefix = name.split('.')[0]
        costume = dll.get('costume')
        if costume:
            symbols[f'{prefix}.costume.shadow'] = int(costume['shadow'], 16) + delta
            symbols[f'{prefix}.costume.real'] = int(costume['shadow'], 16) + 0x1000 + delta
        match = dll.get('match_table')
        if match:
            symbols[f'{prefix}.match.delia_map'] = int(match['map'], 16) + delta
    return symbols


class MEMORY_BASIC_INFORMATION(ctypes.Structure):
    _fields_ = [('BaseAddress', ctypes.c_void_p), ('AllocationBase', ctypes.c_void_p),
                ('AllocationProtect', wt.DWORD), ('PartitionId', wt.WORD), ('RegionSize', ctypes.c_size_t),
                ('State', wt.DWORD), ('Protect', wt.DWORD), ('Type', wt.DWORD)]


def read_block(handle, address: int, size: int) -> bytes:
    """ReadProcessMemory of any size (works with a Game created by an older reload)."""
    buffer, done = ctypes.create_string_buffer(size), ctypes.c_size_t()
    if not kernel32.ReadProcessMemory(handle, ctypes.c_void_p(address), buffer, size, ctypes.byref(done)):
        raise InspectError(f'ReadProcessMemory({address:#x}, {size}) failed: error {ctypes.get_last_error()}.')
    return buffer.raw[:done.value]


def regions(game: Game):
    """Committed, readable, non-guard regions of the 32-bit address space."""
    info, address = MEMORY_BASIC_INFORMATION(), 0x10000
    query = kernel32.VirtualQueryEx
    query.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.POINTER(MEMORY_BASIC_INFORMATION), ctypes.c_size_t]
    while address < 0x80000000 and query(game.handle, ctypes.c_void_p(address), ctypes.byref(info), ctypes.sizeof(info)):
        size = info.RegionSize
        readable = info.Protect & 0xEE and not info.Protect & 0x100  # any read access, no PAGE_GUARD
        if info.State == 0x1000 and readable:  # MEM_COMMIT
            yield address, size
        address += size


def class_name(game: Game, address: int) -> str | None:
    """RTTI class of the object at ADDRESS (its first dword a vtable inside a loaded module)."""
    if not 0x10000 < address < 0x80000000:
        return None
    try:
        vtable = game.dword(address) & 0xFFFFFFFF
        if not any(base <= vtable < base + size for base, size, _ in game.mods.values()) and \
                not any(PREFERRED[n] <= vtable < PREFERRED[n] + 0x2000000 for n in PREFERRED):
            return None
        locator = game.dword(vtable - 4) & 0xFFFFFFFF
        if game.dword(locator) != 0:
            return None
        descriptor = game.dword(locator + 12) & 0xFFFFFFFF
        name = game.string(descriptor + 8)
    except Exception:  # unreadable page; the serve reload may use another error class
        return None
    return name if name.startswith('.?AV') else None


def hexdump(game: Game, address: int, size: int) -> None:
    data = game.read(address, size)
    for i in range(0, len(data), 16):
        row = data[i:i + 16]
        text = ''.join(chr(c) if 32 <= c < 127 else '.' for c in row)
        print(f'{address + i:08x}  {row.hex(" "):<47}  {text}')


def costume(game: Game, klass: int) -> None:
    for prefix in ('client', 'server'):
        real = f'{prefix}.costume.real'
        if real not in game.symbols:
            print(f'{prefix}: no costume symbols (apply patch_class_routing.py)')
            continue
        target = game.dword(game.symbols[real]) & 0xFFFFFFFF
        column = 0 if klass == 10 else klass
        if klass == 10:
            base = game.symbols[f'{prefix}.costume.shadow']
            source = 'shadow'
        else:
            base, source = target, 'real'
        if not base:
            print(f'{prefix}: costume object not loaded yet (real pointer is 0)')
            continue
        print(f'{prefix} class {klass} ({source} {base:#x}, column {column}; real object {target:#x})')
        for part, slot in enumerate(SLOTS):
            default = game.dword(base + 0xF0 + part * 40 + column * 4)
            extra = game.dword(base + 0x2D0 + part * 40 + column * 4)
            print(f'  {slot:<12} default {default:>7}  default_ex {extra:>7}')
        print(f'  town {game.dword(base + 0xBB8 + column * 4)}')


def run(game: Game | None, argv: list[str]) -> None:
    command, args = argv[0], argv[1:]
    if command == 'ps':
        for pid, name in processes():
            mods = modules(pid)
            if 'client.dll' in mods:
                print(pid, name, mods['client.dll'][2])
        return
    if game is None:
        raise InspectError('No game attached.')
    if command == 'modules':
        for name in MODULES:
            base, size, path = game.mods.get(name, (0, 0, 'not loaded'))
            moved = '' if base in (0, PREFERRED[name]) else f' (preferred {PREFERRED[name]:#x})'
            print(f'{name:<11} {base:#010x} {size:#x}{moved} {path}')
    elif command == 'dump':
        hexdump(game, game.eval(args[0]), int(args[1], 0) if len(args) > 1 else 0x40)
    elif command == 'dword':
        address = game.eval(args[0])
        for i in range(int(args[1], 0) if len(args) > 1 else 1):
            print(f'{address + 4 * i:#010x}  {game.dword(address + 4 * i)}')
    elif command == 'str':
        print(repr(game.string(game.eval(args[0]))))
    elif command == 'ptrstr':
        # Dwords in [EXPR, EXPR+LEN) that point (directly or through one more pointer) to a
        # printable string containing FILTER.
        base, length = game.eval(args[0]), int(args[1], 0)
        needle = args[2].lower() if len(args) > 2 else ''
        for offset in range(0, length, MAX_READ):
            data = game.read(base + offset, min(MAX_READ, length - offset))
            for i in range(0, len(data) - 3, 4):
                value = struct.unpack_from('<I', data, i)[0]
                if not 0x10000 < value < 0x80000000:
                    continue
                # direct string, then a pointer at +0/+4/+8 of the target (model_t keeps its name
                # as a string pointer one field in)
                for depth, target in ((0, value), (1, 0), (1, 4), (1, 8)):
                    try:
                        if depth == 1:
                            target = game.dword(value + target) & 0xFFFFFFFF
                        text = game.string(target) if 0x10000 < target < 0x80000000 else ''
                    except Exception:  # unreadable page; the serve reload may use another class
                        text = ''
                    if len(text) >= 4 and text.isprintable() and needle in text.lower():
                        print(f'+{offset + i:#06x} {"*" * depth}{target:#010x} {text[:100]!r}')
                        break
    elif command == 'rtti':
        # Dwords in [EXPR, EXPR+LEN) that point to an object with an MSVC RTTI vtable (or are
        # one): offset, object and class name. FILTER narrows the names.
        base, length = game.eval(args[0]), int(args[1], 0)
        needle = args[2].lower() if len(args) > 2 else ''
        for offset in range(0, length, MAX_READ):
            data = game.read(base + offset, min(MAX_READ, length - offset))
            for i in range(0, len(data) - 3, 4):
                value = struct.unpack_from('<I', data, i)[0]
                name = class_name(game, value)
                if name and needle in name.lower():
                    print(f'+{offset + i:#06x} {value:#010x} {name}')
    elif command == 'class':
        print(class_name(game, game.eval(args[0])) or 'no RTTI class')
    elif command == 'save':
        # save EXPR LEN NAME: raw bytes to .cache/lab-memory/NAME.bin, for `diff`
        base, length, name = game.eval(args[0]), int(args[1], 0), args[2]
        if not re.fullmatch(r'[\w-]{1,40}', name):
            raise InspectError('NAME: 1-40 letters, digits, _ or -.')
        data = b''.join(game.read(base + o, min(MAX_READ, length - o)) for o in range(0, length, MAX_READ))
        CHANNEL.mkdir(parents=True, exist_ok=True)
        (CHANNEL / f'{name}.bin').write_bytes(data)
        print(f'saved {len(data):#x} bytes from {base:#x} to {name}.bin')
    elif command == 'finddword':
        # finddword VALUE MODULE [MAX]: addresses inside MODULE holding the dword VALUE, with the
        # strings the 16 dwords before it point to (name pointers of RecvProp/SendProp entries)
        value, module = int(args[0], 0), args[1].lower()
        limit = int(args[2], 0) if len(args) > 2 else 20
        base, size, _ = game.mods[module]
        needle, found = struct.pack('<I', value & 0xFFFFFFFF), 0
        for offset in range(0, size, 0x10000):
            try:
                chunk = read_block(game.handle, base + offset, min(0x10000, size - offset))
            except Exception:
                continue
            at = chunk.find(needle)
            while at != -1:
                address = base + offset + at
                names = []
                for back in range(4, 0x44, 4):
                    try:
                        text = game.string(game.dword(address - back) & 0xFFFFFFFF)
                    except Exception:
                        text = ''
                    if len(text) > 3 and text.isprintable():
                        names.append(f'-{back:#x}:{text[:40]}')
                print(f'{address:#010x} {" ".join(names[:3])}')
                found += 1
                if found >= limit:
                    return
                at = chunk.find(needle, at + 4)
    elif command == 'entities':
        # entities [client|server] [FILTER] [MAX]: entity-list slots with their RTTI class
        side = args[0] if args else 'client'
        needle = args[1].lower() if len(args) > 1 else ''
        slots = int(args[2], 0) if len(args) > 2 else 8192
        table = {'client': 0x38C8F3F4, 'server': 0x40CB3740}[side]
        base = (game.dword(table) & 0xFFFFFFFF) + 4  # 16-byte entries {object, serial, ...}
        data = b''.join(read_block(game.handle, base + o, min(0x10000, slots * 16 - o))
                        for o in range(0, slots * 16, 0x10000))
        shown = 0
        for index in range(len(data) // 16):
            obj = struct.unpack_from('<I', data, index * 16)[0]
            if not obj:
                continue
            name = class_name(game, obj) or '?'
            if needle in name.lower():
                print(f'{index:5d} {obj:#010x} {name}')
                shown += 1
        print(f'{shown} shown')
    elif command == 'regions':
        listed = list(regions(game))
        print(f'{len(listed)} readable regions, {sum(s for _, s in listed) // 0x100000} MiB; '
              f'last query error {ctypes.get_last_error()}')
        for start, size in listed[:5]:
            print(f'  {start:#010x} {size:#x}')
    elif command == 'find':
        # find TEXT [MAX]: addresses of an ASCII string in the committed, readable memory
        needle, limit = args[0].encode('latin1'), int(args[1], 0) if len(args) > 1 else 40
        found = 0
        for start, size in regions(game):
            for offset in range(0, size, 0x10000):
                try:
                    chunk = read_block(game.handle, start + offset, min(0x10000 + len(needle), size - offset))
                except Exception:
                    break
                at = chunk.find(needle)
                while at != -1 and at < 0x10000:
                    address = start + offset + at
                    owner = next((n for n, (b, s, _) in game.mods.items() if b <= address < b + s), 'heap')
                    print(f'{address:#010x} {owner}')
                    found += 1
                    if found >= limit:
                        return
                    at = chunk.find(needle, at + 1)
        print(f'{found} found')
    elif command == 'symbols':
        for name, value in sorted(game.symbols.items()):
            print(f'{name:<28} {value:#010x}')
    elif command == 'costume':
        costume(game, int(args[0], 0) if args else 10)
    else:
        raise InspectError(f'Unknown command {command!r}; see --help.')


CHANNEL = ROOT / '.cache/lab-memory'


def serve(game: Game) -> None:
    """Answer request.txt with response.txt (written atomically) until `quit` or the game exits."""
    import contextlib
    import io
    import os
    import time
    CHANNEL.mkdir(parents=True, exist_ok=True)
    request, response = CHANNEL / 'request.txt', CHANNEL / 'response.txt'
    (CHANNEL / 'serving.txt').write_text(f'{game.name} {game.pid}\n', encoding='utf-8')
    while True:
        if not any(pid == game.pid for pid, _ in processes()):
            break
        if not request.is_file():
            time.sleep(0.2)
            continue
        lines = request.read_text(encoding='utf-8').splitlines()
        request.unlink()
        # Reload this file for each request, so new commands work without another elevation.
        import importlib.util
        spec = importlib.util.spec_from_file_location('lab_memory_live', Path(__file__))
        live = importlib.util.module_from_spec(spec)
        try:
            spec.loader.exec_module(live)
            runner, errors = live.run, (live.InspectError, InspectError, IndexError, ValueError)
        except Exception as error:  # a broken edit must not stop the reader
            runner, errors = run, (InspectError, IndexError, ValueError)
            print(f'reload failed, using the loaded code: {error}', file=sys.stderr)
        out = io.StringIO()
        stop = False
        for line in lines:
            words = shlex.split(line)
            if not words:
                continue
            if words[0] == 'quit':
                stop = True
                break
            print(f'> {line}', file=out)
            with contextlib.redirect_stdout(out):
                try:
                    runner(game, words)
                except errors as error:
                    print(f'error: {error}')
        partial = response.with_suffix('.partial')
        partial.write_text(out.getvalue(), encoding='utf-8')
        os.replace(partial, response)
        if stop:
            break
    (CHANNEL / 'serving.txt').unlink(missing_ok=True)


def diff(first: str, second: str) -> None:
    """Dwords that differ between two saved dumps (no game needed)."""
    a, b = ((CHANNEL / f'{n}.bin').read_bytes() for n in (first, second))
    for i in range(0, min(len(a), len(b)) - 3, 4):
        x, y = struct.unpack_from('<I', a, i)[0], struct.unpack_from('<I', b, i)[0]
        if x != y:
            print(f'+{i:#06x}  {x:#010x} {struct.unpack_from("<i", a, i)[0]:>11}  |  '
                  f'{y:#010x} {struct.unpack_from("<i", b, i)[0]:>11}')


def main() -> int:
    if len(sys.argv) < 2 or sys.argv[1] in ('-h', '--help'):
        print(__doc__)
        return 0
    if sys.argv[1] == 'diff':
        diff(sys.argv[2], sys.argv[3])
        return 0
    try:
        if sys.argv[1] == 'ps':
            run(None, ['ps'])
            return 0
        game = Game()
        if sys.argv[1] == 'serve':
            serve(game)
            return 0
        if sys.argv[1] == 'shell':
            print(f'attached to {game.name} ({game.pid}); empty line or "quit" ends')
            for line in sys.stdin:
                words = shlex.split(line)
                if not words or words[0] == 'quit':
                    break
                try:
                    run(game, words)
                except (InspectError, IndexError, ValueError) as error:
                    print(f'error: {error}')
            return 0
        run(game, sys.argv[1:])
    except (InspectError, IndexError, ValueError) as error:
        print(f'error: {error}')
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main())
