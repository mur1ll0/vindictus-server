"""Log in with the launcher's ticket when Start Game is clicked, skipping the login page, in a COPY.

Dry-run by default.

The title's Start Game button sends the `ShowLoginDialog` command to GameTitleMenu_V2 (handler at
0x3826a1d0). It fills the server list from EndPointNetwork, selects the first server
(`GameTitleMenu_V2.selectServerItem`) and returns; the page then shows the ID and password fields.
The page's Login button sends `Login` (0x3826a3c5): it copies `GameTitleMenu_V2.selectedServerStr`
into the `server_name` ConVar (0x38269c70, SetValue at +0x2c), builds the KeyValues
`TransitionArgs {ID, Password}` (new 0x28, constructor 0x38917d10) and fires the `LoginDialog_OK`
transition (0x3851e5f0 → 0x385194d0(name, args)). With NGM_disable the ID becomes the passport of
UserLoginMessage (0x3852245d); the password never leaves the client.

The patch replaces the epilogue of the ShowLoginDialog branch (0x3826a3b9) with a jump to a new
`.hlgn` section. There, `CommandLine()->ParmValue("-launcher_ticket", "")` is read; when it is empty
the original epilogue runs and nothing changes. Otherwise the stub does what Login does, with the
ticket as ID and an empty password: the server is the selected one, or the first EndPointNetwork
entry (+0x24) when the page has not set it yet; with no server at all it falls back to the page.
"""
from __future__ import annotations
import argparse
import json
import os
import struct
import sys
from pathlib import Path

from patch_class_table import Image, align, digest
from prepare_lab_client import PreparationError, reject_original

SECTION = b'.hlgn\0\0\0'
PARAMETER = '-launcher_ticket'
SITE = 0x3826a3b9
# pop edi; pop ebx; pop esi; add esp, 0x408; ret 0xc (end of the ShowLoginDialog branch)
EPILOGUE = bytes.fromhex('5f5b5e81c408040000c20c00')
# Branch of the title handler that must not jump into the replaced bytes.
HANDLER = (0x3826a1d0, 0x3826a4e7)

COMMAND_LINE = 0x38ada3cc      # import: tier0 CommandLine()
EMPTY = 0x38ae996a             # ""
SCALEFORM = 0x385f7ef0         # UI movie; +0x14 GetVariable string
SELECTED_SERVER = 0x38b44174   # "GameTitleMenu_V2.selectedServerStr"
ENDPOINTS = 0x38e50e54         # EndPointNetwork interface cached by the handler; +0x18 count, +0x24 name
SERVER_NAME = 0x38269c70       # server_name ConVar; +0x2c SetValue
NEW = 0x38916210               # operator new (cdecl)
KEYVALUES = 0x38917d10         # KeyValues(name, k1, v1, k2, v2), ret 0x14
TRANSITION_ARGS = 0x38b44154   # "TransitionArgs"
ID = 0x38b44164                # "ID"
PASSWORD = 0x38b44168          # "Password"
LOGIN_OK = 0x38b44144          # "LoginDialog_OK"
STATES = 0x3851e5f0            # state machine getter
TRANSITION = 0x385194d0        # Transition(name, args), ret 8


class Stub:
    """Position-dependent code for the section: bytes plus the offsets of absolute dwords."""

    def __init__(self, va: int):
        self.va, self.body, self.relocs, self.labels, self.fixups = va, bytearray(), [], {}, []

    def here(self) -> int:
        return self.va + len(self.body)

    def raw(self, hexbytes: str) -> None:
        self.body += bytes.fromhex(hexbytes)

    def absolute(self, prefix: str, value: int) -> None:
        self.raw(prefix)
        self.relocs.append(len(self.body))
        self.body += struct.pack('<I', value)

    def call(self, target: int) -> None:
        self.body += b'\xe8' + struct.pack('<i', target - (self.here() + 5))

    def branch(self, opcode: str, label: str) -> None:
        self.raw(opcode)
        self.fixups.append((len(self.body), label))
        self.body += bytes(4)

    def label(self, name: str) -> None:
        self.labels[name] = len(self.body)

    def finish(self) -> bytes:
        for at, name in self.fixups:
            struct.pack_into('<i', self.body, at, self.labels[name] - (at + 4))
        return bytes(self.body)


