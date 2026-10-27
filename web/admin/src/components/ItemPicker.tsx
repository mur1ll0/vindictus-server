import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { ChevronLeft, ChevronRight, LayoutGrid, Search } from 'lucide-react';
import { useState } from 'react';
import { CLASS_NAME } from '@/api/characters';
import { api, type components, fail } from '@/api/client';
import type { paths } from '@/api/schema';
import { ItemIcon } from '@/components/ItemIcon';
import { Modal } from '@/components/Modal';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { cn } from '@/lib/utils';

export type PickerItem = components['schemas']['PickerItem'];
type TradeCategory = components['schemas']['TradeCategory'];
type PickerQuery = NonNullable<paths['/api/items/catalog']['get']['parameters']['query']>;

const PER_PAGE = 30;

async function tradeCategories(): Promise<TradeCategory[]> {
  const { data, error, response } = await api.GET('/api/items/categories');
  if (!data) fail(response, error);
  return data;
}

async function pickItems(query: PickerQuery) {
  const { data, error, response } = await api.GET('/api/items/catalog', { params: { query } });
  if (!data) fail(response, error);
  return data;
}

/** The game's marketplace categories, shared by every picker of the session. */
export function useTradeCategories() {
  return useQuery({ queryKey: ['trade-categories'], queryFn: tradeCategories, staleTime: Infinity });
}

function int(value: string) {
  return value.trim() === '' ? undefined : Number(value);
}

/**
 * Item catalog by the game's marketplace categories (Weapon → Longsword, Plate Armor → Helm…),
 * with search, class and level filters, page by page. Picking an item closes the modal.
 */
