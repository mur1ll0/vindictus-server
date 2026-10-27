"""HFS index and pack codec of the 1.69 EN client (docs/future/game-launcher.md, sections 2.2 and 2.4).

Index (hfs/E608DF10....hfs): 14 plain bytes (an HF 01 02 record head), then a zlib stream XORed with
78 BD 02 47 by position & 3. Plain: 'HFI' 01, u16 pack count at 0x04, u32 file count at 0x10, one
0x14-byte record per pack (its SHA-1 name as 5 little-endian u32, sorted), then 16-byte file entries
sorted by hash: name key (u32 hash, first and last character), u16 pack index, u32 data position, u32 size.

Pack: a stored zip with HF 01 02 local/central headers and an HF 05 06 end record. File names and data
are XORed with a 4096-byte keystream indexed by absolute file position; header fields stay plain and the
CRC-32 is of the plain data. Scripts and models are stored as 'x.comp' (b'comp', u32 size, zlib), which
the index lists as 'x'.

The keystream is derived from game content and is never versioned: pass --keystream <file>, made once
with 'keystream <pack.hfs> <plain folder> <out>' from a pack and the plain files of its folder."""
from __future__ import annotations
import argparse
import hashlib
import json
import struct
import sys
import zlib
from pathlib import Path
from typing import NamedTuple

INDEX_KEY = bytes.fromhex('78bd0247')
INDEX_HEAD = 14
INDEX_MAGIC = b'HFI\x01'
KEYSTREAM_SIZE = 4096
LOCAL = struct.Struct('<4sHHHHHIIIHH')
CENTRAL = struct.Struct('<4sHHHHHHIIIHHHHHII')
END = struct.Struct('<4sHHHHIIH')
SIG = b'HF\x01\x02'
END_SIG = b'HF\x05\x06'
COMP = b'comp'
# External attributes: 0x20 (archive) in most packs, 0x80 (normal) in others.
DEFAULT_ATTR = 0x20


class HfsError(ValueError):
    pass


class Entry(NamedTuple):
    name: str
    data: bytes
    attr: int = DEFAULT_ATTR


# --- keystream ---------------------------------------------------------------------------------

def load_keystream(path: Path) -> bytes:
    data = Path(path).read_bytes()
    if len(data) != KEYSTREAM_SIZE:
        raise HfsError('The keystream must have 4096 bytes.')
    return data


def xor_at(ks: bytes, pos: int, data: bytes) -> bytes:
    return bytes(b ^ ks[(pos + i) & 4095] for i, b in enumerate(data))


def comp_candidates(plain: bytes) -> list[bytes]:
    # The level the game used is unknown; the CRC tells which one reproduces the stored bytes.
    return [COMP + struct.pack('<I', len(plain)) + zlib.compress(plain, level) for level in (9, 6, 1, 2, 3, 4, 5, 7, 8)]


def derive_keystream(raw: bytes, plain_files: dict[str, bytes]) -> bytes:
    """Recovers the keystream from a pack and the plain files of its folder (name -> bytes, names
    without '.comp'). Each stored file is matched by size and CRC; the name bytes come from the match."""
    ks: list[int | None] = [None] * KEYSTREAM_SIZE

    def learn(pos: int, cipher: bytes, plain: bytes) -> None:
        for i, (c, p) in enumerate(zip(cipher, plain)):
            k = c ^ p
            at = (pos + i) & 4095
            if ks[at] is not None and ks[at] != k:
                raise HfsError('The plain files do not match this pack.')
            ks[at] = k

    for record in central_records(raw):
        found = None
        for name, plain in plain_files.items():
            for stored_name, stored in [(name, plain)] + [(name + '.comp', c) for c in comp_candidates(plain)]:
                if len(stored) == record.size and len(stored_name.encode('latin-1')) == record.name_len \
                        and zlib.crc32(stored) == record.crc:
                    found = stored_name.encode('latin-1'), stored
                    break
            if found:
                break
        if found is None:
            continue
        learn(record.name_at, raw[record.name_at:record.name_at + record.name_len], found[0])
        learn(record.data_at, raw[record.data_at:record.data_at + record.size], found[1])
    if any(k is None for k in ks):
        raise HfsError('The matched plain files do not cover all 4096 keystream positions.')
    return bytes(k for k in ks if k is not None)


# --- pack --------------------------------------------------------------------------------------

class Record(NamedTuple):
    crc: int
    size: int
    name_at: int
    name_len: int
    data_at: int
    attr: int


