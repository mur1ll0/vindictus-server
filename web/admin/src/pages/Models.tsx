import { useMutation, useQuery } from '@tanstack/react-query';
import { Box, Download, Search } from 'lucide-react';
import { useState } from 'react';
import { api, type components, fail } from '@/api/client';
import { ItemIcon } from '@/components/ItemIcon';
import { ModelViewer } from '@/components/ModelViewer';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { type PreviewModel, readKit, unzipStored } from '@/lib/vindictusModel';

type CatalogItem = components['schemas']['CatalogItem'];
type ItemModel = components['schemas']['ItemModel'];
type Loaded = { path: string; zip: Blob; model: PreviewModel };

async function catalog(search: string): Promise<CatalogItem[]> {
  const { data, error, response } = await api.GET('/api/items', { params: { query: { search } } });
  if (!data) fail(response, error);
  return data;
}

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
  const [search, setSearch] = useState('');
  const [path, setPath] = useState('');
  const [picked, setPicked] = useState<ItemModel | null>(null);
  const [showBones, setShowBones] = useState(false);
  const [wireframe, setWireframe] = useState(false);
  const term = search.trim();
  const found = useQuery({ queryKey: ['items', term], queryFn: () => catalog(term), enabled: term.length >= 2 });
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
            <Label htmlFor="model-search">Item</Label>
            <div className="relative">
              <Search className="pointer-events-none absolute top-2.5 left-2.5 size-4 text-muted" />
              <Input id="model-search" className="pl-8" placeholder="Nome ou classe (2+ letras)" value={search} onChange={(e) => setSearch(e.target.value)} />
            </div>
          </div>
          <ul className="mt-3 max-h-80 space-y-1 overflow-y-auto">
            {(found.data ?? []).slice(0, 50).map((item) => (
              <li key={item.class}>
                <button
                  type="button"
                  className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left hover:bg-muted/40"
                  onClick={() => pick.mutate(item.class)}
                >
                  <ItemIcon icon={item.icon} size="sm" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm">{item.name}</span>
                    <span className="block truncate font-mono text-xs text-muted">{item.class}</span>
                  </span>
                </button>
              </li>
            ))}
          </ul>
          {picked && !picked.model && (
            <p className="mt-2 text-sm text-muted">
              {picked.item_class}: sem modelo de traje resolvível ({picked.category ?? 'sem categoria'}). Armas e cabelos ainda não são resolvidos;
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
