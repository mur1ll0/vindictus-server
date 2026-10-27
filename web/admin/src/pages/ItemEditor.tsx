import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Copy, Eye, EyeOff, PackagePlus, Pencil, RotateCcw, Search, Trash2, Upload } from 'lucide-react';
import { useEffect, useState } from 'react';
import { api, fail } from '@/api/client';
import { KIND_LABEL } from '@/api/content';
import {
  CLASSES,
  type Draft,
  EQUIP_FIELDS,
  type EditorDetail,
  editorItem,
  editorItems,
  type Fields,
  hideItem,
  ITEM_FIELDS,
  itemStatus,
  removeItem,
  restoreItem,
  saveItem,
  type StoredKit,
  uploadKit,
} from '@/api/items';
import { ActionForm, errorText } from '@/components/forms';
import { ItemIcon } from '@/components/ItemIcon';
import { Modal } from '@/components/Modal';
import { ModelViewer } from '@/components/ModelViewer';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { type PreviewModel, readKit, unzipStored } from '@/lib/vindictusModel';

type Mode = { kind: 'new' | 'clone' | 'edit'; source?: EditorDetail };
type Pending = { kind: 'hide' | 'show' | 'remove' | 'restore'; item: EditorDetail };

const FILTERS: [string, string][] = [
  ['', 'Todos'],
  ['added', 'Adicionados'],
  ['edited', 'Base editados'],
  ['hidden', 'Ocultos'],
  ['pending', 'Pendentes'],
];
type Tone = { label: string; tone: 'accent' | 'ok' | 'neutral' };
const BASE: Tone = { label: 'base', tone: 'neutral' };
const ORIGIN: Record<string, Tone> = {
  base: BASE,
  added: { label: 'adição', tone: 'ok' },
  edited: { label: 'base editado', tone: 'accent' },
};
const BODIES = ['female', 'male', 'giant', 'tall'];

async function kitFromUrl(url: string): Promise<PreviewModel> {
  const response = await fetch(url, { credentials: 'same-origin' });
  if (!response.ok) throw new Error(response.status === 404 ? 'Modelo não encontrado.' : `Falha ao baixar o kit (${response.status}).`);
  return readKit(unzipStored(await response.arrayBuffer()));
}

/** The model of an item for the preview: its stored kit, or its costume model in the client copy. */
async function previewOf(itemClass: string, assets?: string | null): Promise<PreviewModel | null> {
  if (assets) return kitFromUrl(`/api/item-editor/kits/${encodeURIComponent(assets)}`);
  const { data, error, response } = await api.GET('/api/items/{item_class}/model', { params: { path: { item_class: itemClass } } });
  if (!data) fail(response, error);
  return data.model ? kitFromUrl(`/api/models/kit?model=${encodeURIComponent(data.model)}`) : null;
}

function definitionOf(detail?: EditorDetail): Record<string, unknown> | null {
  return (detail?.definition as Record<string, unknown> | null | undefined) ?? null;
}

/** The form's columns: the exported values, over them the newest definition of an added item. */
function fieldsOf(detail: EditorDetail): Fields {
  const fields = { ...(detail.fields as unknown as Fields) };
  const set = (definitionOf(detail)?.set ?? {}) as Record<string, Fields>;
  for (const table of Object.values(set)) Object.assign(fields, table);
  return fields;
}

function Preview({ model, busy, error }: { model?: PreviewModel | null; busy: boolean; error: unknown }) {
  const [wireframe, setWireframe] = useState(false);
  if (busy) return <p className="py-16 text-center text-sm text-muted">Carregando o modelo…</p>;
  if (error) return <p className="py-16 text-center text-sm text-danger">{errorText(error)}</p>;
  if (!model) return <p className="py-16 text-center text-sm text-muted">Sem modelo de traje para mostrar.</p>;
  return (
    <div className="space-y-2">
      <label className="flex items-center gap-2 text-sm">
        <input type="checkbox" checked={wireframe} onChange={(e) => setWireframe(e.target.checked)} /> Wireframe
      </label>
      <ModelViewer model={model} showBones={false} wireframe={wireframe} />
    </div>
  );
}

