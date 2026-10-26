import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Plus, Search, Store, Trash2 } from 'lucide-react';
import { useMemo, useState } from 'react';
import { api, type components, fail } from '@/api/client';
import { createEdit, type ShopRow, type ShopSummary, shop, shops } from '@/api/content';
import { ItemIcon } from '@/components/ItemIcon';
import { sessionQuery } from '@/api/session';
import { ActionForm, errorText } from '@/components/forms';
import { Modal } from '@/components/Modal';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { cn } from '@/lib/utils';

type CatalogItem = components['schemas']['CatalogItem'];

const PENDING_NOTE =
  'A edição fica pendente; um admin publica a versão em Conteúdo e ela chega ao jogo pelo scripts/content/apply-content-edits.ps1.';

async function catalog(search: string): Promise<CatalogItem[]> {
  const { data, error, response } = await api.GET('/api/items', { params: { query: { search } } });
  if (!data) fail(response, error);
  return data;
}

function NumberField({ id, label, value, onChange }: { id: string; label: string; value: string; onChange: (v: string) => void }) {
  return (
    <div className="space-y-1.5">
      <Label htmlFor={id}>{label}</Label>
      <Input id={id} required inputMode="numeric" pattern="[0-9]{1,10}" value={value} onChange={(e) => onChange(e.target.value.replace(/[^0-9]/g, ''))} />
    </div>
  );
}

