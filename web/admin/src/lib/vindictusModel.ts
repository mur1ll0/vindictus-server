/**
 * Model kits of the client (GET /api/models/kit): a stored zip with the studio model (.mdl v44/v48),
 * vertex data (.vvd), optimized model (.dx90.vtx) and the materials' VTF textures. Same formats as
 * tools/blender/vindictus/formats; read here only for the panel's 3D preview.
 */

export type KitFiles = Map<string, Uint8Array>;

export type PreviewMesh = {
  material: string;
  positions: Float32Array;
  normals: Float32Array;
  uvs: Float32Array;
  indices: Uint32Array;
};

export type PreviewTexture = { width: number; height: number; rgba: Uint8Array };

export type PreviewBone = { name: string; parent: number; position: [number, number, number] };

export type PreviewModel = {
  name: string;
  version: number;
  meshes: PreviewMesh[];
  textures: Map<string, PreviewTexture>;
  bones: PreviewBone[];
};

const decoder = new TextDecoder('latin1');

/** Little-endian reads on a byte slice; out-of-range reads throw. */
class Reader {
  readonly bytes: Uint8Array;
  private readonly view: DataView;

  constructor(bytes: Uint8Array) {
    this.bytes = bytes;
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  }
  i32(at: number) {
    return this.view.getInt32(at, true);
  }
  u32(at: number) {
    return this.view.getUint32(at, true);
  }
  u16(at: number) {
    return this.view.getUint16(at, true);
  }
  i16(at: number) {
    return this.view.getInt16(at, true);
  }
  u8(at: number) {
    return this.view.getUint8(at);
  }
  f32(at: number) {
    return this.view.getFloat32(at, true);
  }
  string(at: number) {
    let end = at;
    while (end < this.bytes.length && this.bytes[end] !== 0) end++;
    return decoder.decode(this.bytes.subarray(at, end));
  }
}

/** Entries of a stored (uncompressed) zip, by lower-case path. */
export function unzipStored(buffer: ArrayBuffer): KitFiles {
  const r = new Reader(new Uint8Array(buffer));
  let end = r.bytes.length - 22;
  while (end >= 0 && r.u32(end) !== 0x06054b50) end--;
  if (end < 0) throw new Error('O kit não é um zip.');
  const count = r.u16(end + 10);
  let at = r.u32(end + 16);
  const files: KitFiles = new Map();
  for (let i = 0; i < count; i++) {
    if (r.u32(at) !== 0x02014b50) throw new Error('Zip corrompido.');
    if (r.u16(at + 10) !== 0) throw new Error('Só zips sem compressão são aceitos.');
    const size = r.u32(at + 20);
    const nameLength = r.u16(at + 28);
    const local = r.u32(at + 42);
    const name = decoder.decode(r.bytes.subarray(at + 46, at + 46 + nameLength)).toLowerCase();
    const dataAt = local + 30 + r.u16(local + 26) + r.u16(local + 28);
    files.set(name, r.bytes.subarray(dataAt, dataAt + size));
    at += 46 + nameLength + r.u16(at + 30) + r.u16(at + 32);
  }
  return files;
}

type MdlMesh = { material: number; count: number; offset: number };
type MdlModel = { vertexIndex: number; meshes: MdlMesh[] };