function ItemForm({ mode, onDone, onCancel }: { mode: Mode; onDone: (itemClass: string) => void; onCancel: () => void }) {
  const source = mode.source;
  const definition = definitionOf(source);
  const editing = mode.kind === 'edit';
  const base = editing && source?.origin !== 'added';
  const [itemClass, setItemClass] = useState(editing ? (source?.item_class ?? '') : '');
  // A clone of an added item copies that item's own template.
  const [template, setTemplate] = useState(mode.kind === 'clone' ? String(definition?.template ?? source?.item_class ?? '') : '');
  const [name, setName] = useState(source?.name ?? String((definition?.text as Record<string, string> | undefined)?.name ?? ''));
  const [description, setDescription] = useState(source?.description ?? '');
  const [fields, setFields] = useState<Fields>(() => (source ? fieldsOf(source) : {}));
  const [setId, setSetId] = useState(source?.set_id ?? '');
  const [kitFile, setKitFile] = useState<File | null>(null);
  const [body, setBody] = useState('');
  const [templateDetail, setTemplateDetail] = useState<EditorDetail | null>(null);
  const loadTemplate = useMutation({
    mutationFn: editorItem,
    onSuccess: (detail) => {
      setTemplateDetail(detail);
      setFields(fieldsOf(detail));
      setSetId(detail.set_id ?? '');
      if (!name) setName(detail.name ?? '');
    },
  });
  const kitPreview = useMutation({ mutationFn: async (file: File) => readKit(unzipStored(await file.arrayBuffer())) });
  const sourcePreview = useQuery({
    queryKey: ['item-preview', source?.item_class, definition?.assets],
    queryFn: () => previewOf(source?.item_class ?? '', definition?.assets as string | undefined),
    enabled: !!source && !kitFile,
  });
  const restriction = Number(fields.ClassRestriction ?? 0);
  const value = (column: string) => fields[column] ?? '';
  const change = (column: string, kind: string, raw: string) =>
    setFields((f) => {
      const next = { ...f };
      if (raw === '') delete next[column];
      else next[column] = kind === 'text' ? raw : Number(raw);
      return next;
    });

  const submit = async (operationId: string) => {
    let kit: StoredKit | undefined;
    if (kitFile) kit = await uploadKit(kitFile);
    const clean: Fields = {};
    for (const [column, v] of Object.entries(fields)) if (v !== '' && !Number.isNaN(v)) clean[column] = v;
    const draft: Draft = {
      item_class: itemClass.trim(),
      template: editing ? null : template.trim() || null,
      name: name.trim(),
      description: description.trim() || null,
      fields: clean,
      set_id: base ? (source?.set_id ?? null) : setId.trim() || null,
      kit: kit?.assets ?? null,
      body: kit && body ? body : null,
    };
    await saveItem(operationId, draft);
  };

  const numberInput = ([column, label, kind]: [string, string, string]) => (
    <div key={column} className="space-y-1">
      <Label htmlFor={`f-${column}`}>{label}</Label>
      <Input
        id={`f-${column}`}
        type={kind === 'text' ? 'text' : 'number'}
        step={kind === 'float' ? 'any' : 1}
        value={value(column)}
        disabled={column === 'CostumeType' && !!kitFile}
        onChange={(e) => change(column, kind, e.target.value)}
      />
    </div>
  );

  return (
    <ActionForm submitLabel={editing ? 'Salvar edição' : 'Criar item'} onSubmit={submit} onCancel={onCancel} onDone={() => onDone(itemClass.trim())}>
      <div className="grid gap-4 lg:grid-cols-[1fr_24rem]">
        <div className="space-y-4">
          <Card className="space-y-3">
            <h3 className="text-sm font-semibold">Identidade</h3>
            <div className="grid gap-3 sm:grid-cols-2">
              <div className="space-y-1">
                <Label htmlFor="item-class">ItemClass</Label>
                <Input
                  id="item-class"
                  className="font-mono"
                  value={itemClass}
                  disabled={editing}
                  placeholder="fiona_inner_novo"
                  onChange={(e) => setItemClass(e.target.value.toLowerCase())}
                />
              </div>
              {!editing && (
                <div className="space-y-1">
                  <Label htmlFor="template">Item-modelo (base)</Label>
                  <div className="flex gap-2">
                    <Input id="template" className="font-mono" value={template} onChange={(e) => setTemplate(e.target.value.toLowerCase())} />
                    <Button type="button" variant="outline" size="sm" disabled={!template || loadTemplate.isPending} onClick={() => loadTemplate.mutate(template)}>
                      Carregar
                    </Button>
                  </div>
                  {loadTemplate.error && <p className="text-xs text-danger">{errorText(loadTemplate.error)}</p>}
                  {templateDetail && templateDetail.origin === 'added' && (
                    <p className="text-xs text-danger">O modelo precisa ser um item base; para copiar uma adição use Clonar nela.</p>
                  )}
                </div>
              )}
              <div className="space-y-1 sm:col-span-2">
                <Label htmlFor="item-name">Nome (inglês)</Label>
                <Input id="item-name" value={name} maxLength={128} onChange={(e) => setName(e.target.value)} />
              </div>
              <div className="space-y-1 sm:col-span-2">
                <Label htmlFor="item-desc">Descrição (inglês)</Label>
                <textarea
                  id="item-desc"
                  className="min-h-16 w-full rounded-md border border-border bg-surface px-3 py-2 text-sm"
                  value={description}
                  maxLength={512}
                  onChange={(e) => setDescription(e.target.value.replace(/[\r\n]/g, ' '))}
                />
              </div>
            </div>
            <div className="flex items-center gap-3">
              <ItemIcon icon={typeof fields.Icon === 'string' ? fields.Icon.toLowerCase() : null} />
              <span className="text-xs text-muted">Ícone: nome de um ícone já usado por algum item (scaleform/item_icon).</span>
            </div>
          </Card>
          <Card className="space-y-3">
            <h3 className="text-sm font-semibold">Classes que usam</h3>
            <div className="flex flex-wrap gap-3 text-sm">
              {CLASSES.map(([label, bit]) => (
                <label key={bit} className="flex items-center gap-1.5">
                  <input
                    type="checkbox"
                    checked={(restriction & bit) !== 0}
                    onChange={(e) => change('ClassRestriction', 'int', String(e.target.checked ? restriction | bit : restriction & ~bit))}
                  />
                  {label}
                </label>
              ))}
            </div>
            <p className="text-xs text-muted">ClassRestriction = {restriction}. O cliente 1.69 conhece só estas 10 classes.</p>
          </Card>
          <Card className="space-y-3">
            <h3 className="text-sm font-semibold">Item (ItemClassInfo)</h3>
            <div className="grid gap-3 sm:grid-cols-3">{ITEM_FIELDS.map(numberInput)}</div>
          </Card>
          <Card className="space-y-3">
            <h3 className="text-sm font-semibold">Equipamento (EquipItemInfo)</h3>
            <div className="grid gap-3 sm:grid-cols-4">{EQUIP_FIELDS.map(numberInput)}</div>
            <div className="space-y-1">
              <Label htmlFor="set-id">Conjunto (SetID)</Label>
              <Input id="set-id" className="font-mono" value={setId} disabled={base} onChange={(e) => setSetId(e.target.value)} />
              {base && <p className="text-xs text-muted">O conjunto de um item base não muda pelo editor.</p>}
            </div>
          </Card>
        </div>
        <div className="space-y-4">
          <Card className="space-y-3">
            <h3 className="text-sm font-semibold">Modelo 3D</h3>
            {base ? (
              <p className="text-sm text-muted">Itens base mantêm o modelo; escolha outro CostumeType existente, se quiser.</p>
            ) : (
              <>
                <div className="space-y-1">
                  <Label htmlFor="kit">Kit novo (.zip do add-on, opcional)</Label>
                  <Input
                    id="kit"
                    type="file"
                    accept=".zip,application/zip"
                    onChange={(e) => {
                      const file = e.target.files?.[0] ?? null;
                      setKitFile(file);
                      if (file) kitPreview.mutate(file);
                    }}
                  />
                </div>
                {kitFile && (
                  <div className="space-y-1">
                    <Label htmlFor="body">Corpo da entrada no player_costume.txt</Label>
                    <select id="body" className="h-9 w-full rounded-md border border-border bg-surface px-2 text-sm" value={body} onChange={(e) => setBody(e.target.value)}>
                      <option value="">o do item-modelo</option>
                      {BODIES.map((b) => (
                        <option key={b} value={b}>
                          {b}
                        </option>
                      ))}
                    </select>
                    <p className="text-xs text-muted">
                      O servidor guarda o kit, dá um CostumeType livre (a partir de 9100) e cria a entrada do player_costume.txt.
                    </p>
                  </div>
                )}
              </>
            )}
            {kitFile ? (
              <Preview model={kitPreview.data} busy={kitPreview.isPending} error={kitPreview.error} />
            ) : (
              <Preview model={sourcePreview.data} busy={sourcePreview.isFetching} error={sourcePreview.error} />
            )}
          </Card>
        </div>
      </div>
    </ActionForm>
  );
}

