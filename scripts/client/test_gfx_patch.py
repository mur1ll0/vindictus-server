import io
import struct
import unittest

import gfx_patch as g


def tag(code, body):
    return struct.pack('<HI', code << 6 | 0x3F, len(body)) + body


def movie():
    """A GFx movie with one atlas (id 1), a sub-image (2), a shape (3) filled with it, and a class sprite (4)
    whose 'Delia' frame places the shape and shows two frames."""
    atlas = tag(1009, struct.pack('<HHHHH', 1, 9, 14, 1024, 512) + b'\0' + bytes((7,)) + b'atl.dds')
    sub = tag(1008, struct.pack('<6H', 2, 1, 382, 145, 483, 199))
    shape = tag(2, struct.pack('<H', 3) + bytes(8) + b'\x01\x43' + struct.pack('<H', 2) + bytes(6))
    frame = (tag(43, b'Delia\0') + tag(26, bytes((0x06,)) + struct.pack('<HH', 1, 3)) + tag(1, b'') + tag(1, b''))
    lethita = tag(43, b'Lethita\0') + tag(26, bytes((0x06,)) + struct.pack('<HH', 1, 3)) + tag(1, b'')
    sprite = tag(39, struct.pack('<HH', 4, 3) + lethita + frame + tag(0, b''))
    body = bytes((0x08, 0x00)) + struct.pack('<HH', 0, 1) + atlas + sub + shape + sprite + tag(0, b'')
    return b'GFX\x09' + struct.pack('<I', 8 + len(body)) + body