def build_stub(va: int) -> tuple[bytes, list[int]]:
    """The section body (code, then the parameter string) and its relocated offsets."""
    s = Stub(va)
    parameter = va + 0x100
    s.absolute('ff15', COMMAND_LINE)        # call [CommandLine]
    s.raw('8b10')                           # mov edx, [eax]
    s.absolute('68', EMPTY)                 # push ""
    s.absolute('68', parameter)             # push "-launcher_ticket"
    s.raw('8bc8')                           # mov ecx, eax
    s.raw('ff5220')                         # call [edx+0x20]  ParmValue(name, default)
    s.raw('85c0')                           # test eax, eax
    s.branch('0f84', 'page')
    s.raw('803800')                         # cmp byte [eax], 0
    s.branch('0f84', 'page')
    s.raw('8bf8')                           # mov edi, eax  (ticket)
    s.call(SCALEFORM)
    s.raw('8b10')                           # mov edx, [eax]
    s.raw('8bc8')                           # mov ecx, eax
    s.absolute('68', SELECTED_SERVER)       # push "GameTitleMenu_V2.selectedServerStr"
    s.raw('ff5214')                         # call [edx+0x14]
    s.raw('8bd8')                           # mov ebx, eax  (server)
    s.raw('85db')                           # test ebx, ebx
    s.branch('0f84', 'first')
    s.raw('803b00')                         # cmp byte [ebx], 0
    s.branch('0f85', 'server')
    s.label('first')
    s.absolute('a1', ENDPOINTS)             # mov eax, [EndPointNetwork]
    s.raw('85c0')                           # test eax, eax
    s.branch('0f84', 'page')
    s.raw('8b10')                           # mov edx, [eax]
    s.raw('8bc8')                           # mov ecx, eax
    s.raw('ff5218')                         # call [edx+0x18]  count
    s.raw('85c0')                           # test eax, eax
    s.branch('0f8e', 'page')                # jle
    s.absolute('a1', ENDPOINTS)             # mov eax, [EndPointNetwork]
    s.raw('8b10')                           # mov edx, [eax]
    s.raw('6a00')                           # push 0
    s.raw('8bc8')                           # mov ecx, eax
    s.raw('ff5224')                         # call [edx+0x24]  name of server 0
    s.raw('8bd8')                           # mov ebx, eax
    s.raw('85db')                           # test ebx, ebx
    s.branch('0f84', 'page')
    s.raw('803b00')                         # cmp byte [ebx], 0
    s.branch('0f84', 'page')
    s.label('server')
    s.call(SERVER_NAME)
    s.raw('8b10')                           # mov edx, [eax]
    s.raw('8bc8')                           # mov ecx, eax
    s.raw('53')                             # push ebx
    s.raw('ff522c')                         # call [edx+0x2c]  SetValue(server)
    s.raw('6a28')                           # push 0x28
    s.call(NEW)
    s.raw('83c404')                         # add esp, 4
    s.raw('85c0')                           # test eax, eax
    s.branch('0f84', 'fire')
    s.absolute('68', EMPTY)                 # push ""  (password)
    s.absolute('68', PASSWORD)              # push "Password"
    s.raw('57')                             # push edi  (ticket)
    s.absolute('68', ID)                    # push "ID"
    s.absolute('68', TRANSITION_ARGS)       # push "TransitionArgs"
    s.raw('8bc8')                           # mov ecx, eax
    s.call(KEYVALUES)
    s.label('fire')
    s.raw('50')                             # push eax  (args, or null)
    s.absolute('68', LOGIN_OK)              # push "LoginDialog_OK"
    s.call(STATES)
    s.raw('8bc8')                           # mov ecx, eax
    s.call(TRANSITION)
    s.label('page')
    s.body += EPILOGUE                      # the replaced epilogue
    code = s.finish()
    if len(code) > 0x100:
        raise PreparationError('The stub overflows its string area.')
    body = code + b'\xcc' * (0x100 - len(code)) + PARAMETER.encode('ascii') + b'\0'
    return body, s.relocs