def central_records(raw: bytes) -> list[Record]:
    end = raw.rfind(END_SIG)
    if end < 0 or end + END.size > len(raw):
        raise HfsError('The pack has no end record.')
    _, _, _, _, count, _, central, _ = END.unpack_from(raw, end)
    records = []
    for _ in range(count):
        if central + CENTRAL.size > end:
            raise HfsError('The central directory runs past the end record.')
        f = CENTRAL.unpack_from(raw, central)
        if f[0] != SIG:
            raise HfsError('Bad central header.')
        size, name_len, extra_len, comment_len, local = f[9], f[10], f[11], f[12], f[16]
        if local + LOCAL.size > len(raw):
            raise HfsError('A local header lies outside the pack.')
        lf = LOCAL.unpack_from(raw, local)
        if lf[0] != SIG:
            raise HfsError('Bad local header.')
        data_at = local + LOCAL.size + lf[9] + lf[10]
        if data_at + size > len(raw):
            raise HfsError('File data lies outside the pack.')
        records.append(Record(f[7], size, central + CENTRAL.size, name_len, data_at, f[15]))
        central += CENTRAL.size + name_len + extra_len + comment_len
    return records


def read_pack(raw: bytes, ks: bytes) -> list[Entry]:
    entries = []
    for r in central_records(raw):
        name = xor_at(ks, r.name_at, raw[r.name_at:r.name_at + r.name_len]).decode('latin-1')
        data = xor_at(ks, r.data_at, raw[r.data_at:r.data_at + r.size])
        if zlib.crc32(data) != r.crc:
            raise HfsError(f'CRC mismatch in {name!r}.')
        entries.append(Entry(name, data, r.attr))
    return entries


def build_pack(entries: list[Entry], ks: bytes) -> bytes:
    out = bytearray()
    central = bytearray()
    for name, data, attr in entries:
        raw_name = name.encode('latin-1')
        crc = zlib.crc32(data)
        local = len(out)
        out += LOCAL.pack(SIG, 10, 0, 0, 0, 0, crc, len(data), len(data), len(raw_name), 0)
        out += xor_at(ks, len(out), raw_name)
        out += xor_at(ks, len(out), data)
        central += CENTRAL.pack(SIG, 20, 10, 0, 0, 0, 0, crc, len(data), len(data), len(raw_name), 0, 0, 0, 0, attr, local)
        central += raw_name  # encrypted below, once its absolute position is known
    start = len(out)
    pos = 0
    for name, _, _ in entries:
        raw_name = name.encode('latin-1')
        name_at = pos + CENTRAL.size
        central[name_at:name_at + len(raw_name)] = xor_at(ks, start + name_at, raw_name)
        pos = name_at + len(raw_name)
    out += central
    out += END.pack(END_SIG, 0, 0, len(entries), len(entries), len(central), start, 0)
    return bytes(out)


def listed_name(name: str) -> str:
    # The index lists 'x.txt.comp' as 'x.txt' and only the file name, without the folder.
    name = name.split('/')[-1]
    return name[:-5] if name.endswith('.comp') else name


def pack_files(raw: bytes, ks: bytes) -> list[tuple[str, int, int]]:
    """(listed file name, data position, size) of each file, without decrypting the data."""
    return [(listed_name(xor_at(ks, r.name_at, raw[r.name_at:r.name_at + r.name_len]).decode('latin-1')), r.data_at, r.size)
            for r in central_records(raw)]


def comp_wrap(plain: bytes, level: int = 9) -> bytes:
    return COMP + struct.pack('<I', len(plain)) + zlib.compress(plain, level)


def comp_unwrap(data: bytes) -> bytes:
    if data[:4] != COMP or len(data) < 8:
        raise HfsError('Not a .comp file.')
    plain = zlib.decompress(data[8:])
    if len(plain) != struct.unpack_from('<I', data, 4)[0]:
        raise HfsError('The .comp size does not match its data.')
    return plain


def pack_name(folder: str, part: int = 0) -> str:
    # FileSystem_Stdio 0x1000b050: separators become '@', '.<part>' is appended, case alternates
    # (upper at even positions), then SHA-1 printed as five %08X words.
    text = '@'.join(p for p in folder.replace('\\', '/').split('/') if p) + f'.{part}'
    text = ''.join(c.upper() if i % 2 == 0 else c.lower() for i, c in enumerate(text))
    return hashlib.sha1(text.encode('latin-1')).hexdigest().upper()


# --- index -------------------------------------------------------------------------------------

def xor_index(data: bytes) -> bytes:
    return bytes(b ^ INDEX_KEY[i & 3] for i, b in enumerate(data))


def check_index(plain: bytes) -> bytes:
    if len(plain) < 0x14 or plain[:4] != INDEX_MAGIC:
        raise HfsError('Not an HFS index.')
    packs = struct.unpack_from('<H', plain, 4)[0]
    files = struct.unpack_from('<I', plain, 0x10)[0]
    if len(plain) != 0x14 + 0x14 * packs + 16 * files:
        raise HfsError('The index size does not match its counts.')
    return plain


