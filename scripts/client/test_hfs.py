import random
import struct
import unittest
import zlib
from pathlib import Path

import hfs
from hfs import Entry, HfsError

# Synthetic keystream: the real one is game content and is never versioned.
KS = bytes(random.Random(169).randrange(256) for _ in range(4096))
HEAD = b'HF\x01\x02\x0a\x00' + bytes(8)
LAB = Path(__file__).resolve().parents[2] / '.cache/play-client'


def index(packs: list[str], files: list[tuple[str, int, int, int]]) -> bytes:
    entries = sorted((hfs.name_key(n) + struct.pack('<HII', p, at, size) for n, p, at, size in files),
                     key=lambda e: struct.unpack_from('<I', e)[0])
    return hfs.assemble_index(hfs.INDEX_MAGIC + bytes(16), packs, entries)


def listing(plain: bytes) -> dict[str, tuple[int, int, int]]:
    return {e[:6]: struct.unpack_from('<HII', e, 6) for e in hfs.index_entries(plain)}


A, B, C = '1' * 40, '5' * 40, '9' * 40


class IndexTest(unittest.TestCase):
    def test_round_trip_keeps_the_head_and_rejects_bad_counts(self):
        plain = index([A, C], [('a.txt', 0, 10, 3), ('b.mdl', 1, 20, 4)])
        raw = hfs.encode_index(plain, HEAD)
        self.assertEqual(raw[:14], HEAD)
        self.assertEqual(hfs.decode_index(raw), plain)
        self.assertEqual(hfs.index_packs(plain), [A, C])
        with self.assertRaises(HfsError):
            hfs.decode_index(HEAD + hfs.xor_index(zlib.compress(plain[:-1])))

    def test_drop_removes_entries_by_file_name_case_insensitively(self):
        plain = index([A], [('heroes_text.txt', 0, 1, 1), ('keep.txt', 0, 2, 1)])
        out, removed = hfs.drop(plain, ['HEROES_TEXT.txt', 'absent.txt'])
        self.assertEqual(removed, 1)
        self.assertEqual(list(listing(out)), [hfs.name_key('keep.txt')])

    def test_sync_replaces_the_entries_of_an_existing_pack(self):
        plain = index([A, C], [('old.txt', 0, 50, 5), ('other.txt', 1, 60, 6)])
        pack = hfs.build_pack([Entry('new.txt', b'abc'), Entry('item.txt.comp', hfs.comp_wrap(b'x' * 100))], KS)
        out, added = hfs.sync(plain, [(A + '.hfs', pack)], KS)
        self.assertEqual(added, [])
        files = listing(out)
        self.assertNotIn(hfs.name_key('old.txt'), files)
        self.assertEqual(files[hfs.name_key('other.txt')], (1, 60, 6))
        positions = {n: (at, size) for n, at, size in hfs.pack_files(pack, KS)}
        self.assertEqual(files[hfs.name_key('new.txt')], (0,) + positions['new.txt'])
        self.assertEqual(files[hfs.name_key('item.txt')], (0,) + positions['item.txt'])
        hashes = [struct.unpack_from('<I', e)[0] for e in hfs.index_entries(out)]
        self.assertEqual(hashes, sorted(hashes))

    def test_sync_inserts_a_new_pack_in_order_and_shifts_later_indices(self):
        plain = index([A, C], [('a.txt', 0, 1, 1), ('c.txt', 1, 2, 2)])
        pack = hfs.build_pack([Entry('model.mdl.comp', hfs.comp_wrap(b'm'))], KS)
        out, added = hfs.sync(plain, [(B.lower() + '.hfs', pack)], KS)
        self.assertEqual(added, [(B, 1)])
        self.assertEqual(hfs.index_packs(out), [A, B, C])
        files = listing(out)
        self.assertEqual(files[hfs.name_key('a.txt')], (0, 1, 1))
        self.assertEqual(files[hfs.name_key('c.txt')], (2, 2, 2))
        self.assertEqual(files[hfs.name_key('model.mdl')][0], 1)
        self.assertEqual(hfs.decode_index(hfs.encode_index(out, HEAD)), out)

    def test_sync_refuses_a_pack_not_named_by_its_hash(self):
        with self.assertRaises(HfsError):
            hfs.sync(index([A], []), [('custom.hfs', hfs.build_pack([], KS))], KS)