def relocation_block(rva: int, offsets: list[int]) -> bytes:
    entries = [0x3000 | o for o in sorted(offsets)]
    if len(entries) % 2:
        entries.append(0)
    return struct.pack('<II', rva, 8 + 2 * len(entries)) + b''.join(struct.pack('<H', e) for e in entries)


def jump(site: int, target: int) -> bytes:
    return b'\xe9' + struct.pack('<i', target - (site + 5))


def plan(data: bytes) -> tuple[bytes, bool]:
    """Return the patched image and whether it was already patched."""
    image = Image(data)
    o = image.offset(SITE)
    existing = [s for s in image.sections() if s[0] == SECTION]
    if existing:
        stub = image.base + existing[0][2]
        if bytes(image.data[o:o + 5]) != jump(SITE, stub):
            raise PreparationError('The .hlgn section exists but the jump differs; restore the backup.')
        return data, True
    if bytes(image.data[o:o + len(EPILOGUE)]) != EPILOGUE:
        raise PreparationError(f'Unexpected bytes at {SITE:#x}; nothing is patched.')
    if image.table + 40 * (image.count + 1) > min(s[4] for s in image.sections()):
        raise PreparationError('No room for another section header.')
    rva = align(max(v + max(vs, rs) for _, vs, v, rs, _ in image.sections()), image.section_align)
    body, offsets = build_stub(image.base + rva)
    reloc_rva, reloc_size = image.directory(5)
    start = image.offset(image.base + reloc_rva)
    relocs = bytes(image.data[start:start + reloc_size])
    pad = align(len(body), 4) - len(body)
    reloc_at = len(body) + pad
    block = relocation_block(rva, offsets)
    body = body + bytes(pad) + relocs + block
    raw = align(len(image.data), image.file_align)
    image.data += b'\0' * (raw - len(image.data)) + body + b'\0' * (align(len(body), image.file_align) - len(body))
    # code + read-only data: IMAGE_SCN_CNT_CODE | MEM_EXECUTE | MEM_READ
    struct.pack_into('<8sIIIIIIHHI', image.data, image.table + 40 * image.count, SECTION, len(body), rva,
                     align(len(body), image.file_align), raw, 0, 0, 0, 0, 0x60000020)
    image.count += 1
    struct.pack_into('<H', image.data, image.pe + 6, image.count)
    struct.pack_into('<I', image.data, image.opt + 56, align(rva + len(body), image.section_align))
    struct.pack_into('<I', image.data, image.opt + 64, 0)  # checksum
    image.set_directory(4, 0, 0)  # the signature no longer covers the image
    image.set_directory(5, rva + reloc_at, len(relocs) + len(block))
    image.data[o:o + 5] = jump(SITE, image.base + rva)
    return bytes(image.data), False


def patch(client_copy: Path, apply: bool = False) -> dict:
    dll = reject_original(client_copy / 'bin' / 'client.dll')
    if not dll.is_file():
        raise PreparationError('The client copy must contain bin/client.dll.')
    data = dll.read_bytes()
    image, already = plan(data)
    backup = dll.with_name(f'client.dll.{digest(data)[:16]}.pre-launcherlogin.bak')
    result = {'mode': 'apply' if apply else 'dry-run', 'dll': str(dll), 'already_patched': already,
              'before_sha256': digest(data), 'after_sha256': digest(image), 'site': hex(SITE),
              'parameter': PARAMETER, 'backup': str(backup)}
    if apply and not already:
        if not backup.exists():
            backup.write_bytes(data)
        partial = dll.with_suffix('.partial')
        partial.write_bytes(image)
        os.replace(partial, dll)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--client-copy', type=Path, required=True)
    parser.add_argument('--apply', action='store_true')
    args = parser.parse_args()
    try:
        print(json.dumps(patch(args.client_copy, args.apply), indent=1))
    except PreparationError as error:
        print(json.dumps({'patched': False, 'error': str(error)}))
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main())