function readMdl(bytes: Uint8Array) {
  const r = new Reader(bytes);
  if (r.string(0).slice(0, 4) !== 'IDST') throw new Error('Modelo inválido.');
  const bones: PreviewBone[] = [];
  for (let i = 0, n = r.i32(156); i < n; i++) {
    const at = r.i32(160) + i * 216;
    // poseToBone (3x4, world -> bone): the bone origin in the world is -R^T t.
    const p = (row: number, col: number) => r.f32(at + 96 + 4 * (4 * row + col));
    const t = [p(0, 3), p(1, 3), p(2, 3)] as const;
    const origin = (col: number) => -(p(0, col) * t[0] + p(1, col) * t[1] + p(2, col) * t[2]);
    bones.push({ name: r.string(at + r.i32(at)), parent: r.i32(at + 4), position: [origin(0), origin(1), origin(2)] });
  }
  const textures: string[] = [];
  for (let i = 0, n = r.i32(204); i < n; i++) {
    const at = r.i32(208) + 64 * i;
    textures.push(r.string(at + r.i32(at)));
  }
  const folders: string[] = [];
  for (let i = 0, n = r.i32(212); i < n; i++) folders.push(r.string(r.i32(r.i32(216) + 4 * i)));
  const skin: number[] = [];
  for (let i = 0, n = r.i32(220); i < n; i++) skin.push(r.i16(r.i32(228) + 2 * i));
  const models: MdlModel[] = [];
  for (let b = 0, parts = r.i32(232); b < parts; b++) {
    const part = r.i32(236) + b * 16;
    for (let m = 0, n = r.i32(part + 4); m < n; m++) {
      const model = part + r.i32(part + 12) + m * 148;
      const meshes: MdlMesh[] = [];
      for (let k = 0, count = r.i32(model + 72); k < count; k++) {
        const mesh = model + r.i32(model + 76) + k * 116;
        meshes.push({ material: r.i32(mesh), count: r.i32(mesh + 8), offset: r.i32(mesh + 12) });
      }
      models.push({ vertexIndex: r.i32(model + 84), meshes });
    }
  }
  return { version: r.i32(4), name: r.string(12), bones, textures, folders, skin, models };
}

type Vertex = { pos: [number, number, number]; normal: [number, number, number]; uv: [number, number] };

function readVvd(bytes: Uint8Array): Vertex[] {
  const r = new Reader(bytes);
  if (r.string(0).slice(0, 4) !== 'IDSV') throw new Error('Arquivo de vértices inválido.');
  const order: number[] = [];
  const fixups = r.i32(48);
  if (fixups) {
    for (let k = 0; k < fixups; k++) {
      const at = r.i32(52) + 12 * k;
      if (r.i32(at) < 0) continue;
      for (let i = 0, source = r.i32(at + 4), count = r.i32(at + 8); i < count; i++) order.push(source + i);
    }
  } else {
    for (let i = 0, total = r.i32(16); i < total; i++) order.push(i);
  }
  const start = r.i32(56);
  return order.map((i) => {
    const at = start + 48 * i;
    return {
      pos: [r.f32(at + 16), r.f32(at + 20), r.f32(at + 24)],
      normal: [r.f32(at + 28), r.f32(at + 32), r.f32(at + 36)],
      uv: [r.f32(at + 40), r.f32(at + 44)],
    };
  });
}

/** LOD 0 triangles of mesh-local vertex ids, per model and mesh. */
function readVtx(bytes: Uint8Array): number[][][] {
  const r = new Reader(bytes);
  const models: number[][][] = [];
  for (let b = 0, parts = r.i32(28); b < parts; b++) {
    const part = r.i32(32) + b * 8;
    for (let m = 0, n = r.i32(part); m < n; m++) {
      const model = part + r.i32(part + 4) + m * 8;
      const lod = model + r.i32(model + 4);
      const meshes: number[][] = [];
      for (let k = 0, count = r.i32(lod); k < count; k++) {
        const mesh = lod + r.i32(lod + 4) + k * 9;
        const triangles: number[] = [];
        for (let g = 0, groups = r.i32(mesh); g < groups; g++) {
          const sg = mesh + r.i32(mesh + 4) + g * 25;
          const original = (i: number) => r.u16(sg + r.i32(sg + 4) + 9 * i + 4);
          for (let s = 0, strips = r.i32(sg + 16); s < strips; s++) {
            const strip = sg + r.i32(sg + 20) + s * 27;
            const first = r.i32(strip + 4);
            for (let i = first, last = first + r.i32(strip); i < last; i++) triangles.push(original(r.u16(sg + r.i32(sg + 12) + 2 * i)));
          }
        }
        meshes.push(triangles);
      }
      models.push(meshes);
    }
  }
  return models;
}