def decode_index(raw: bytes) -> bytes:
    inflater = zlib.decompressobj(47)
    try:
        plain = inflater.decompress(xor_index(raw[INDEX_HEAD:]))
    except zlib.error as error:
        raise HfsError('The index stream is not zlib data.') from error
    if not inflater.eof:
        raise HfsError('The index stream is truncated.')
    return check_index(plain)


def encode_index(plain: bytes, head: bytes) -> bytes:
    # Level 9 reproduces the original index byte for byte.
    if len(head) != INDEX_HEAD:
        raise HfsError('The index head must have 14 bytes.')
    raw = head + xor_index(zlib.compress(check_index(plain), 9))
    if decode_index(raw) != plain:
        raise HfsError('Index round trip failed.')
    return raw


def name_key(name: str) -> bytes:
    # The lookup (FileSystem_Stdio 0x10027f30) hashes only the lower-case file name.
    h = 0xAAAAAAAA
    for c in name.lower().encode('latin-1'):
        h = (h * 33 + c) & 0xFFFFFFFF
    return struct.pack('<IBB', h, ord(name[0].lower()), ord(name[-1].lower()))


def index_packs(plain: bytes) -> list[str]:
    count = struct.unpack_from('<H', plain, 4)[0]
    return [''.join('%08X' % w for w in struct.unpack_from('<5I', plain, 0x14 + 0x14 * i)) for i in range(count)]


def index_entries(plain: bytes) -> list[bytes]:
    table = 0x14 + 0x14 * struct.unpack_from('<H', plain, 4)[0]
    return [plain[i:i + 16] for i in range(table, len(plain), 16)]


def assemble_index(head: bytes, packs: list[str], entries: list[bytes]) -> bytes:
    out = bytearray(head[:0x14])
    struct.pack_into('<H', out, 4, len(packs))
    struct.pack_into('<I', out, 0x10, len(entries))
    records = b''.join(struct.pack('<5I', *(int(n[i:i + 8], 16) for i in range(0, 40, 8))) for n in packs)
    return check_index(bytes(out) + records + b''.join(entries))


def drop(plain: bytes, names: list[str]) -> tuple[bytes, int]:
    """Removes the entries of these file names; returns the new index and how many were removed."""
    check_index(plain)
    keys = {name_key(n) for n in names}
    entries = index_entries(plain)
    kept = [e for e in entries if e[:6] not in keys]
    return assemble_index(plain, index_packs(plain), kept), len(entries) - len(kept)


def pack_id(name: str) -> str:
    name = name.upper()
    if name.endswith('.HFS'):
        name = name[:-4]
    if len(name) != 40 or any(c not in '0123456789ABCDEF' for c in name):
        raise HfsError(f'A pack must be named <40 hex>.hfs: {name}')
    return name


def sync(plain: bytes, packs: list[tuple[str, bytes]], ks: bytes) -> tuple[bytes, list[tuple[str, int]]]:
    """Points the index at rebuilt or new packs ((pack name, pack bytes) pairs). A new pack keeps the
    pack list sorted, so the indices of later packs shift by one. Returns the index and the new packs."""
    check_index(plain)
    names = index_packs(plain)
    entries = index_entries(plain)
    added = []
    for name, raw in packs:
        pack = pack_id(name)
        if pack not in names:
            at = sum(1 for n in names if n < pack)
            names.insert(at, pack)
            entries = [e[:6] + struct.pack('<H', i + 1 if (i := struct.unpack_from('<H', e, 6)[0]) >= at else i) + e[8:]
                       for e in entries]
            added.append((pack, at))
        index = names.index(pack)
        entries = [e for e in entries if struct.unpack_from('<H', e, 6)[0] != index]
        for file_name, data_at, size in pack_files(raw, ks):
            entries.append(name_key(file_name) + struct.pack('<HII', index, data_at, size))
    # Stable sort by hash: entries of untouched packs keep their order.
    entries.sort(key=lambda e: struct.unpack_from('<I', e)[0])
    return assemble_index(plain, names, entries), added


# --- command line ------------------------------------------------------------------------------

def plain_folder(folder: Path) -> dict[str, bytes]:
    return {p.name: p.read_bytes() for p in sorted(folder.iterdir()) if p.is_file()}