class PackTest(unittest.TestCase):
    def test_round_trip_keeps_attributes_and_hides_names_and_data(self):
        entries = [Entry('a.txt', b'hello world' * 50, 0x20), Entry('b.mdl.comp', hfs.comp_wrap(b'\0' * 999), 0x80)]
        raw = hfs.build_pack(entries, KS)
        self.assertEqual(raw[:4], hfs.SIG)
        self.assertNotIn(b'hello', raw)
        self.assertNotIn(b'a.txt', raw)
        self.assertEqual(hfs.read_pack(raw, KS), entries)
        self.assertEqual(hfs.build_pack(hfs.read_pack(raw, KS), KS), raw)

    def test_a_wrong_keystream_fails_the_crc(self):
        raw = hfs.build_pack([Entry('a.txt', b'data')], KS)
        with self.assertRaises(HfsError):
            hfs.read_pack(raw, bytes(4096))

    def test_comp_files_are_listed_without_comp(self):
        raw = hfs.build_pack([Entry('player_costume.txt.comp', hfs.comp_wrap(b'text')), Entry('a.vtf', b'v')], KS)
        self.assertEqual([n for n, _, _ in hfs.pack_files(raw, KS)], ['player_costume.txt', 'a.vtf'])
        self.assertEqual(hfs.comp_unwrap(hfs.read_pack(raw, KS)[0].data), b'text')

    def test_pack_name_matches_the_client_summary(self):
        self.assertEqual(hfs.pack_name('models/armor/kingdom'), '0000AA13EFACC8399BA8F63B23986D12A7697817')
        self.assertEqual(hfs.pack_name('maps/16l'), '0050CB1A909FD1018A268223050D89E02C83472F')
        self.assertEqual(hfs.pack_name('materials\\world\\Nature', 1), '00A6B53F4708149694C70B4A0C3F3036C8F2AA00')
        self.assertEqual(hfs.pack_name('scaleform/video/createcharacter', 20),'0049F6BD1FA09CF4E5DD8C8619432EC2D1D291F8')

    def test_keystream_is_derived_from_a_pack_and_its_plain_files(self):
        plain = {'big.txt': bytes(range(256)) * 20, 'model.mdl': b'mdl' * 30}
        raw = hfs.build_pack([Entry('big.txt', plain['big.txt']), Entry('model.mdl.comp', hfs.comp_wrap(plain['model.mdl']))], KS)
        self.assertEqual(hfs.derive_keystream(raw, plain), KS)
        with self.assertRaises(HfsError):
            hfs.derive_keystream(raw, {'model.mdl': plain['model.mdl']})


@unittest.skipUnless((LAB / 'l0-hfs-keystream.bin').is_file() and (LAB / 'hfs').is_dir(), 'lab client copy absent')
class LabClientTest(unittest.TestCase):
    """Read-only check against the lab copy of the 1.69 EN client."""

    def test_real_index_and_packs_re_encode_byte_for_byte(self):
        ks = hfs.load_keystream(LAB / 'l0-hfs-keystream.bin')
        # With laboratory items applied (apply_lab_content_additions.py), the original index and the packs
        # it rebuilt are in lab-backup/; sample only packs the laboratory never touched.
        backup = LAB / 'lab-backup'
        original = backup / 'E608DF10F284E19AB75FD7D0D8AE9CCF87831041.hfs'
        raw = (original if original.is_file() else LAB / 'hfs/E608DF10F284E19AB75FD7D0D8AE9CCF87831041.hfs').read_bytes()
        plain = hfs.decode_index(raw)
        self.assertEqual(hfs.encode_index(plain, raw[:hfs.INDEX_HEAD]), raw)
        packs = [n for n in hfs.index_packs(plain)
                 if (LAB / f'hfs/{n}.hfs').is_file() and not (backup / f'{n}.hfs').is_file()][::400]
        self.assertTrue(packs)
        for name in packs:
            pack = (LAB / f'hfs/{name}.hfs').read_bytes()
            self.assertEqual(hfs.build_pack(hfs.read_pack(pack, ks), ks), pack, name)
        # Syncing unchanged packs reproduces the original index.
        self.assertEqual(hfs.sync(plain, [(n, (LAB / f'hfs/{n}.hfs').read_bytes()) for n in packs], ks)[0], plain)

    @unittest.skipUnless((LAB / 'hfs_extracted/models/armor/kingdom').is_dir(), 'extracted files absent')
    def test_keystream_derived_from_a_real_pack_matches_the_lab_table(self):
        pack = (LAB / 'hfs/0000AA13EFACC8399BA8F63B23986D12A7697817.hfs').read_bytes()
        plain = hfs.plain_folder(LAB / 'hfs_extracted/models/armor/kingdom')
        self.assertEqual(hfs.derive_keystream(pack, plain), hfs.load_keystream(LAB / 'l0-hfs-keystream.bin'))


if __name__ == '__main__':
    unittest.main()