export function ItemPickerModal({
  title = 'Escolher item',
  characterClass,
  onPick,
  onClose,
}: {
  title?: string;
  /** Start filtered by this character class (ClassRestriction). */
  characterClass?: number;
  onPick: (item: PickerItem) => void;
  onClose: () => void;
}) {
  const categories = useTradeCategories();
  const [category, setCategory] = useState<string | null>(null);
  const [sub, setSub] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [klass, setKlass] = useState(characterClass === undefined ? '' : String(characterClass));
  const [minLevel, setMinLevel] = useState('');
  const [maxLevel, setMaxLevel] = useState('');
  const [page, setPage] = useState(1);
  const term = search.trim();
  const query: PickerQuery = {
    search: term,
    category: category ?? undefined,
    sub: sub ?? undefined,
    class: int(klass),
    min_level: int(minLevel),
    max_level: int(maxLevel),
    page,
    per_page: PER_PAGE,
  };
  const ready = category !== null || term.length >= 2;
  const found = useQuery({
    queryKey: ['item-picker', query],
    queryFn: () => pickItems(query),
    enabled: ready,
    placeholderData: keepPreviousData,
  });
  const pages = found.data ? Math.max(1, Math.ceil(found.data.total / found.data.per_page)) : 1;
  // Any filter change starts again from the first page.
  const reset = <T,>(set: (v: T) => void) => (v: T) => {
    set(v);
    setPage(1);
  };
  const choose = (main: string | null, subCode: string | null) => {
    setCategory(main);
    setSub(subCode);
    setPage(1);
  };

  return (
    <Modal open title={title} subtitle="Categorias do marketplace do jogo" icon={<LayoutGrid className="size-5 text-muted" />} onClose={onClose} width="max-w-5xl">
      <div className="grid gap-4 md:grid-cols-[15rem_1fr]">
        <nav aria-label="Categorias" className="max-h-[65vh] space-y-1 overflow-y-auto pr-1 text-sm">
          <button
            type="button"
            onClick={() => choose(null, null)}
            className={cn('w-full rounded-md px-2 py-1 text-left hover:bg-raised', category === null && 'bg-raised font-semibold')}
          >
            Todas (busca por nome)
          </button>
          {categories.error && <p className="px-2 text-danger">{String(categories.error.message)}</p>}
          {(categories.data ?? []).map((c) => (
            <div key={c.code}>
              <button
                type="button"
                onClick={() => choose(c.code, null)}
                className={cn(
                  'w-full rounded-md px-2 py-1 text-left font-medium hover:bg-raised',
                  category === c.code && sub === null && 'bg-raised font-semibold',
                )}
              >
                {c.name}
              </button>
              {category === c.code && (
                <ul className="ml-3 border-l border-border pl-2">
                  {c.subs.map((s) => (
                    <li key={s.code}>
                      <button
                        type="button"
                        onClick={() => choose(c.code, s.code)}
                        className={cn(
                          'flex w-full justify-between rounded-md px-2 py-0.5 text-left text-muted hover:bg-raised hover:text-fg',
                          sub === s.code && 'bg-raised text-fg',
                        )}
                      >
                        <span className="truncate">{s.name}</span>
                        <span className="text-xs">{s.count}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          ))}
        </nav>
        <div className="min-w-0 space-y-3">
          <div className="grid gap-2 sm:grid-cols-[1fr_9rem_5rem_5rem]">
            <div className="space-y-1">
              <Label htmlFor="picker-search">Nome ou classe</Label>
              <div className="relative">
                <Search className="pointer-events-none absolute top-2.5 left-3 size-4 text-muted" />
                <Input
                  id="picker-search"
                  className="pl-9"
                  placeholder={category ? 'Opcional' : '2+ letras'}
                  value={search}
                  onChange={(e) => reset(setSearch)(e.target.value)}
                />
              </div>
            </div>
            <div className="space-y-1">
              <Label htmlFor="picker-class">Classe</Label>
              <select
                id="picker-class"
                className="h-9 w-full rounded-md border border-border bg-surface px-2 text-sm"
                value={klass}
                onChange={(e) => reset(setKlass)(e.target.value)}
              >
                <option value="">Todas</option>
                {Object.entries(CLASS_NAME).map(([value, name]) => (
                  <option key={value} value={value}>
                    {name}
                  </option>
                ))}
              </select>
            </div>
            <div className="space-y-1">
              <Label htmlFor="picker-min">Nível ≥</Label>
              <Input id="picker-min" inputMode="numeric" value={minLevel} onChange={(e) => reset(setMinLevel)(e.target.value.replace(/[^0-9]/g, ''))} />
            </div>
            <div className="space-y-1">
              <Label htmlFor="picker-max">Nível ≤</Label>
              <Input id="picker-max" inputMode="numeric" value={maxLevel} onChange={(e) => reset(setMaxLevel)(e.target.value.replace(/[^0-9]/g, ''))} />
            </div>
          </div>
          {!ready && <p className="text-sm text-muted">Escolha uma categoria ou digite 2+ letras.</p>}
          {found.error && <p className="text-sm text-danger">{String(found.error.message)}</p>}
          {ready && found.data && (
            <>
              <ul className={cn('grid max-h-[52vh] gap-1 overflow-y-auto sm:grid-cols-2', found.isPlaceholderData && 'opacity-60')}>
                {found.data.items.map((item) => (
                  <li key={item.class}>
                    <button
                      type="button"
                      onClick={() => {
                        onPick(item);
                        onClose();
                      }}
                      className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left hover:bg-raised"
                    >
                      <ItemIcon icon={item.icon} size="sm" />
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-sm font-medium">{item.name ?? item.class}</span>
                        <span className="block truncate font-mono text-xs text-muted">
                          {item.class}
                          {item.required_level ? ` · nv ${item.required_level}` : ''}
                        </span>
                      </span>
                    </button>
                  </li>
                ))}
                {found.data.items.length === 0 && <li className="px-2 py-1 text-sm text-muted">Nada encontrado.</li>}
              </ul>
              <div className="flex items-center justify-between text-sm text-muted">
                <span>
                  {found.data.total} item(ns) · página {found.data.page} de {pages}
                </span>
                <span className="flex gap-1">
                  <Button variant="outline" size="sm" aria-label="Página anterior" disabled={page <= 1} onClick={() => setPage(page - 1)}>
                    <ChevronLeft />
                  </Button>
                  <Button variant="outline" size="sm" aria-label="Próxima página" disabled={page >= pages} onClick={() => setPage(page + 1)}>
                    <ChevronRight />
                  </Button>
                </span>
              </div>
            </>
          )}
        </div>
      </div>
    </Modal>
  );
}

/** A class field with a button that opens the picker; the typed text stays editable. */
export function ItemClassField({
  id,
  label,
  value,
  onChange,
  required,
  readOnly,
  characterClass,
  pickTitle,
}: {
  id: string;
  label: string;
  value: string;
  onChange: (value: string, item?: PickerItem) => void;
  required?: boolean;
  readOnly?: boolean;
  characterClass?: number;
  pickTitle?: string;
}) {
  const [open, setOpen] = useState(false);
  return (
    <div className="space-y-1.5">
      <Label htmlFor={id}>{label}</Label>
      <div className="flex gap-2">
        <Input id={id} required={required} readOnly={readOnly} className="font-mono" value={value} onChange={(e) => onChange(e.target.value.trim())} />
        {!readOnly && (
          <Button type="button" variant="outline" onClick={() => setOpen(true)}>
            <LayoutGrid /> Catálogo
          </Button>
        )}
      </div>
      {open && <ItemPickerModal title={pickTitle} characterClass={characterClass} onPick={(item) => onChange(item.class, item)} onClose={() => setOpen(false)} />}
    </div>
  );
}