/** Add a row (free Order) or change the row at `row.order`. */
function RowModal({
  shopId,
  row,
  nextOrder,
  onClose,
  onDone,
}: {
  shopId: string;
  row: ShopRow | null;
  nextOrder: number;
  onClose: () => void;
  onDone: () => void;
}) {
  const [removing, setRemoving] = useState(false);
  const [item, setItem] = useState(row?.item_class ?? '');
  const [search, setSearch] = useState('');
  const [count, setCount] = useState(String(row?.count ?? 1));
  const [priceItem, setPriceItem] = useState(row?.price_item_class ?? 'gold');
  const [price, setPrice] = useState(String(row?.price_count ?? ''));
  const term = search.trim();
  const found = useQuery({ queryKey: ['items', term], queryFn: () => catalog(term), enabled: !row && term.length >= 2 });
  const order = row?.order ?? nextOrder;
  return (
    <Modal
      open
      title={row ? (row.item_name ?? row.item_class) : `Nova linha em ${shopId}`}
      subtitle={`${shopId} · ordem ${order}. ${PENDING_NOTE}`}
      icon={<Store className="size-5 text-muted" />}
      onClose={onClose}
      width="max-w-xl"
    >
      {removing && row ? (
        <ActionForm
          submitLabel="Remover linha"
          danger
          onCancel={() => setRemoving(false)}
          onDone={onDone}
          onSubmit={async (operationId) => {
            await createEdit(operationId, { kind: 'shop_row_remove', shop_id: shopId, order: row.order });
          }}
        >
          <p className="text-sm text-muted">A linha some da loja quando a versão for aplicada.</p>
        </ActionForm>
      ) : (
        <ActionForm
          submitLabel={row ? 'Salvar mudança' : 'Adicionar linha'}
          onCancel={onClose}
          onDone={onDone}
          onSubmit={async (operationId) => {
            await createEdit(operationId, {
              kind: 'shop_row',
              shop_id: shopId,
              order,
              item_class: item,
              count: Number(count),
              price_item_class: priceItem,
              price_count: Number(price),
            });
          }}
        >
          {!row && (
            <div className="space-y-1.5">
              <Label htmlFor="shop-catalog">Buscar item no catálogo</Label>
              <div className="relative">
                <Search className="pointer-events-none absolute top-2.5 left-3 size-4 text-muted" />
                <Input id="shop-catalog" className="pl-9" value={search} onChange={(e) => setSearch(e.target.value)} />
              </div>
              {term.length >= 2 && (
                <ul className="max-h-40 space-y-1 overflow-y-auto rounded-lg border border-border p-1">
                  {(found.data ?? []).map((c) => (
                    <li key={c.class}>
                      <button
                        type="button"
                        onClick={() => setItem(c.class)}
                        className={cn('w-full rounded-md px-2 py-1 text-left hover:bg-raised', item === c.class && 'bg-raised')}
                      >
                        <span className="flex items-center gap-2">
                          <ItemIcon icon={c.icon} size="sm" />
                          <span className="text-sm font-medium">{c.name}</span>
                          <span className="font-mono text-xs text-muted">{c.class}</span>
                        </span>
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          )}
          <div className="space-y-1.5">
            <Label htmlFor="shop-item">Item (classe)</Label>
            <Input id="shop-item" required className="font-mono" value={item} onChange={(e) => setItem(e.target.value.trim())} readOnly={!!row} />
          </div>
          <div className="grid grid-cols-2 gap-2">
            <NumberField id="shop-count" label="Quantidade" value={count} onChange={setCount} />
            <NumberField id="shop-price" label="Preço" value={price} onChange={setPrice} />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="shop-price-item">Moeda (gold ou classe de item)</Label>
            <Input id="shop-price-item" required className="font-mono" value={priceItem} onChange={(e) => setPriceItem(e.target.value.trim())} />
          </div>
          {row && (
            <Button type="button" variant="danger" onClick={() => setRemoving(true)}>
              <Trash2 /> Remover esta linha
            </Button>
          )}
        </ActionForm>
      )}
    </Modal>
  );
}

function ShopModal({ shop: summary, canEdit, onClose }: { shop: ShopSummary; canEdit: boolean; onClose: () => void }) {
  const shopId = summary.shop_id;
  const queryClient = useQueryClient();
  const rows = useQuery({ queryKey: ['shop', shopId], queryFn: () => shop(shopId) });
  const [edit, setEdit] = useState<ShopRow | 'new' | null>(null);
  const [search, setSearch] = useState('');
  const [all, setAll] = useState(false);
  const nextOrder = useMemo(() => Math.max(0, ...(rows.data ?? []).map((r) => r.order)) + 1, [rows.data]);
  const term = search.trim().toLowerCase();
  const visible = (rows.data ?? []).filter(
    (r) =>
      (all || r.available) &&
      (!term || r.item_class.toLowerCase().includes(term) || (r.item_name ?? '').toLowerCase().includes(term)),
  );
  const hidden = (rows.data ?? []).filter((r) => !r.available).length;
  const done = () => {
    setEdit(null);
    void queryClient.invalidateQueries({ queryKey: ['content'] });
    void queryClient.invalidateQueries({ queryKey: ['audit'] });
  };
  const npcs = summary.npcs.map((n) => `${n.name ?? n.npc} (${n.place})`).join(', ');
  return (
    <Modal
      open
      title={summary.npcs[0]?.name ?? shopId}
      subtitle={`${shopId}${npcs ? ` · ${npcs}` : ''} · preço efetivo que o jogo cobra`}
      icon={<Store className="size-5 text-muted" />}
      onClose={onClose}
      width="max-w-3xl"
    >
      <div className="space-y-3">
        <div className="flex flex-wrap items-center gap-2">
          <div className="relative min-w-0 flex-1">
            <Search className="pointer-events-none absolute top-2.5 left-3 size-4 text-muted" />
            <Input aria-label="Buscar item" placeholder="Buscar item (nome ou classe)" className="pl-9" value={search} onChange={(e) => setSearch(e.target.value)} />
          </div>
          {canEdit && (
            <Button variant="outline" size="sm" onClick={() => setEdit('new')}>
              <Plus /> Adicionar linha
            </Button>
          )}
        </div>
        {hidden > 0 && (
          <label className="flex items-center gap-2 text-sm text-muted">
            <input type="checkbox" checked={all} onChange={(e) => setAll(e.target.checked)} />
            Mostrar {hidden} linha(s) que o jogo não vende nesta região (NotSell, TestServer, eventos de outras regiões)
          </label>
        )}
        <div className="grid max-h-[60vh] gap-2 overflow-y-auto sm:grid-cols-2">
          {visible.map((r) => (
            <button
              key={r.order}
              type="button"
              disabled={!canEdit}
              onClick={() => setEdit(r)}
              className={cn(
                'rounded-xl border border-border bg-surface p-3 text-left transition-colors enabled:hover:border-accent/70',
                !r.available && 'opacity-60',
              )}
            >
              <div className="flex items-start gap-3">
                <ItemIcon icon={r.icon} />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-semibold">{r.item_name ?? r.item_class}</p>
                  <p className="truncate font-mono text-xs text-muted">
                    #{r.order} · {r.item_class}
                    {r.count > 1 && ` ×${r.count}`}
                  </p>
                </div>
                <span className="text-sm font-semibold whitespace-nowrap">
                  {r.price_count.toLocaleString('pt-BR')} {r.price_item_class === 'gold' ? 'ouro' : r.price_item_class}
                </span>
              </div>
              <div className="mt-2 flex flex-wrap gap-1">
                {!r.available && <Badge tone="danger">fora da região</Badge>}
                {r.restricted && <Badge tone="danger">limitado</Badge>}
                {r.character_bind && <Badge>vinculado</Badge>}
                {r.feature && <Badge>{r.feature}</Badge>}
              </div>
            </button>
          ))}
          {visible.length === 0 && !rows.isPending && <p className="text-sm text-muted">Nenhuma linha.</p>}
        </div>
        {rows.error && <p className="text-sm text-danger">{errorText(rows.error)}</p>}
      </div>
      {edit && (
        <RowModal shopId={shopId} row={edit === 'new' ? null : edit} nextOrder={nextOrder} onClose={() => setEdit(null)} onDone={done} />
      )}
    </Modal>
  );
}

const PLACE_ORDER = ['Colhen', 'Rocheste', 'Malina (Morvan)', 'Campos', 'Navios', 'Eventos', 'Ofícios', 'Desenvolvimento', 'Outros', 'Sem NPC'];

function placeOf(s: ShopSummary) {
  return s.npcs[0]?.place ?? 'Sem NPC';
}

export function ShopsPage() {
  const { data: session } = useQuery(sessionQuery);
  const canEdit = session?.role === 'admin' || session?.role === 'gm';
  const list = useQuery({ queryKey: ['shops'], queryFn: shops });
  const [filter, setFilter] = useState('');
  const [place, setPlace] = useState('Todos');
  const [open, setOpen] = useState<ShopSummary | null>(null);
  const term = filter.trim().toLowerCase();
  const all = list.data ?? [];
  const places = PLACE_ORDER.filter((p) => all.some((s) => placeOf(s) === p));
  const visible = all
    .filter((s) => place === 'Todos' || placeOf(s) === place)
    .filter(
      (s) =>
        !term ||
        s.shop_id.toLowerCase().includes(term) ||
        s.npcs.some((n) => n.npc.toLowerCase().includes(term) || (n.name ?? '').toLowerCase().includes(term)),
    )
    .sort((a, b) => (a.npcs[0]?.name ?? a.shop_id).localeCompare(b.npcs[0]?.name ?? b.shop_id));
  return (
    <section className="space-y-4">
      <div role="tablist" className="flex flex-wrap gap-1 rounded-lg bg-raised p-1 text-sm">
        {['Todos', ...places].map((p) => (
          <button
            key={p}
            role="tab"
            type="button"
            aria-selected={place === p}
            onClick={() => setPlace(p)}
            className={cn('rounded-md px-3 py-1.5 text-muted', place === p && 'bg-surface font-medium text-fg shadow-sm')}
          >
            {p} <span className="text-xs">({p === 'Todos' ? all.length : all.filter((s) => placeOf(s) === p).length})</span>
          </button>
        ))}
      </div>
      <div className="relative max-w-sm">
        <Search className="pointer-events-none absolute top-2.5 left-3 size-4 text-muted" />
        <Input aria-label="Buscar loja" placeholder="Buscar por NPC ou loja (ex.: ferghus)" className="pl-9" value={filter} onChange={(e) => setFilter(e.target.value)} />
      </div>
      <p className="text-sm text-muted">{list.isPending ? 'Carregando…' : `${visible.length} loja(s)`}</p>
      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        {visible.map((s) => (
          <button
            key={s.shop_id}
            type="button"
            onClick={() => setOpen(s)}
            className="flex items-center gap-3 rounded-xl border border-border bg-surface p-4 text-left shadow-sm transition-colors hover:border-accent/70"
          >
            <span className="flex size-10 shrink-0 items-center justify-center rounded-lg bg-accent-soft text-accent">
              <Store className="size-5" />
            </span>
            <div className="min-w-0">
              <p className="truncate font-semibold">{s.npcs[0]?.name ?? s.npcs[0]?.npc ?? s.shop_id}</p>
              <p className="truncate font-mono text-xs text-muted">{s.shop_id}</p>
              <p className="text-xs text-muted">
                {placeOf(s)} · {s.available_rows} à venda{s.rows > s.available_rows && ` (${s.rows - s.available_rows} fora da região)`}
              </p>
            </div>
          </button>
        ))}
      </div>
      {list.error && <p className="text-sm text-danger">{errorText(list.error)}</p>}
      {open && <ShopModal shop={open} canEdit={canEdit} onClose={() => setOpen(null)} />}
    </section>
  );
}
