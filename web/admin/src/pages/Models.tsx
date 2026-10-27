import { useMutation } from '@tanstack/react-query';
import { Box, Download, LayoutGrid } from 'lucide-react';
import { useState } from 'react';
import { api, type components, fail } from '@/api/client';
import { ItemPickerModal } from '@/components/ItemPicker';
import { ModelViewer } from '@/components/ModelViewer';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { type PreviewModel, readKit, unzipStored } from '@/lib/vindictusModel';

type ItemModel = components['schemas']['ItemModel'];
type Loaded = { path: string; zip: Blob; model: PreviewModel };

async function itemModel(itemClass: string): Promise<ItemModel> {
  const { data, error, response } = await api.GET('/api/items/{item_class}/model', {
    params: { path: { item_class: itemClass } },
  });
  if (!data) fail(response, error);
  return data;
}

async function loadKit(path: string): Promise<Loaded> {
  const response = await fetch(`/api/models/kit?model=${encodeURIComponent(path)}`, { credentials: 'same-origin' });
  if (!response.ok) {
    throw new Error(response.status === 404 ? 'Modelo não encontrado nos pacotes do cliente.' : `Falha ao baixar o kit (${response.status}).`);
  }
  const zip = await response.blob();
  return { path, zip, model: readKit(unzipStored(await zip.arrayBuffer())) };
}

function save(loaded: Loaded) {
  const url = URL.createObjectURL(loaded.zip);
  const link = document.createElement('a');
  link.href = url;
  link.download = `${loaded.path.split('/').pop()?.replace(/\.mdl$/, '') ?? 'modelo'}.zip`;
  link.click();
  URL.revokeObjectURL(url);
}

export function ModelsPage() {
  const [picking, setPicking] = useState(false);
  const [path, setPath] = useState('');
  const [picked, setPicked] = useState<ItemModel | null>(null);
  const [showBones, setShowBones] = useState(false);
  const [wireframe, setWireframe] = useState(false);
  const kit = useMutation({ mutationFn: loadKit });
  const pick = useMutation({
    mutationFn: itemModel,
    onSuccess: (info) => {
      setPicked(info);
      if (info.model) {
        setPath(info.model);
        kit.mutate(info.model);
      }
    },
  });
  const loaded = kit.data;
  const meshCount = loaded?.model.meshes.length ?? 0;
  const triangles = loaded?.model.meshes.reduce((n, m) => n + m.indices.length / 3, 0) ?? 0;

  return (
    <section className="grid gap-4 lg:grid-cols-[22rem_1fr]">
      <div className="space-y-4">
        <Card>
          <div className="space-y-1.5">
            <Label>Item</Label>
            <Button variant="outline" className="w-full" onClick={() => setPicking(true)}>
              <LayoutGrid /> Escolher no catálogo
            </Button>
            {picked && <p className="truncate font-mono text-xs text-muted">{picked.item_class}</p>}
          </div>
          {picking && <ItemPickerModal title="Item do modelo" onPick={(item) => pick.mutate(item.class)} onClose={() => setPicking(false)} />}
          {picked && !picked.model && (
            <p className="mt-2 text-sm text-muted">
              {picked.item_class}: sem modelo resolvível ({picked.category ?? 'sem categoria'}). Roupas, armaduras, armas e escudos são resolvidos; cabelos e acessórios não;
              use o caminho do modelo abaixo.
            </p>
          )}
          {pick.error && <p className="mt-2 text-sm text-danger">{String(pick.error.message)}</p>}
        </Card>
        <Card>
          <form
            className="space-y-2"
            onSubmit={(e) => {
              e.preventDefault();
              setPicked(null);
              kit.mutate(path.trim());
            }}
          >
            <Label htmlFor="model-path">Caminho do modelo</Label>
            <Input id="model-path" className="font-mono text-xs" placeholder="models/player/fiona/fiona_inner_fringe.mdl" value={path} onChange={(e) => setPath(e.target.value)} />
            <Button type="submit" size="sm" disabled={!path.trim() || kit.isPending}>
              <Box /> Ver modelo
            </Button>
          </form>
        </Card>
        {loaded && (
          <Card className="space-y-2 text-sm">
            <p className="font-mono text-xs break-all">{loaded.path}</p>
            <div className="flex flex-wrap gap-1.5">
              <Badge>MDL v{loaded.model.version}</Badge>
              <Badge>{meshCount} malhas</Badge>
              <Badge>{triangles.toLocaleString('pt-BR')} triângulos</Badge>
              <Badge>{loaded.model.bones.length} ossos</Badge>
              <Badge>{loaded.model.textures.size} texturas</Badge>
            </div>
            <p className="text-muted">
              O kit leva modelo, materiais e texturas nas pastas do jogo. Importe no Blender pelo add-on (tools/blender) em File → Import →
              Vindictus Kit.
            </p>
            <Button size="sm" onClick={() => save(loaded)}>
              <Download /> Exportar kit (.zip)
            </Button>
          </Card>
        )}
      </div>
      <Card className="space-y-3">
        <div className="flex flex-wrap items-center gap-4 text-sm">
          <label className="flex items-center gap-2">
            <input type="checkbox" checked={showBones} onChange={(e) => setShowBones(e.target.checked)} /> Ossos
          </label>
          <label className="flex items-center gap-2">
            <input type="checkbox" checked={wireframe} onChange={(e) => setWireframe(e.target.checked)} /> Wireframe
          </label>
          <span className="text-muted">Arraste para girar, roda do mouse para aproximar, botão direito para mover.</span>
        </div>
        {kit.isPending && <p className="text-sm text-muted">Carregando o kit…</p>}
        {kit.error && <p className="text-sm text-danger">{String(kit.error.message)}</p>}
        {loaded ? (
          <ModelViewer model={loaded.model} showBones={showBones} wireframe={wireframe} />
        ) : (
          !kit.isPending && <p className="py-24 text-center text-sm text-muted">Escolha um item ou digite o caminho de um modelo.</p>
        )}
      </Card>
    </section>
  );
}
