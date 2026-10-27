"""Model kit: a studio model of the client with every file it needs, read straight from the HFS packs
through the index (scripts/client/hfs.py), in the game's folder layout:
  <kit>/models/.../<name>.mdl, .vvd, .dx90.vtx, .dx80.vtx, .sw.vtx
  <kit>/materials/<cdmaterials folder>/<texture>.vmt and the .vtf files those materials name
The Blender add-on (tools/blender/vindictus) imports a kit and exports an edited one; the admin panel
will hand kits out (docs/future/item-editor.md).
  model_kit.py --client-copy <client> --keystream <file> <models/....mdl> <out kit folder>
  model_kit.py --folder <loose files in the game layout> <models/....mdl> <out kit folder> [--zip <kit.zip>]
The zip (stored, forward slashes, kit.json) is what the panel's item editor accepts as a new model."""
from __future__ import annotations
import argparse
import json
import re
import struct
import sys
from pathlib import Path

import hfs

MODEL_SUFFIXES = ('.mdl', '.vvd', '.dx90.vtx', '.dx80.vtx', '.sw.vtx')
INDEX = 'hfs/E608DF10F284E19AB75FD7D0D8AE9CCF87831041.hfs'
TEXTURE_PARAMS = re.compile(r'"\$(basetexture|bumpmap|basetexture2|detail|envmapmask|phongexponenttexture|lightwarptexture|'
                            r'blendmodulatetexture|normalmap|iris|corneatexture|ambientoccltexture|selfillummask|tintmasktexture)"\s+"([^"]*)"',
                            re.IGNORECASE)


class KitError(ValueError):
    pass


class ClientFiles:
    """Reads plain files of the client through the HFS index."""

    def __init__(self, client: Path, keystream: Path):
        self.client = client
        self.ks = hfs.load_keystream(keystream)
        plain = hfs.decode_index((client / INDEX).read_bytes())
        self.packs = hfs.index_packs(plain)
        self.entries: dict[bytes, list[tuple[int, int, int]]] = {}
        for e in hfs.index_entries(plain):
            pack, at, size = struct.unpack_from('<HII', e, 6)
            self.entries.setdefault(e[:6], []).append((pack, at, size))

    def read(self, path: str) -> bytes | None:
        path = path.replace('\\', '/').lower().strip('/')
        folder, _, name = path.rpartition('/')
        folder_packs = {hfs.pack_name(folder, part) for part in range(64)}
        for pack, at, size in self.entries.get(hfs.name_key(name), []):
            if self.packs[pack] not in folder_packs:
                continue
            pack_file = self.client / 'hfs' / (self.packs[pack] + '.hfs')
            with pack_file.open('rb') as f:
                f.seek(at)
                data = hfs.xor_at(self.ks, at, f.read(size))
            return hfs.comp_unwrap(data) if data[:4] == hfs.COMP else data
        loose = self.client / path
        return loose.read_bytes() if loose.is_file() else None


class FolderFiles:
    """Reads loose files laid out like the game (models/..., materials/...), for example a model
    supplied outside the client."""

    def __init__(self, root: Path):
        self.root = root.resolve()

    def read(self, path: str) -> bytes | None:
        path = path.replace('\\', '/').lower().strip('/')
        if '..' in path.split('/'):
            return None
        target = (self.root / path).resolve()
        return target.read_bytes() if target.is_relative_to(self.root) and target.is_file() else None


def zip_kit(folder: Path, zip_path: Path) -> None:
    """A kit folder as a stored zip with forward slashes and lower-case game paths."""
    import zipfile
    with zipfile.ZipFile(zip_path, 'w', zipfile.ZIP_STORED) as archive:
        for path in sorted(p for p in folder.rglob('*') if p.is_file()):
            archive.write(path, path.relative_to(folder).as_posix().lower())


def model_materials(mdl: bytes) -> tuple[list[str], list[str]]:
    textures_at, num_cd, cd_index = struct.unpack_from('<ii', mdl, 204), *struct.unpack_from('<ii', mdl, 212)
    num_textures, texture_index = textures_at

    def cstring(at):
        return mdl[at:mdl.index(b'\0', at)].decode('latin-1')
    textures = [cstring(texture_index + 64 * i + struct.unpack_from('<i', mdl, texture_index + 64 * i)[0]) for i in range(num_textures)]
    folders = [cstring(struct.unpack_from('<i', mdl, cd_index + 4 * i)[0]) for i in range(num_cd)]
    return textures, folders


def make_kit(files: ClientFiles, model: str, out: Path) -> dict:
    model = model.replace('\\', '/').lower()
    if not model.startswith('models/') or not model.endswith('.mdl'):
        raise KitError('The model path must be models/....mdl.')
    base = model[:-4]
    written = []
    for suffix in MODEL_SUFFIXES:
        data = files.read(base + suffix)
        if data is None:
            if suffix in ('.mdl', '.vvd', '.dx90.vtx'):
                raise KitError(f'Missing {base + suffix}.')
            continue
        target = out / (base + suffix)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        written.append(base + suffix)
    textures, folders = model_materials((out / model).read_bytes())
    missing = []
    for texture in textures:
        for folder in folders:
            vmt_path = 'materials/' + (folder + texture).replace('\\', '/').lower() + '.vmt'
            vmt = files.read(vmt_path)
            if vmt is None:
                continue
            (out / vmt_path).parent.mkdir(parents=True, exist_ok=True)
            (out / vmt_path).write_bytes(vmt)
            written.append(vmt_path)
            for _, name in TEXTURE_PARAMS.findall(vmt.decode('latin-1')):
                if not name:
                    continue
                vtf_path = 'materials/' + name.replace('\\', '/').lower().strip('/') + '.vtf'
                vtf = files.read(vtf_path)
                if vtf is None:
                    missing.append(vtf_path)
                    continue
                (out / vtf_path).parent.mkdir(parents=True, exist_ok=True)
                (out / vtf_path).write_bytes(vtf)
                written.append(vtf_path)
            break
        else:
            missing.append(texture)
    manifest = {'format_version': 1, 'model': model, 'files': sorted(set(written)), 'missing': sorted(set(missing))}
    (out / 'kit.json').write_text(json.dumps(manifest, indent=1), encoding='utf-8')
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--client-copy', type=Path)
    parser.add_argument('--keystream', type=Path)
    parser.add_argument('--folder', type=Path, help='loose files in the game layout instead of the client packs')
    parser.add_argument('--zip', type=Path, help='also write the kit as a stored zip for the panel')
    parser.add_argument('model')
    parser.add_argument('out', type=Path)
    args = parser.parse_args()
    if not args.folder and not (args.client_copy and args.keystream):
        parser.error('--folder or --client-copy with --keystream is required')
    try:
        files = FolderFiles(args.folder) if args.folder else ClientFiles(args.client_copy, args.keystream)
        manifest = make_kit(files, args.model, args.out)
        if args.zip:
            zip_kit(args.out, args.zip)
    except (KitError, hfs.HfsError) as error:
        print(json.dumps({'error': str(error)}))
        return 2
    print(json.dumps({'files': len(manifest['files']), 'missing': manifest['missing']}))
    return 0


if __name__ == '__main__':
    sys.exit(main())
