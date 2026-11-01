"""Add a class to the 1.69 character list portrait (scaleform/SelectCharacterDialog_V2.gfx).

The movie keeps one portrait per class in a sprite whose frames are labelled with the class name (Lethita
.. Delia); the client sends the class name (patch_class_routing CLASS_STRINGS, "{Name}") and a name with no
frame stays on the first one, Lethita's: the Miri's list portrait was a man's face (C4 MI4-4). Each frame
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
