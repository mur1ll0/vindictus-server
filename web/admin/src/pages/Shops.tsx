import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Plus, Search, Store, Trash2 } from 'lucide-react';
import { useMemo, useState } from 'react';
import { api, type components, fail } from '@/api/client';
import { createEdit, type ShopRow, shop, shops } from '@/api/content';
import { sessionQuery } from '@/api/session';
import { ActionForm, errorText } from '@/components/forms';
import { Modal } from '@/components/Modal';
import { Stat } from '@/components/Stat';
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
          onSubmit={async (operationId, reason) => {
            await createEdit(operationId, reason, { kind: 'shop_row_remove', shop_id: shopId, order: row.order });
          }}
        >
          <p className="text-sm text-muted">A linha some da loja quando a versão for aplicada.</p>
        </ActionForm>
      ) : (
        <ActionForm
          submitLabel={row ? 'Salvar mudança' : 'Adicionar linha'}
          onCancel={onClose}
          onDone={onDone}
          onSubmit={async (operationId, reason) => {
            await createEdit(operationId, reason, {
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
                        <span className="text-sm font-medium">{c.name}</span>{' '}
                        <span className="font-mono text-xs text-muted">{c.class}</span>
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

function ShopModal({ shopId, canEdit, onClose }: { shopId: string; canEdit: boolean; onClose: () => void }) {
  const queryClient = useQueryClient();
  const rows = useQuery({ queryKey: ['shop', shopId], queryFn: () => shop(shopId) });
  const [edit, setEdit] = useState<ShopRow | 'new' | null>(null);
  const nextOrder = useMemo(() => Math.max(0, ...(rows.data ?? []).map((r) => r.order)) + 1, [rows.data]);
  const done = () => {
    setEdit(null);
    void queryClient.invalidateQueries({ queryKey: ['content'] });
    void queryClient.invalidateQueries({ queryKey: ['audit'] });
  };
  return (
    <Modal open title={shopId} subtitle="Como o jogo vende agora (preço efetivo)." icon={<Store className="size-5 text-muted" />} onClose={onClose} width="max-w-3xl">
      <div className="space-y-3">
        {canEdit && (
          <Button variant="outline" size="sm" onClick={() => setEdit('new')}>
            <Plus /> Adicionar linha
          </Button>
        )}
        <div className="grid max-h-[60vh] gap-2 overflow-y-auto sm:grid-cols-2">
          {(rows.data ?? []).map((r) => (
            <button
              key={r.order}
              type="button"
              disabled={!canEdit}
              onClick={() => setEdit(r)}
              className="rounded-xl border border-border bg-surface p-3 text-left transition-colors enabled:hover:border-accent/70"
            >
              <div className="flex items-start gap-2">
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
                {r.restricted && <Badge tone="danger">limitado</Badge>}
                {r.character_bind && <Badge>vinculado</Badge>}
                {r.feature && <Badge>{r.feature}</Badge>}
              </div>
            </button>
          ))}
        </div>
        {rows.error && <p className="text-sm text-danger">{errorText(rows.error)}</p>}
      </div>
      {edit && (
        <RowModal shopId={shopId} row={edit === 'new' ? null : edit} nextOrder={nextOrder} onClose={() => setEdit(null)} onDone={done} />
      )}
    </Modal>
  );
}

export function ShopsPage() {
  const { data: session } = useQuery(sessionQuery);
  const canEdit = session?.role === 'admin' || session?.role === 'gm';
  const list = useQuery({ queryKey: ['shops'], queryFn: shops });
  const [filter, setFilter] = useState('');
  const [open, setOpen] = useState<string | null>(null);
  const visible = (list.data ?? []).filter((s) => s.shop_id.toLowerCase().includes(filter.trim().toLowerCase()));
  return (
    <section className="space-y-4">
      <div className="relative max-w-sm">
        <Search className="pointer-events-none absolute top-2.5 left-3 size-4 text-muted" />
        <Input aria-label="Filtrar lojas" placeholder="Filtrar lojas (ex.: ferghus)" className="pl-9" value={filter} onChange={(e) => setFilter(e.target.value)} />
      </div>
      <p className="text-sm text-muted">{list.isPending ? 'Carregando…' : `${visible.length} loja(s)`}</p>
      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        {visible.map((s) => (
          <button
            key={s.shop_id}
            type="button"
            onClick={() => setOpen(s.shop_id)}
            className="flex items-center gap-3 rounded-xl border border-border bg-surface p-4 text-left shadow-sm transition-colors hover:border-accent/70"
          >
            <span className="flex size-10 items-center justify-center rounded-lg bg-accent-soft text-accent">
              <Store className="size-5" />
            </span>
            <div className="min-w-0">
              <p className="truncate font-semibold">{s.shop_id}</p>
              <p className="text-xs text-muted">{s.rows} linha(s)</p>
            </div>
          </button>
        ))}
      </div>
      {list.error && <p className="text-sm text-danger">{errorText(list.error)}</p>}
      {open && <ShopModal shopId={open} canEdit={canEdit} onClose={() => setOpen(null)} />}
      <Stat label="Como aplicar">Edições → Conteúdo → Publicar (admin) → no host: pwsh scripts/content/apply-content-edits.ps1</Stat>
    </section>
  );
}