class GfxPatchTest(unittest.TestCase):
    def test_a_class_frame_copies_the_donor_onto_a_new_atlas(self):
        src = movie()
        out = g.with_class_frame(src, 'Miri', 'portrait.dds')
        self.assertEqual(struct.unpack_from('<I', out, 4)[0], len(out))
        top = list(g._tags(out, 8 + 2 + 4, len(out)))
        atlases = [out[b:b + l] for c, _, b, l in top if c == 1009]
        self.assertEqual(len(atlases), 2)
        self.assertEqual(struct.unpack_from('<HHHHH', atlases[1]), (5, 9, 14, *g.ATLAS_SIZE))
        self.assertTrue(atlases[1].endswith(bytes((12,)) + b'portrait.dds'))
        subs = [struct.unpack_from('<6H', out, b) for c, _, b, l in top if c == 1008]
        self.assertEqual(subs[1], (6, 5, 0, 0, *g.PORTRAIT_SIZE))
        shapes = [out[b:b + l] for c, _, b, l in top if c == 2]
        self.assertEqual(struct.unpack_from('<H', shapes[1])[0], 7)
        self.assertEqual(struct.unpack_from('<H', shapes[1], 12)[0], 6)
        sprite = next((b, l) for c, _, b, l in top if c == 39)
        self.assertEqual(struct.unpack_from('<HH', out, sprite[0]), (4, 5))  # two more frames
        inner = list(g._tags(out, sprite[0] + 4, sprite[0] + sprite[1]))
        labels = [out[b:out.index(b'\0', b)] for c, _, b, l in inner if c == 43]
        self.assertEqual(labels, [b'Lethita', b'Delia', b'Miri'])
        placed = [struct.unpack_from('<H', out, b + 3)[0] for c, _, b, l in inner if c == 26]
        self.assertEqual(placed, [3, 3, 7])
        self.assertEqual(inner[-1][0], 0)
        self.assertIsNone(g.with_class_frame(out, 'Miri', 'portrait.dds'))  # already there
        with self.assertRaises(g.GfxError):
            g.with_class_frame(src, 'Miri', 'portrait.dds', donor='Kay')

    def test_an_array_entry_is_appended_to_the_root_actions(self):
        # push "labels" twice; pop; end
        body = bytes((0x08, 0x00)) + struct.pack('<HH', 0, 1) + tag(12, b'\x96\x09\x00\x00labels\x00\x00\x17\x00') + tag(0, b'')
        src = b'GFX\x09' + struct.pack('<I', 8 + len(body)) + body
        out = g.with_array_entry(src, 'labels', 11, 'Miri')
        self.assertEqual(struct.unpack_from('<I', out, 4)[0], len(out))
        top = list(g._tags(out, 8 + 2 + 4, len(out)))
        block = next(out[b:b + l] for c, _, b, l in top if c == 12)
        self.assertTrue(block.startswith(b'\x96\x09\x00\x00labels\x00\x00\x17'))
        self.assertTrue(block.endswith(b'\x96\x08\x00\x00labels\x00\x1c\x96\x0b\x00\x07\x0b\x00\x00\x00\x00Miri\x00\x4f\x00'))
        self.assertIsNone(g.with_array_entry(out, 'labels', 11, 'Miri'))
        with self.assertRaises(g.GfxError):
            g.with_array_entry(src, 'other', 11, 'Miri')

    def test_the_portrait_is_a_dxt5_atlas(self):
        from PIL import Image
        bust = Image.new('RGBA', (95, 95), (200, 100, 50, 255))
        raw = io.BytesIO()
        bust.save(raw, 'PNG')
        dds = g.class_portrait_dds(raw.getvalue())
        self.assertEqual(dds[84:88], b'DXT5')
        atlas = Image.open(io.BytesIO(dds)).convert('RGBA')
        self.assertEqual(atlas.size, g.ATLAS_SIZE)
        self.assertEqual(atlas.getpixel((g.PORTRAIT_SIZE[0] // 2, 10))[3], 255)  # the bust, centred
        self.assertEqual(atlas.getpixel((g.ATLAS_SIZE[0] - 1, g.ATLAS_SIZE[1] - 1))[3], 0)


def place(depth, character, x, y, name):
    return tag(26, bytes((0x26,)) + struct.pack('<HH', depth, character) + g._translate(x, y) + name.encode() + b'\0')


def grid_movie():
    """The grid's shape: an atlas (1) with a 70x136 icon (2) filling a shape (3), a button sprite (4) placing
    it and a hit box (5), the grid sprite (6) with the Hagie and Delia buttons, the panel sprite (7) with
    MC_Delia and a skill button above it, and the root actions naming characterTypeArray."""
    atlas = tag(1009, struct.pack('<HHHHH', 1, 9, 14, 1024, 1024) + b'\0' + bytes((7,)) + b'i3a.dds')
    sub = tag(1008, struct.pack('<6H', 2, 1, 72, 382, 142, 518))
    shape = tag(2, struct.pack('<H', 3) + bytes(8) + b'\x01\x41' + struct.pack('<H', 2) + bytes(6))
    hit = tag(39, struct.pack('<HH', 5, 1) + tag(1, b'') + tag(0, b''))
    button = tag(39, struct.pack('<HH', 4, 2) + tag(43, b'Normal\0') + tag(26, bytes((0x06,)) + struct.pack('<HH', 1, 3) + b'\0')
                 + tag(26, bytes((0x06,)) + struct.pack('<HH', 4, 5) + b'\0') + tag(1, b'') + tag(1, b'') + tag(0, b''))
    grid = tag(39, struct.pack('<HH', 6, 1) + place(1, 4, 72, 140, 'MC_Button_Delia') + place(6, 4, 0, 140, 'MC_Button_Hagie')
               + tag(1, b'') + tag(0, b''))
    panels = tag(39, struct.pack('<HH', 7, 1) + place(153, 5, 0, -262, 'MC_Delia') + place(168, 5, 0, 135, 'Button_Skill_PreView1')
                 + tag(1, b'') + tag(0, b''))
    root = tag(12, b'\x96\x14\x00\x00characterTypeArray\x00\x17\x00')
    body = bytes((0x08, 0x00)) + struct.pack('<HH', 0, 1) + atlas + sub + shape + hit + button + panels + grid + root + tag(0, b'')
    return b'GFX\x09' + struct.pack('<I', 8 + len(body)) + body


class GridTest(unittest.TestCase):
    def test_the_translate_matrix_is_the_movies_own_encoding(self):
        self.assertEqual(g._translate(72, 140).hex(), '1a5a057800')  # MC_Button_Delia in the 1.69 movie
        record = g._translate(-31, -296.9)
        self.assertEqual(g._matrix_end(record, 0), len(record))

    def test_a_button_shows_its_atlas_rect(self):
        self.assertEqual(g.button_icon(grid_movie(), 'MC_Button_Delia'), ('i3a.dds', (72, 382, 142, 518)))
        with self.assertRaises(g.GfxError):
            g.button_icon(grid_movie(), 'MC_Button_Miri')

    def test_a_class_button_copies_the_donor_onto_its_own_icon(self):
        out = g.with_class_button(grid_movie(), 'Miri', 'NewCharacterDialog_V2_IMiri.dds', (70, 136), (144, 140))
        self.assertEqual(struct.unpack_from('<I', out, 4)[0], len(out))
        data, _, first = g._open(out)
        top = list(g._tags(data, first, len(data)))
        defines = g._defines(data, top)
        k, button = g._named_place(data, top, 'MC_Button_Miri')
        self.assertEqual((button['depth'], button['matrix']), (11, g._translate(144, 140)))  # next depth step
        self.assertEqual(g.button_icon(out, 'MC_Button_Miri'), ('NewCharacterDialog_V2_IMiri.dds', (0, 0, 70, 136)))
        self.assertEqual(struct.unpack_from('<HHHHH', data, defines[8][1]), (8, 9, 14, *g.ICON_ATLAS_SIZE))
        # the copy keeps the donor's frames and hit box, with only the icon shape changed
        inner = [g._place(data, b)['character'] for c, _, b, _ in g._tags(data, defines[11][1] + 4, sum(defines[11][1:]))
                 if c == 26]
        self.assertEqual(inner, [10, 5])
        self.assertLess(top.index(next(t for t in top if t[0] == 39 and struct.unpack_from('<H', data, t[2])[0] == 11)), k)
        _, panel = g._named_place(data, top, 'MC_Miri')
        self.assertEqual((panel['depth'], panel['character'], panel['matrix']), (154, 5, g._translate(0, -262)))
        self.assertIsNone(g.with_class_button(out, 'Miri', 'x.dds', (70, 136), (144, 140)))
        with self.assertRaises(g.GfxError):
            g.with_class_button(grid_movie(), 'Miri', 'x.dds', (70, 136), (144, 140), donor='Kay')

    def test_the_grid_actions_run_after_the_frame_code(self):
        code = g.grid_class_actions('Miri', 11, 'Delia', 'DRAKER', ('video/skill/skill_miri_sp5.usm',))
        out = g.with_root_actions(grid_movie(), 'characterTypeArray', code)
        data, _, first = g._open(out)
        block = next(data[b:b + l] for c, _, b, l in g._tags(data, first, len(data)) if c == 12)
        self.assertTrue(block.startswith(b'\x96\x14\x00\x00characterTypeArray\x00\x17'))
        self.assertTrue(block.endswith(code + b'\x00'))
        self.assertIsNone(g.with_root_actions(out, 'characterTypeArray', code))
        # characterTypeArray[11] = 'Miri' first, then the texts the movie asks the client for
        self.assertTrue(code.startswith(g.As2().path('characterTypeArray').push(11, 'Miri').op(g.As2.SET_MEMBER).assemble()))
        for text in (b'GameUI_Heroes_NewCharacterDialog_Type_Comment_Miri', b'Heroes_Item_Equip_Class_Name_DRAKER',
                     b'MC_Button_Miri', b'MC_Button_Delia', b'onRollOver', b'addCharacterInfo', b'Text_SkillPreviewUsm1',
                     b'Text_CharacterPreviewUsm',
                     b'video/skill/skill_miri_sp5.usm', b'_hc_showRolloverPanel', b'_hc_showCharacterSummary'):
            self.assertIn(text + b'\0', code)
        with self.assertRaises(g.GfxError):
            g.with_root_actions(grid_movie(), 'otherArray', code)

    def test_wrapped_functions_skip_to_their_end_for_other_classes(self):
        body = g.As2().push(g.Reg(3), True).op(g.As2.STRICT_EQUALS).if_not('end').push(1).op(g.As2.POP).label('end')
        code = body.assemble()
        self.assertEqual(code[-1], g.As2.POP)
        self.assertEqual(struct.unpack_from('<h', code, code.index(bytes((g.As2.IF,))) + 3)[0], 9)  # over push 1; pop
        function = g.As2().function(('posX', 'posY', 'charType'), body).assemble()
        self.assertEqual(function[0], g.As2.DEFINE_FUNCTION2)
        head = struct.unpack_from('<H', function, 1)[0]
        self.assertEqual(function[3:9], b'\0' + struct.pack('<HBBB', 3, 4, 0x2A, 0))
        self.assertEqual(struct.unpack_from('<H', function, 3 + head - 2)[0], len(code))
        self.assertEqual(function[3 + head:], code)

    def test_the_class_runs_the_slider_functions_as_the_donor(self):
        code = g.class_alias_actions(11, 10)
        for function, params in g.CLASS_SLIDER_FUNCTIONS:
            self.assertIn(f'_hc_{function}'.encode() + b'\0', code)
        # one wrapper: save the class (r2), swap 11 for 10, call, put g_selectedCharacter back, return the result
        one = g.class_alias_actions(11, 10, g.CLASS_SLIDER_FUNCTIONS[:1])
        head = one.index(bytes((g.As2.DEFINE_FUNCTION2,)))
        self.assertEqual(one[head + 3:head + 9], b'\0' + struct.pack('<HBBB', 1, 3, 0x2A, 0))
        body = one[head + 3 + struct.unpack_from('<H', one, head + 1)[0]:]
        self.assertTrue(body.startswith(g.As2().push(g.Reg(1)).store(2).op(g.As2.POP).assemble()))
        self.assertIn(g.As2().push(10).store(1).op(g.As2.POP).assemble(), body)
        self.assertIn(g.As2().push('g_selectedCharacter', 11).op(g.As2.SET_VARIABLE).assemble(), body)
        self.assertIn(bytes((g.As2.RETURN,)), body)


if __name__ == '__main__':
    unittest.main()
