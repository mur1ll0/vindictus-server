"""Add a class to the 1.69 character list portrait (scaleform/SelectCharacterDialog_V2.gfx).

The movie keeps one portrait per class in a sprite whose frames are labelled with the class name (Lethita
.. Delia); the client sends the class name (patch_class_routing CLASS_STRINGS, "{Name}") and a name with no
frame stays on the first one, Lethita's: the Miri's list portrait was a man's face (C4 MI4-4). The name
does not come from the client as such: the movie's root code maps the class number to the label through
`var characterImageLabel = new Array("Lethita", .., "Delia")` (11 entries), so the class also needs its
entry there (with_array_entry; C4 MI20: the frame alone changed nothing). Each frame
places a shape filled with a 101x54 sub-image (GFx DefineSubImage, tag 1008) of an external DDS atlas
(GFx DefineExternalImage2, tag 1009). with_class_frame copies the donor class's frame, shape and sub-image
with new ids onto a new atlas holding the class's portrait (class_portrait_dds, from the NA's
scaleform/player_portrait/c_info_<class>.tga, a 95x95 bust).
"""
from __future__ import annotations

import io
import struct
import zlib

SUB_IMAGE, EXTERNAL_IMAGE, SPRITE, FRAME_LABEL, SHOW_FRAME, PLACE_OBJECT2, END = 1008, 1009, 39, 43, 1, 26, 0
SHAPES = (2, 22, 32, 83)
# tags whose body starts with the character id they define
DEFINES = {2, 6, 10, 11, 13, 14, 20, 21, 22, 32, 33, 34, 35, 36, 37, 39, 46, 48, 60, 75, 83, 84, 87, 88,
           1001, 1008, 1009}
PORTRAIT_SIZE = (101, 54)
ATLAS_SIZE = (128, 64)
BUST_SCALE = 1.35  # bust side / portrait height


class GfxError(ValueError):
    pass