const BLOCK_BYTES: Record<number, number> = { 13: 8, 14: 16, 15: 16 };
const PIXEL_BYTES: Record<number, number> = { 0: 4, 2: 3, 3: 3, 12: 4, 16: 4 };

function levelSize(format: number, w: number, h: number) {
  const block = BLOCK_BYTES[format];
  if (block) return Math.max(1, (w + 3) >> 2) * Math.max(1, (h + 3) >> 2) * block;
  return w * h * (PIXEL_BYTES[format] ?? 0);
}

function rgb565(c: number): [number, number, number] {
  return [(((c >> 11) & 31) * 255) / 31, (((c >> 5) & 63) * 255) / 63, ((c & 31) * 255) / 31];
}

function mix(a: [number, number, number], b: [number, number, number], wa: number, wb: number): [number, number, number] {
  return [(a[0] * wa + b[0] * wb) / (wa + wb), (a[1] * wa + b[1] * wb) / (wa + wb), (a[2] * wa + b[2] * wb) / (wa + wb)];
}

function dxt5Alpha(r: Reader, at: number): number[] {
  const a0 = r.u8(at);
  const a1 = r.u8(at + 1);
  const table = [a0, a1];
  for (let i = 1; i < 7; i++) table.push(a0 > a1 ? ((7 - i) * a0 + i * a1) / 7 : i < 5 ? ((5 - i) * a0 + i * a1) / 5 : i === 5 ? 0 : 255);
  let low = 0;
  let high = 0;
  for (let i = 0; i < 3; i++) low |= r.u8(at + 2 + i) << (8 * i);
  for (let i = 0; i < 3; i++) high |= r.u8(at + 5 + i) << (8 * i);
  const out: number[] = [];
  for (let i = 0; i < 16; i++) {
    const code = i < 8 ? (low >> (3 * i)) & 7 : (high >> (3 * (i - 8))) & 7;
    out.push(table[code] ?? 255);
  }
  return out;
}

function decodeVtf(bytes: Uint8Array): PreviewTexture | null {
  const r = new Reader(bytes);
  if (r.string(0).slice(0, 3) !== 'VTF') return null;
  const width = r.u16(16);
  const height = r.u16(18);
  const frames = r.u16(24);
  const format = r.i32(52);
  const mips = r.u8(56);
  const lowFormat = r.i32(57);
  const lowW = r.u8(61);
  const lowH = r.u8(62);
  if (!BLOCK_BYTES[format] && !PIXEL_BYTES[format]) return null;
  let at = r.u32(12) + (lowFormat >= 0 && lowW && lowH ? levelSize(lowFormat, lowW, lowH) : 0);
  for (let level = mips - 1; level > 0; level--) at += levelSize(format, Math.max(1, width >> level), Math.max(1, height >> level)) * frames;
  const rgba = new Uint8Array(width * height * 4);
  const block = BLOCK_BYTES[format];
  if (block) {
    const bw = Math.max(1, (width + 3) >> 2);
    const bh = Math.max(1, (height + 3) >> 2);
    for (let by = 0; by < bh; by++) {
      for (let bx = 0; bx < bw; bx++) {
        const b = at + (by * bw + bx) * block;
        const color = format === 13 ? b : b + 8;
        const c0 = r.u16(color);
        const c1 = r.u16(color + 2);
        const p0 = rgb565(c0);
        const p1 = rgb565(c1);
        const four = c0 > c1 || format !== 13;
        const palette = [p0, p1, four ? mix(p0, p1, 2, 1) : mix(p0, p1, 1, 1), four ? mix(p0, p1, 1, 2) : ([0, 0, 0] as [number, number, number])];
        const codes = r.u32(color + 4);
        const alphas =
          format === 15 ? dxt5Alpha(r, b) : format === 14 ? Array.from({ length: 16 }, (_, i) => ((r.u8(b + (i >> 1)) >> ((i & 1) * 4)) & 15) * 17) : null;
        for (let i = 0; i < 16; i++) {
          const x = bx * 4 + (i & 3);
          const y = by * 4 + (i >> 2);
          if (x >= width || y >= height) continue;
          const code = (codes >>> (2 * i)) & 3;
          const p = palette[code] ?? p0;
          const o = (y * width + x) * 4;
          rgba[o] = p[0];
          rgba[o + 1] = p[1];
          rgba[o + 2] = p[2];
          rgba[o + 3] = alphas ? (alphas[i] ?? 255) : !four && code === 3 ? 0 : 255;
        }
      }
    }
  } else {
    const n = PIXEL_BYTES[format] ?? 4;
    for (let i = 0; i < width * height; i++) {
      const s = at + i * n;
      const bgr = format === 12 || format === 16 || format === 3;
      rgba[i * 4] = r.u8(bgr ? s + 2 : s);
      rgba[i * 4 + 1] = r.u8(s + 1);
      rgba[i * 4 + 2] = r.u8(bgr ? s : s + 2);
      rgba[i * 4 + 3] = format === 12 || format === 0 ? r.u8(s + 3) : 255;
    }
  }
  return { width, height, rgba };
}