function Detail({ item, onAction, onForm }: { item: EditorDetail; onAction: (p: Pending) => void; onForm: (m: Mode) => void }) {
  const fields = fieldsOf(item);
  const definition = definitionOf(item);
  const origin = ORIGIN[item.origin] ?? BASE;
  const preview = useQuery({
    queryKey: ['item-preview', item.item_class, definition?.assets],
    queryFn: () => previewOf(item.item_class, definition?.assets as string | undefined),
  });
  return (
    <div className="grid gap-4 xl:grid-cols-[1fr_24rem]">
      <Card className="space-y-4">
        <div className="flex items-start gap-3">
          <ItemIcon icon={item.icon} />
          <div className="min-w-0 flex-1">
            <p className="font-semibold">{item.name ?? String((definition?.text as Record<string, string> | undefined)?.name ?? item.item_class)}</p>
            <p className="font-mono text-xs text-muted">{item.item_class}</p>
            <div className="mt-1 flex flex-wrap gap-1.5">
              <Badge tone={origin.tone}>{origin.label}</Badge>
              {item.hidden && <Badge tone="danger">oculto</Badge>}
              {!item.exported && <Badge tone="accent">ainda não exportado</Badge>}
              {definition?.template ? <Badge>modelo: {String(definition.template)}</Badge> : null}
            </div>
          </div>
        </div>
        {item.description && <p className="text-sm text-muted">{item.description}</p>}
        <div className="flex flex-wrap gap-2">
          <Button size="sm" onClick={() => onForm({ kind: 'edit', source: item })}>
            <Pencil /> Editar
          </Button>
          <Button size="sm" variant="outline" onClick={() => onForm({ kind: 'clone', source: item })}>
            <Copy /> Clonar
          </Button>
          <Button size="sm" variant="outline" onClick={() => onAction({ kind: item.hidden ? 'show' : 'hide', item })}>
            {item.hidden ? <Eye /> : <EyeOff />} {item.hidden ? 'Mostrar' : 'Ocultar'}
          </Button>
          {item.origin === 'added' && (
            <Button size="sm" variant="danger" onClick={() => onAction({ kind: 'remove', item })}>
              <Trash2 /> Remover
            </Button>
          )}
          {item.origin === 'edited' && (
            <Button size="sm" variant="outline" onClick={() => onAction({ kind: 'restore', item })}>
              <RotateCcw /> Restaurar original
            </Button>
          )}
        </div>
        <dl className="grid grid-cols-2 gap-x-4 gap-y-1 text-sm sm:grid-cols-3">
          {[...ITEM_FIELDS, ...EQUIP_FIELDS]
            .filter(([column]) => fields[column] !== undefined)
            .map(([column, label]) => (
              <div key={column} className="flex justify-between gap-2 border-b border-border/50 py-0.5">
                <dt className="text-muted">{label}</dt>
                <dd className="font-mono">{String(fields[column])}</dd>
              </div>
            ))}
          {item.set_id && (
            <div className="flex justify-between gap-2 border-b border-border/50 py-0.5">
              <dt className="text-muted">Conjunto</dt>
              <dd className="font-mono">{item.set_id}</dd>
            </div>
          )}
        </dl>
        <div className="space-y-1 text-sm">
          <h3 className="font-semibold">Quem tem o item</h3>
          <p className="text-muted">
            {item.usage.total} no jogo, {item.usage.queued} em entregas pendentes.{' '}
            {item.origin === 'added'
              ? 'Remover é recusado enquanto houver algum; ocultar mantém os itens dos personagens funcionando.'
              : 'Ocultar mantém os itens dos personagens funcionando; itens base nunca são apagados.'}
          </p>
          <ul className="space-y-0.5">
            {item.usage.characters.map((c) => (
              <li key={c.cid}>
                <Link to="/personagens/$cid" params={{ cid: c.cid }} className="underline">
                  {c.name || c.cid}
                </Link>{' '}
                × {c.count}
              </li>
            ))}
          </ul>
        </div>
        {item.edits.length > 0 && (
          <div className="space-y-1 text-sm">
            <h3 className="font-semibold">Edições deste item</h3>
            <ul className="space-y-0.5">
              {item.edits.map((e) => (
                <li key={e.id} className="flex flex-wrap gap-2">
                  <Badge tone={e.state === 'pending' ? 'accent' : 'ok'}>{e.state === 'pending' ? 'pendente' : `versão ${e.version}`}</Badge>
                  <span>{KIND_LABEL[e.kind] ?? e.kind}</span>
                  <span className="text-muted">por {e.created_by ?? '—'}</span>
                </li>
              ))}
            </ul>
            <p className="text-muted">
              Edições pendentes entram no jogo quando um admin publica em <Link to="/conteudo" className="underline">Conteúdo</Link>; depois, o
              start-lab.bat reexporta os itens do servidor e atualiza a cópia do cliente.
            </p>
          </div>
        )}
      </Card>
      <Card>
        <Preview model={preview.data} busy={preview.isFetching} error={preview.error} />
      </Card>
    </div>
  );
}