def class_portrait_dds(portrait: bytes) -> bytes:
    """The class's list portrait (the top of the bust, PORTRAIT_SIZE) at the top left of a DXT5 atlas."""
    from PIL import Image
    bust = Image.open(io.BytesIO(portrait)).convert('RGBA')
    width, height = PORTRAIT_SIZE
    # head to shoulders, centred, as the 1.69 portraits frame their busts
    side = round(height * BUST_SCALE)
    scaled = bust.resize((side, round(bust.height * side / bust.width)), Image.LANCZOS)
    atlas = Image.new('RGBA', ATLAS_SIZE, (0, 0, 0, 0))
    atlas.paste(scaled.crop((0, 0, side, height)), ((width - side) // 2, 0))
    out = io.BytesIO()
    atlas.save(out, 'DDS', pixel_format='DXT5')
    return out.getvalue()


def _tags(data: bytes, at: int, end: int):
    while at < end:
        code_len = struct.unpack_from('<H', data, at)[0]
        code, length, head = code_len >> 6, code_len & 0x3F, 2
        if length == 0x3F:
            length, head = struct.unpack_from('<I', data, at + 2)[0], 6
        yield code, at, at + head, length
        at += head + length
        if code == END:
            return


def _tag(code: int, body: bytes, long: bool = True) -> bytes:
    if not long and len(body) < 0x3F:
        return struct.pack('<H', code << 6 | len(body)) + body
    return struct.pack('<HI', code << 6 | 0x3F, len(body)) + body


DO_ACTION = 12


def _open(movie: bytes) -> tuple[bytes, bool, int]:
    """The uncompressed movie, whether it was compressed, and the offset of its first tag."""
    if movie[:3] not in (b'GFX', b'CFX'):
        raise GfxError('Not a GFx movie.')
    compressed = movie[:3] == b'CFX'
    data = movie[:8] + zlib.decompress(movie[8:]) if compressed else movie
    return data, compressed, 8 + (5 + 4 * (data[8] >> 3) + 7) // 8 + 4


def _close(data: bytearray, compressed: bool) -> bytes:
    struct.pack_into('<I', data, 4, len(data))
    return bytes(data[:8]) + zlib.compress(bytes(data[8:])) if compressed else bytes(data)


def with_array_entry(movie: bytes, array: str, index: int, value: str) -> bytes | None:
    """The movie with `array[index] = value` appended to the root action block that names the array (before
    its End action, so no earlier offset moves); None when it is already there."""
    data, compressed, first = _open(movie)
    code = (b'\x96' + struct.pack('<H', len(array) + 2) + b'\x00' + array.encode('latin-1') + b'\x00' + b'\x1c'
            + b'\x96' + struct.pack('<H', len(value) + 7) + b'\x07' + struct.pack('<i', index) + b'\x00'
            + value.encode('latin-1') + b'\x00' + b'\x4f')
    for tag, start, body, length in _tags(data, first, len(data)):
        if tag != DO_ACTION or array.encode('latin-1') + b'\0' not in data[body:body + length]:
            continue
        if code in data[body:body + length]:
            return None
        if data[body + length - 1] != 0:
            raise GfxError('The action block does not end with End.')
        block = data[body:body + length - 1] + code + b'\x00'
        return _close(bytearray(data[:start] + _tag(DO_ACTION, block) + data[body + length:]), compressed)
    raise GfxError(f'No root action block names {array}.')


def with_class_frame(movie: bytes, label: str, image_file: str, donor: str = 'Delia') -> bytes | None:
    """The movie with a `label` frame showing `image_file` (an ATLAS_SIZE DDS, portrait at its top left),
    built from the `donor` frame; None when the movie already has the label."""
    compressed = movie[:3] == b'CFX'
    if movie[:3] not in (b'GFX', b'CFX'):
        raise GfxError('Not a GFx movie.')
    data = movie[:8] + zlib.decompress(movie[8:]) if compressed else movie
    nbits = data[8] >> 3
    first = 8 + (5 + 4 * nbits + 7) // 8 + 4
    top = list(_tags(data, first, len(data)))
    ids = [struct.unpack_from('<H', data, body)[0] for code, _, body, length in top if code in DEFINES and length >= 2]
    sprite = None
    for code, start, body, length in top:
        if code != SPRITE:
            continue
        inner = list(_tags(data, body + 4, body + length))
        labels = [data[b:data.index(b'\0', b)].decode('latin-1') for c, _, b, _ in inner if c == FRAME_LABEL]
        if label in labels:
            return None
        if donor in labels:
            sprite = (start, body, length, inner)
            break
    if sprite is None:
        raise GfxError(f'No sprite has a {donor} frame.')
    start, body, length, inner = sprite
    # the donor's frame block: its label up to the next label or the end
    at = next(k for k, (c, _, b, _) in enumerate(inner) if c == FRAME_LABEL and data[b:data.index(b'\0', b)] == donor.encode())
    block = []
    for c, s, b, l in inner[at:]:
        if c == END or (c == FRAME_LABEL and block):
            break
        block.append((c, s, b, l))
    places = [(s, b) for c, s, b, l in block if c == PLACE_OBJECT2 and data[b] & 2]
    if len(places) != 1:
        raise GfxError(f'The {donor} frame places {len(places)} characters.')
    shape_id = struct.unpack_from('<H', data, places[0][1] + 3)[0]
    shape = next(((c, b, l) for c, _, b, l in top if c in SHAPES and struct.unpack_from('<H', data, b)[0] == shape_id), None)
    subs = {struct.unpack_from('<H', data, b)[0]: (b, l) for c, _, b, l in top if c == SUB_IMAGE}
    atlases = {struct.unpack_from('<H', data, b)[0]: (b, l) for c, _, b, l in top if c == EXTERNAL_IMAGE}
    if shape is None:
        raise GfxError(f'No shape {shape_id}.')
    code, shape_body, shape_length = shape
    shape_bytes = bytearray(data[shape_body:shape_body + shape_length])
    fills = [k for k in range(2, len(shape_bytes) - 2)
             if shape_bytes[k] in (0x40, 0x41, 0x42, 0x43) and struct.unpack_from('<H', shape_bytes, k + 1)[0] in subs]
    if len(fills) != 1:
        raise GfxError(f'Shape {shape_id} has {len(fills)} bitmap fills of a sub-image.')
    sub_body, _ = subs[struct.unpack_from('<H', shape_bytes, fills[0] + 1)[0]]
    atlas_body, atlas_length = atlas = atlases[struct.unpack_from('<H', data, sub_body + 2)[0]]
    new_atlas, new_sub, new_shape = max(ids) + 1, max(ids) + 2, max(ids) + 3
    # DefineExternalImage2: id, format, flags (the donor atlas's), size, export name (none), file name
    head = bytearray(data[atlas_body:atlas_body + 6])
    struct.pack_into('<H', head, 0, new_atlas)
    name = image_file.encode('latin-1')
    atlas_tag = _tag(EXTERNAL_IMAGE, bytes(head) + struct.pack('<HH', *ATLAS_SIZE) + b'\0' + bytes((len(name),)) + name, False)
    sub_tag = _tag(SUB_IMAGE, struct.pack('<6H', new_sub, new_atlas, 0, 0, *PORTRAIT_SIZE), False)
    struct.pack_into('<H', shape_bytes, 0, new_shape)
    struct.pack_into('<H', shape_bytes, fills[0] + 1, new_sub)
    shape_tag = _tag(code, bytes(shape_bytes), shape_length >= 0x3F)
    # the frame: the donor's tags with the label and the placed character changed
    frame = b''
    frames = 0
    for c, s, b, l in block:
        tag = bytearray(data[s:b + l])
        if c == FRAME_LABEL:
            frame += _tag(FRAME_LABEL, label.encode('latin-1') + b'\0', False)
            continue
        if c == PLACE_OBJECT2 and data[b] & 2:
            struct.pack_into('<H', tag, (b - s) + 3, new_shape)
        frames += c == SHOW_FRAME
        frame += bytes(tag)
    end_at = next(s for c, s, b, l in inner if c == END)
    sprite_body = bytearray(data[body:end_at]) + frame + data[end_at:body + length]
    struct.pack_into('<H', sprite_body, 2, struct.unpack_from('<H', sprite_body, 2)[0] + frames)
    out = bytearray(data[:start] + atlas_tag + sub_tag + shape_tag + _tag(SPRITE, bytes(sprite_body))
                    + data[body + length:])
    struct.pack_into('<I', out, 4, len(out))
    if compressed:
        return bytes(out[:8]) + zlib.compress(bytes(out[8:]))
    return bytes(out)


# The class grid of the character creation (scaleform/NewCharacterDialog_V2.gfx, NewCharacterDialogS3). The
# 1.69 movie lists its eleven classes in fixed AS2: `characterTypeArray` (index -> class name), one button
# sprite per class placed in MC_SelectCharacter as MC_Button_<Class> (a 3-column grid, 72 x 138 apart),
# showing a 70x136 sub-image of the NewCharacterDialog_V2_I3.dds atlas, and one panel MC_Video.MC_<Class>
# (all the same sprite) whose texts and videos the code fills; the frame loops wire the buttons and fill the
# panels, and switches over the eleven names give each its comment, rollover title and weapon. The NA movie
# has the same buttons for its 28 classes (MC_Button_Miri), filled the same way from its own atlas.
ICON_ATLAS_SIZE = (128, 256)
PLACE_HAS_CHARACTER, PLACE_HAS_MATRIX, PLACE_HAS_COLOR, PLACE_HAS_RATIO, PLACE_HAS_NAME = 2, 4, 8, 16, 32


def _matrix_end(data: bytes, at: int) -> int:
    """The byte after the SWF MATRIX record at `at`."""
    bit = at * 8

    def read(n: int) -> int:
        nonlocal bit
        value = 0
        for _ in range(n):
            value = value << 1 | data[bit >> 3] >> (7 - (bit & 7)) & 1
            bit += 1
        return value
    for _ in range(2):  # scale, rotate/skew
        if read(1):
            read(2 * read(5))
    read(2 * read(5))
    return (bit + 7) // 8


def _translate(x: float, y: float) -> bytes:
    """A MATRIX record that only translates (in pixels)."""
    tx, ty = round(x * 20), round(y * 20)
    n = max(v.bit_length() + 1 for v in (tx, ty, 1))
    bits = '00' + format(n, '05b') + ''.join(format(v & (1 << n) - 1, f'0{n}b') for v in (tx, ty))
    bits += '0' * (-len(bits) % 8)
    return bytes(int(bits[k:k + 8], 2) for k in range(0, len(bits), 8))


def _place(data: bytes, body: int) -> dict:
    """The fields of a PlaceObject2 body: flags, depth, character, matrix bytes, name."""
    flags, depth = data[body], struct.unpack_from('<H', data, body + 1)[0]
    at = body + 3
    out = {'flags': flags, 'depth': depth, 'character': None, 'matrix': b'', 'name': None}
    if flags & PLACE_HAS_CHARACTER:
        out['character'] = struct.unpack_from('<H', data, at)[0]
        at += 2
    if flags & PLACE_HAS_MATRIX:
        end = _matrix_end(data, at)
        out['matrix'], at = data[at:end], end
    if flags & PLACE_HAS_NAME and not flags & (PLACE_HAS_COLOR | PLACE_HAS_RATIO):
        out['name'] = data[at:data.index(b'\0', at)].decode('latin-1')
    return out


def _place_tag(depth: int, character: int, matrix: bytes, name: str) -> bytes:
    flags = PLACE_HAS_CHARACTER | PLACE_HAS_MATRIX | PLACE_HAS_NAME
    return _tag(PLACE_OBJECT2, bytes((flags,)) + struct.pack('<HH', depth, character) + matrix
                + name.encode('latin-1') + b'\0', False)


def _named_place(data: bytes, top: list, name: str) -> tuple[int, dict]:
    """(index in `top` of the sprite, fields) of the PlaceObject2 named `name`."""
    for k, (code, _, body, length) in enumerate(top):
        if code != SPRITE:
            continue
        for c, _, b, _ in _tags(data, body + 4, body + length):
            if c == PLACE_OBJECT2 and data[b] & PLACE_HAS_NAME:
                place = _place(data, b)
                if place['name'] == name:
                    return k, place
    raise GfxError(f'No sprite places {name}.')


def _defines(data: bytes, top: list) -> dict[int, tuple[int, int, int]]:
    """Character id -> (tag code, body offset, body length) of every top-level define."""
    return {struct.unpack_from('<H', data, b)[0]: (c, b, l) for c, _, b, l in top if c in DEFINES and l >= 2}


def _icon_shape(data: bytes, top: list, sprite_id: int) -> tuple[int, int]:
    """(shape id, sub-image id) of the first shape a button sprite places that is filled with a sub-image."""
    defines = _defines(data, top)
    _, body, length = defines[sprite_id]
    for c, _, b, _ in _tags(data, body + 4, body + length):
        if c != PLACE_OBJECT2 or not data[b] & PLACE_HAS_CHARACTER:
            continue
        shape = _place(data, b)['character']
        code, sb, sl = defines.get(shape, (None, 0, 0))
        if code not in SHAPES:
            continue
        fills = [struct.unpack_from('<H', data, k + 1)[0] for k in range(sb + 2, sb + sl - 2)
                 if data[k] in (0x40, 0x41, 0x42, 0x43)
                 and defines.get(struct.unpack_from('<H', data, k + 1)[0], (None,))[0] == SUB_IMAGE]
        if len(fills) == 1:
            return shape, fills[0]
    raise GfxError(f'Sprite {sprite_id} places no shape filled with a sub-image.')


def button_icon(movie: bytes, button: str) -> tuple[str, tuple[int, int, int, int]]:
    """(atlas file, (x1, y1, x2, y2)) of the icon a grid button (MC_Button_<Class>) shows."""
    data, _, first = _open(movie)
    top = list(_tags(data, first, len(data)))
    _, place = _named_place(data, top, button)
    _, sub = _icon_shape(data, top, place['character'])
    defines = _defines(data, top)
    _, atlas, *rect = struct.unpack_from('<6H', data, defines[sub][1])
    code, body, _ = defines.get(atlas, (None, 0, 0))
    if code != EXTERNAL_IMAGE:
        raise GfxError(f'No atlas {atlas}.')
    # id, format, flags, width, height, export name (one length byte), file name (one length byte)
    at = body + 11 + data[body + 10]
    return data[at + 1:at + 1 + data[at]].decode('latin-1'), tuple(rect)


def class_icon_dds(atlas: bytes, rect: tuple[int, int, int, int]) -> bytes:
    """The grid icon `rect` of an atlas at the top left of a DXT5 ICON_ATLAS_SIZE atlas."""
    from PIL import Image
    icon = Image.open(io.BytesIO(atlas)).convert('RGBA').crop(rect)
    out = Image.new('RGBA', ICON_ATLAS_SIZE, (0, 0, 0, 0))
    out.paste(icon, (0, 0))
    raw = io.BytesIO()
    out.save(raw, 'DDS', pixel_format='DXT5')
    return raw.getvalue()


def with_class_button(movie: bytes, label: str, image_file: str, size: tuple[int, int], cell: tuple[float, float],
                      donor: str = 'Delia') -> bytes | None:
    """The grid movie with MC_Button_<label> at `cell` of the button grid (a copy of the donor's button sprite
    showing the `size` icon at the top left of `image_file`) and the panel MC_Video.MC_<label> (the donor's
    panel sprite, one depth above it); None when the button is already there."""
    data, compressed, first = _open(movie)
    top = list(_tags(data, first, len(data)))
    try:
        _named_place(data, top, f'MC_Button_{label}')
        return None
    except GfxError:
        pass
    grid, button = _named_place(data, top, f'MC_Button_{donor}')
    panels, panel = _named_place(data, top, f'MC_{donor}')
    shape_id, sub_id = _icon_shape(data, top, button['character'])
    defines = _defines(data, top)
    new_atlas, new_sub, new_shape, new_button = (max(defines) + k for k in range(1, 5))
    _, atlas_body, _ = defines[struct.unpack_from('<H', data, defines[sub_id][1] + 2)[0]]
    head = bytearray(data[atlas_body:atlas_body + 6])
    struct.pack_into('<H', head, 0, new_atlas)
    name = image_file.encode('latin-1')
    added = _tag(EXTERNAL_IMAGE, bytes(head) + struct.pack('<HH', *ICON_ATLAS_SIZE) + b'\0' + bytes((len(name),)) + name,
                 False)
    added += _tag(SUB_IMAGE, struct.pack('<6H', new_sub, new_atlas, 0, 0, *size), False)
    code, shape_body, shape_length = defines[shape_id]
    shape = bytearray(data[shape_body:shape_body + shape_length])
    struct.pack_into('<H', shape, 0, new_shape)
    fills = [k for k in range(2, len(shape) - 2) if shape[k] in (0x40, 0x41, 0x42, 0x43)
             and struct.unpack_from('<H', shape, k + 1)[0] == sub_id]
    struct.pack_into('<H', shape, fills[0] + 1, new_sub)
    added += _tag(code, bytes(shape), shape_length >= 0x3F)
    _, sprite_body, sprite_length = defines[button['character']]
    sprite = bytearray(struct.pack('<H', new_button) + data[sprite_body + 2:sprite_body + 4])
    for c, s, b, l in _tags(data, sprite_body + 4, sprite_body + sprite_length):
        tag = bytearray(data[s:b + l])
        if c == PLACE_OBJECT2 and data[b] & PLACE_HAS_CHARACTER and _place(data, b)['character'] == shape_id:
            struct.pack_into('<H', tag, (b - s) + 3, new_shape)
        sprite += tag
    added += _tag(SPRITE, bytes(sprite))

    def placed(index: int, name: str, character: int, matrix: bytes, depth: int) -> bytes:
        """The sprite top[index] with a new place before its first ShowFrame."""
        _, _, body, length = top[index]
        inner = list(_tags(data, body + 4, body + length))
        if depth in {struct.unpack_from('<H', data, b + 1)[0] for c, _, b, _ in inner if c == PLACE_OBJECT2}:
            raise GfxError(f'Depth {depth} of sprite {struct.unpack_from("<H", data, body)[0]} is taken.')
        show = next(s for c, s, _, _ in inner if c == SHOW_FRAME)
        return _tag(SPRITE, data[body:show] + _place_tag(depth, character, matrix, name) + data[show:body + length])
    _, _, grid_body, grid_length = top[grid]
    buttons = [_place(data, b) for c, _, b, _ in _tags(data, grid_body + 4, grid_body + grid_length) if c == PLACE_OBJECT2]
    step = sorted({p['depth'] for p in buttons})
    depth = step[-1] + (step[-1] - step[-2] if len(step) > 1 else 1)
    out = bytearray(data[:first])
    for k, (c, s, b, l) in enumerate(top):
        if k == grid:
            out += added + placed(k, f'MC_Button_{label}', new_button, _translate(*cell), depth)
        elif k == panels:
            out += placed(k, f'MC_{label}', panel['character'], panel['matrix'], panel['depth'] + 1)
        else:
            out += data[s:b + l]
    return _close(out, compressed)


def with_root_actions(movie: bytes, anchor: str, code: bytes) -> bytes | None:
    """The movie with `code` appended to the root action block that names `anchor` (before its End action,
    so no earlier offset moves); None when the block already ends with it."""
    data, compressed, first = _open(movie)
    for tag, start, body, length in _tags(data, first, len(data)):
        if tag != DO_ACTION or anchor.encode('latin-1') + b'\0' not in data[body:body + length]:
            continue
        if data[body + length - 1] != 0:
            raise GfxError('The action block does not end with End.')
        if data[body:body + length - 1].endswith(code):
            return None
        block = data[body:body + length - 1] + code + b'\x00'
        return _close(bytearray(data[:start] + _tag(DO_ACTION, block) + data[body + length:]), compressed)
    raise GfxError(f'No root action block names {anchor}.')


class Reg(int):
    """A register operand of Push."""


class As2:
    """A small AVM1 assembler: pushes, the stack actions the grid code needs, a forward conditional jump and
    anonymous DefineFunction2 (no super, arguments or this; parameters in registers 1..n)."""
    GET_VARIABLE, SET_VARIABLE, GET_MEMBER, SET_MEMBER, CALL_FUNCTION = 0x1C, 0x1D, 0x4E, 0x4F, 0x3D
    POP, NOT, STRICT_EQUALS, IF, PUSH, DEFINE_FUNCTION2 = 0x17, 0x12, 0x66, 0x9D, 0x96, 0x8E
    RETURN, STORE_REGISTER = 0x3E, 0x87
    SUPPRESS = 0x2A  # SuppressSuper | SuppressArguments | SuppressThis (the 1.69 movie's own functions)

    def __init__(self) -> None:
        self.code = bytearray()
        self.labels: dict[str, int] = {}
        self.jumps: list[tuple[int, str]] = []

    def op(self, code: int) -> 'As2':
        self.code.append(code)
        return self

    def push(self, *values) -> 'As2':
        body = b''
        for v in values:
            if isinstance(v, Reg):
                body += bytes((4, v))
            elif isinstance(v, bool):
                body += bytes((5, v))
            elif isinstance(v, int):
                body += b'\x07' + struct.pack('<i', v)
            elif isinstance(v, str):
                body += b'\x00' + v.encode('utf-8') + b'\0'
            else:
                raise GfxError(f'Cannot push {v!r}.')
        self.code += bytes((self.PUSH,)) + struct.pack('<H', len(body)) + body
        return self

    def store(self, register: int) -> 'As2':
        """Copy the top of the stack into a register (the value stays on the stack)."""
        self.code += bytes((self.STORE_REGISTER,)) + b'\x01\x00' + bytes((register,))
        return self

    def path(self, *names: str) -> 'As2':
        """Push the value of names[0].names[1]...."""
        self.push(names[0]).op(self.GET_VARIABLE)
        for name in names[1:]:
            self.push(name).op(self.GET_MEMBER)
        return self

    def call(self, function: str, *args) -> 'As2':
        """function(*args) with pushed arguments, its result dropped."""
        return self.push(*reversed(args), len(args), function).op(self.CALL_FUNCTION).op(self.POP)

    def if_not(self, label: str) -> 'As2':
        """Jump forward to `label` when the top of the stack is false."""
        self.op(self.NOT).code += bytes((self.IF,)) + b'\x02\x00\x00\x00'
        self.jumps.append((len(self.code), label))
        return self

    def label(self, name: str) -> 'As2':
        self.labels[name] = len(self.code)
        return self

    def function(self, params: tuple[str, ...], body: 'As2', registers: int = 0) -> 'As2':
        """Push an anonymous function whose parameters are registers 1..n (`registers` more after them)."""
        code = body.assemble()
        head = b'\0' + struct.pack('<HBBB', len(params), len(params) + 1 + registers, self.SUPPRESS, 0)
        head += b''.join(bytes((k,)) + p.encode('latin-1') + b'\0' for k, p in enumerate(params, 1))
        head += struct.pack('<H', len(code))
        self.code += bytes((self.DEFINE_FUNCTION2,)) + struct.pack('<H', len(head)) + head + code
        return self

    def assemble(self) -> bytes:
        out = bytearray(self.code)
        for end, label in self.jumps:
            struct.pack_into('<h', out, end - 2, self.labels[label] - end)
        return bytes(out)


def grid_class_actions(label: str, index: int, donor: str, weapon: str, skill_movies: tuple[str, ...]) -> bytes:
    """AS2 that adds class `label` (BaseCharacter `index`) to the 1.69 grid once its frame code ran: the array
    entry; the texts the movie asks the client for (LocalizedText); the wiring of the first frame loop (OnChoose,
    the class number, the donor's rollover handlers, which read the button they run on); what the second loop
    does to a panel (hidden, addCharacterInfo), then the comment, which addCharacterInfo's switch leaves empty
    for a name it does not know, and the skill preview videos; and the rollover and summary functions wrapped so
    that the class gets its name and weapon, which their switches leave as the last class shown."""
    name_key = f'GameUI_Heroes_NewCharacterDialog_Type_{label}'
    comment_key = f'GameUI_Heroes_NewCharacterDialog_Type_Comment_{label}'
    weapon_key = f'Heroes_Item_Equip_Class_Name_{weapon}'
    a = As2()
    a.path('characterTypeArray').push(index, label).op(As2.SET_MEMBER)
    for key in (name_key, comment_key, weapon_key):
        a.call('LocalizedText', key, '')
    a.push('_hc_panel').path('baseMC', 'MC_SelectCharacter', f'MC_Button_{label}').op(As2.SET_VARIABLE)
    a.path('OnChoose').path('_hc_panel').push(2, 'applyButtonEventEx').op(As2.CALL_FUNCTION).op(As2.POP)
    a.path('_hc_panel').push('charactorType', index).op(As2.SET_MEMBER)
    for event in ('onRollOut', 'onRollOver'):
        a.path('_hc_panel').push(event).path('baseMC', 'MC_SelectCharacter', f'MC_Button_{donor}', event)
        a.op(As2.SET_MEMBER)
    a.push('_hc_video').path('baseMC', 'MC_Video', f'MC_{label}').op(As2.SET_VARIABLE)
    a.path('_hc_video').push('_visible', False).op(As2.SET_MEMBER)
    a.call('addCharacterInfo', label)
    a.path('_hc_video', 'Text_Comment').push('htmlText', comment_key).op(As2.GET_VARIABLE).op(As2.SET_MEMBER)
    # the intro video's path is kept in a text field over the video; onLoad hides it class by class, by name
    # (C4 MG1: "video/createcharacter/_eneu_" showed over the Miri's video)
    a.path('_hc_video', 'Text_CharacterPreviewUsm').push('_visible', False).op(As2.SET_MEMBER)
    for n, path in enumerate(skill_movies, 1):
        a.path('_hc_video', f'Text_SkillPreviewUsm{n}').push('text', path).op(As2.SET_MEMBER)
    for function, holder, title, kind in (('showRolloverPanel', 'MC_TopName', 'Title_Text', 'Summary_Text'),
                                          ('showCharacterSummary', 'MC_CharacterTitle', 'Text_Character', 'Text_Class')):
        original = f'_hc_{function}'
        a.push(original).path(function).op(As2.SET_VARIABLE)
        body = As2()
        body.push(Reg(3), Reg(2), Reg(1), 3, original).op(As2.CALL_FUNCTION).op(As2.POP)
        body.path('characterTypeArray').push(Reg(3)).op(As2.GET_MEMBER).push(label).op(As2.STRICT_EQUALS).if_not('end')
        body.path('baseMC', holder, title).push('text', name_key).op(As2.GET_VARIABLE).op(As2.SET_MEMBER)
        body.path('baseMC', holder, kind).push('text', weapon_key).op(As2.GET_VARIABLE).op(As2.SET_MEMBER)
        body.label('end')
        a.push(function).function(('posX', 'posY', 'charType'), body).op(As2.SET_VARIABLE)
    return a.assemble()


# The customization (scaleform/CreateCharacterDialog.gfx) sets the Height and Bust sliders of a class in four
# switches over the class number 0-10 (GetBodyShapeDefaultValue_Height/_Bust, BodyShapeCommon_loadBodyShapeDefault
# and loadDefaultValues, which also keeps the class in g_selectedCharacter); any other class gets the 97-103
# height of the classes of natural height 100 and no bust. The server scales the figure by height * 100 / the
# class's natural height (slot 414: Delia and Miri 92) and the client keeps the scale within the class's
# constants (Miri 0.865-0.945), so the Miri's height stayed at the top whatever the slider said (C4 MG1). The
# wrappers call the originals with the donor class in place of the class (the donor's ranges and defaults) and
# put the class back in g_selectedCharacter.
CLASS_SLIDER_FUNCTIONS = (('GetBodyShapeDefaultValue_Height', ('character',)),
                          ('GetBodyShapeDefaultValue_Bust', ('character',)),
                          ('BodyShapeCommon_loadBodyShapeDefault', ('character', 'height', 'bust')),
                          ('loadDefaultValues', ('character', 'isBodyShapeMode')))
SELECTED_CLASS = 'g_selectedCharacter'


def class_alias_actions(index: int, donor: int, functions=CLASS_SLIDER_FUNCTIONS) -> bytes:
    """AS2 wrapping each function(class, ...) so that class `index` runs as `donor`, its result returned and
    g_selectedCharacter left at `index`."""
    a = As2()
    for function, params in functions:
        original, saved = f'_hc_{function}', len(params) + 1
        body = As2()
        body.push(Reg(1)).store(saved).op(As2.POP)
        body.push(Reg(1), index).op(As2.STRICT_EQUALS).if_not('call')
        body.push(donor).store(1).op(As2.POP)
        body.label('call')
        body.push(*(Reg(k) for k in range(len(params), 0, -1)), len(params), original).op(As2.CALL_FUNCTION)
        body.push(Reg(saved), index).op(As2.STRICT_EQUALS).if_not('end')
        body.push(SELECTED_CLASS, index).op(As2.SET_VARIABLE)
        body.label('end')
        body.op(As2.RETURN)
        a.push(original).path(function).op(As2.SET_VARIABLE)
        a.push(function).function(params, body, registers=1).op(As2.SET_VARIABLE)
    return a.assemble()