function basetexture(vmt: string): string | null {
  const match = /"\$basetexture"\s+"([^"]+)"/i.exec(vmt);
  return match?.[1] ?? null;
}

/** The kit's model as preview meshes (LOD 0) with each material's base texture. */
export function readKit(files: KitFiles): PreviewModel {
  const manifest = JSON.parse(decoder.decode(files.get('kit.json') ?? new TextEncoder().encode('{}'))) as { model?: string };
  const modelPath = (manifest.model ?? [...files.keys()].find((k) => k.endsWith('.mdl')) ?? '').toLowerCase();
  const base = modelPath.slice(0, -4);
  const need = (path: string) => {
    const data = files.get(path);
    if (!data) throw new Error(`O kit não tem ${path}.`);
    return data;
  };
  const mdl = readMdl(need(modelPath));
  const vertices = readVvd(need(`${base}.vvd`));
  const triangles = readVtx(need(`${base}.dx90.vtx`));
  const textures = new Map<string, PreviewTexture>();
  for (const texture of mdl.textures) {
    const vmt = mdl.folders.map((f) => `materials/${(f + texture).replace(/\\/g, '/').toLowerCase()}.vmt`).find((p) => files.has(p));
    const vmtBytes = vmt ? files.get(vmt) : undefined;
    const name = vmtBytes ? basetexture(decoder.decode(vmtBytes)) : null;
    const vtf = name ? files.get(`materials/${name.replace(/\\/g, '/').toLowerCase()}.vtf`) : undefined;
    const decoded = vtf ? decodeVtf(vtf) : null;
    if (decoded) textures.set(texture, decoded);
  }
  const meshes: PreviewMesh[] = [];
  mdl.models.forEach((model, m) => {
    const first = model.vertexIndex / 48;
    model.meshes.forEach((mesh, k) => {
      const slice = vertices.slice(first + mesh.offset, first + mesh.offset + mesh.count);
      meshes.push({
        material: mdl.textures[mdl.skin[mesh.material] ?? mesh.material] ?? '',
        positions: Float32Array.from(slice.flatMap((x) => x.pos)),
        normals: Float32Array.from(slice.flatMap((x) => x.normal)),
        uvs: Float32Array.from(slice.flatMap((x) => x.uv)),
        indices: Uint32Array.from(triangles[m]?.[k] ?? []),
      });
    });
  });
  return { name: mdl.name, version: mdl.version, meshes, textures, bones: mdl.bones };
}