def run(args: argparse.Namespace) -> dict:
    ks = lambda: load_keystream(args.keystream)  # noqa: E731
    if args.command == 'decode':
        plain = decode_index(args.index.read_bytes())
        args.out.write_bytes(plain)
        return {'bytes': len(plain), 'packs': len(index_packs(plain)), 'files': len(index_entries(plain))}
    if args.command == 'encode':
        raw = encode_index(args.plain.read_bytes(), args.template.read_bytes()[:INDEX_HEAD])
        args.out.write_bytes(raw)
        return {'bytes': len(raw)}
    if args.command == 'drop':
        plain, removed = drop(args.plain.read_bytes(), args.names)
        args.out.write_bytes(plain)
        return {'removed': removed}
    if args.command == 'sync':
        plain, added = sync(args.plain.read_bytes(), [(p.name, p.read_bytes()) for p in args.packs], ks())
        args.out.write_bytes(plain)
        return {'files': len(index_entries(plain)), 'added': [{'pack': p, 'at': at} for p, at in added]}
    if args.command == 'list':
        return {'files': [{'name': e.name, 'size': len(e.data), 'attr': e.attr} for e in read_pack(args.pack.read_bytes(), ks())]}
    if args.command == 'extract':
        entries = read_pack(args.pack.read_bytes(), ks())
        for e in entries:
            target = args.out / e.name
            if args.out.resolve() not in target.resolve().parents:
                raise HfsError(f'Unsafe file name in pack: {e.name!r}')
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(e.data)
        return {'extracted': len(entries)}
    if args.command == 'build':
        names = args.names or sorted(p.name for p in args.folder.iterdir() if p.is_file())
        entries = [Entry(n, (args.folder / n).read_bytes(), args.attr) for n in names]
        raw = build_pack(entries, ks())
        if read_pack(raw, ks()) != entries:
            raise HfsError('Pack round trip failed.')
        args.out.write_bytes(raw)
        return {'files': len(entries), 'bytes': len(raw)}
    if args.command == 'roundtrip':
        key = ks()
        result = {}
        for path in args.packs:
            raw = path.read_bytes()
            result[path.name] = build_pack(read_pack(raw, key), key) == raw
        return {'identical': result}
    if args.command == 'name':
        return {'pack': pack_name(args.folder, args.part)}
    if args.command == 'keystream':
        key = derive_keystream(args.pack.read_bytes(), plain_folder(args.folder))
        args.out.write_bytes(key)
        return {'bytes': len(key)}
    raise HfsError('Unknown command.')


def parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--keystream', type=Path, help='4096-byte pack keystream (list, extract, build, roundtrip, sync)')
    sub = p.add_subparsers(dest='command', required=True)
    s = sub.add_parser('decode', help='index -> plain table')
    s.add_argument('index', type=Path)
    s.add_argument('out', type=Path)
    s = sub.add_parser('encode', help='plain table -> index, with the 14-byte head of a template index')
    s.add_argument('plain', type=Path)
    s.add_argument('template', type=Path)
    s.add_argument('out', type=Path)
    s = sub.add_parser('drop', help='remove index entries by file name')
    s.add_argument('plain', type=Path)
    s.add_argument('out', type=Path)
    s.add_argument('names', nargs='+')
    s = sub.add_parser('sync', help='point the index at rebuilt or new packs')
    s.add_argument('plain', type=Path)
    s.add_argument('out', type=Path)
    s.add_argument('packs', type=Path, nargs='+')
    s = sub.add_parser('list')
    s.add_argument('pack', type=Path)
    s = sub.add_parser('extract')
    s.add_argument('pack', type=Path)
    s.add_argument('out', type=Path)
    s = sub.add_parser('build', help='folder -> pack (default: sorted file names)')
    s.add_argument('folder', type=Path)
    s.add_argument('out', type=Path)
    s.add_argument('names', nargs='*')
    s.add_argument('--attr', type=lambda v: int(v, 0), default=DEFAULT_ATTR)
    s = sub.add_parser('roundtrip', help='rebuild packs and compare byte for byte')
    s.add_argument('packs', type=Path, nargs='+')
    s = sub.add_parser('name', help='pack name of a folder part')
    s.add_argument('folder')
    s.add_argument('part', type=int, nargs='?', default=0)
    s = sub.add_parser('keystream', help='derive the keystream from a pack and the plain files of its folder')
    s.add_argument('pack', type=Path)
    s.add_argument('folder', type=Path)
    s.add_argument('out', type=Path)
    return p


def main() -> int:
    args = parser().parse_args()
    needs_key = args.command in ('sync', 'list', 'extract', 'build', 'roundtrip')
    if needs_key and args.keystream is None:
        print(json.dumps({'error': '--keystream is required for this command.'}))
        return 2
    try:
        print(json.dumps(run(args)))
    except (HfsError, OSError) as error:
        print(json.dumps({'error': str(error)}))
        return 2
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