const ACTION_TEXT: Record<Pending['kind'], { title: string; submit: string; text: string }> = {
  hide: {
    title: 'Ocultar item',
    submit: 'Ocultar',
    text: 'O item sai das lojas de NPC e da lista de dar item do painel. Os personagens que já têm o item continuam com ele, funcionando.',
  },
  show: { title: 'Mostrar item', submit: 'Mostrar', text: 'O item volta às lojas e à lista de dar item.' },
  remove: {
    title: 'Remover item adicionado',
    submit: 'Remover',
    text: 'O item sai do conteúdo do servidor e da cópia do cliente na próxima publicação. É recusado enquanto algum personagem tiver o item.',
  },
  restore: { title: 'Restaurar item base', submit: 'Restaurar', text: 'Os valores e o texto originais do item voltam na próxima publicação.' },
};

export function ItemEditorPage() {
  const queryClient = useQueryClient();
  const [search, setSearch] = useState('');
  const [filter, setFilter] = useState('');
  const [picked, setPicked] = useState<string | null>(null);
  const [mode, setMode] = useState<Mode | null>(null);
  const [pending, setPending] = useState<Pending | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const term = search.trim();
  const status = useQuery({ queryKey: ['item-editor-status'], queryFn: itemStatus });
  const list = useQuery({
    queryKey: ['item-editor', term, filter],
    queryFn: () => editorItems(term, filter),
    enabled: term.length >= 2 || filter !== '',
  });
  const detail = useQuery({ queryKey: ['item-editor-item', picked], queryFn: () => editorItem(picked ?? ''), enabled: !!picked });
  useEffect(() => setMode(null), [picked]);
  const refresh = () => queryClient.invalidateQueries({ predicate: (q) => String(q.queryKey[0]).startsWith('item-editor') });

  return (
    <section className="space-y-4">
      {status.data && !status.data.up_to_date && (
        <Card className="text-sm">
          Itens publicados até a versão {status.data.published_version}; o servidor exportou até {status.data.exported_version ?? 'nenhuma'}. Rode o
          start-lab.bat (ou scripts/content/refresh_item_content.py) no host para reexportar e reiniciar o jogo.
        </Card>
      )}
      {saved && (
        <Card className="text-sm">
          Edição de <span className="font-mono">{saved}</span> salva como pendente. Publique em{' '}
          <Link to="/conteudo" className="underline">
            Conteúdo
          </Link>{' '}
          e inicie o cliente pelo start-lab.bat.
        </Card>
      )}
      <div className="grid gap-4 lg:grid-cols-[22rem_1fr]">
        <Card className="space-y-3">
          <div className="relative">
            <Search className="pointer-events-none absolute top-2.5 left-2.5 size-4 text-muted" />
            <Input className="pl-8" placeholder="Nome ou classe (2+ letras)" value={search} onChange={(e) => setSearch(e.target.value)} />
          </div>
          <div className="flex flex-wrap gap-1">
            {FILTERS.map(([key, label]) => (
              <Button key={key} size="sm" variant={filter === key ? 'default' : 'ghost'} onClick={() => setFilter(key)}>
                {label}
              </Button>
            ))}
          </div>
          <Button size="sm" variant="outline" onClick={() => setMode({ kind: 'new' })}>
            <PackagePlus /> Novo item
          </Button>
          {list.error && <p className="text-sm text-danger">{errorText(list.error)}</p>}
          <ul className="max-h-[32rem] space-y-1 overflow-y-auto">
            {(list.data ?? []).map((item) => (
              <li key={item.item_class}>
                <button
                  type="button"
                  className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left hover:bg-raised"
                  onClick={() => setPicked(item.item_class)}
                >
                  <ItemIcon icon={item.icon} size="sm" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm">{item.name ?? item.item_class}</span>
                    <span className="block truncate font-mono text-xs text-muted">{item.item_class}</span>
                  </span>
                  {item.origin !== 'base' && <Badge tone={ORIGIN[item.origin]?.tone}>{ORIGIN[item.origin]?.label}</Badge>}
                  {item.hidden && <Badge tone="danger">oculto</Badge>}
                  {item.pending && <Badge tone="accent">pendente</Badge>}
                </button>
              </li>
            ))}
          </ul>
        </Card>
        <div>
          {mode ? (
            <ItemForm
              key={`${mode.kind}-${mode.source?.item_class ?? ''}`}
              mode={mode}
              onCancel={() => setMode(null)}
              onDone={(itemClass) => {
                setSaved(itemClass);
                setMode(null);
                setPicked(itemClass);
                refresh();
              }}
            />
          ) : detail.data ? (
            <Detail item={detail.data} onAction={setPending} onForm={setMode} />
          ) : (
            <Card className="py-16 text-center text-sm text-muted">
              {detail.error ? errorText(detail.error) : 'Busque um item para editar, clonar, ocultar ou remover, ou crie um novo a partir de um item-modelo.'}
            </Card>
          )}
        </div>
      </div>
      <Modal open={!!pending} title={pending ? ACTION_TEXT[pending.kind].title : ''} icon={<Upload />} onClose={() => setPending(null)}>
        {pending && (
          <ActionForm
            submitLabel={ACTION_TEXT[pending.kind].submit}
            danger={pending.kind === 'remove'}
            onCancel={() => setPending(null)}
            onDone={() => {
              setSaved(pending.item.item_class);
              setPending(null);
              refresh();
            }}
            onSubmit={async (operationId) => {
              const target = pending.item.item_class;
              if (pending.kind === 'remove') await removeItem(target, operationId);
              else if (pending.kind === 'restore') await restoreItem(target, operationId);
              else await hideItem(target, operationId, pending.kind === 'hide');
            }}
          >
            <p className="text-sm">{ACTION_TEXT[pending.kind].text}</p>
            {pending.item.usage.total + pending.item.usage.queued > 0 && (
              <p className="text-sm text-danger">
                {pending.item.usage.total} itens no jogo e {pending.item.usage.queued} entregas pendentes com esta classe.
              </p>
            )}
          </ActionForm>
        )}
      </Modal>
    </section>
  );
}

