import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useParams } from '@tanstack/react-router';
import { ArrowDownWideNarrow, ArrowLeft, Clock, Coins, Gem, PackagePlus, Search, Shirt, Sparkles, Trash2, TrendingUp } from 'lucide-react';
import { useMemo, useState } from 'react';
import {
  type CharacterItem,
  type CharacterSheet,
  characterSheet,
  className,
  BAG_SLOTS,
  collectionName,
  collectionOrder,
  gold,
  isBag,
  moveItem,
  sortInventory,
} from '@/api/characters';
import { sessionQuery } from '@/api/session';
import { Avatar } from '@/components/Avatar';
import { ItemIcon } from '@/components/ItemIcon';
import { ItemTooltipBox, itemColors, itemTitle, WithItemTooltip } from '@/components/ItemTooltip';
import { CashModal, GiveItemModal, GoldModal, RemoveItemForm } from '@/components/CharacterEdits';
import { ProgressionModal, RemoveEffectModal } from '@/components/ProgressionEdits';
import { Button } from '@/components/ui/button';
import { ActionForm, errorText } from '@/components/forms';
import { Modal } from '@/components/Modal';
import { Stat } from '@/components/Stat';
import { Badge } from '@/components/ui/badge';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { cn } from '@/lib/utils';

type Tab = 'items' | 'skills' | 'effects' | 'titles';

function when(seconds?: number | null) {
  return seconds == null ? null : new Date(seconds * 1000).toLocaleString('pt-BR');
}

/** The attributes as the game writes them: class[NAME:value(arg)<arg2>,...]. */
function classEx(item: CharacterItem) {
  if (item.attributes.length === 0) return item.class;
  const parts = item.attributes.map((a) => `${a.name}:${a.value}${a.arg ? `(${a.arg})` : ''}${a.arg2 ? `<${a.arg2}>` : ''}`);
  return `${item.class}[${parts.join(',')}]`;
}

function ItemCard({ item, onOpen }: { item: CharacterItem; onOpen: () => void }) {
  return (
    <WithItemTooltip item={item}>
    <button
      type="button"
      onClick={onOpen}
      className="flex w-full flex-col gap-2 rounded-xl border border-border bg-surface p-3 text-left transition-colors hover:border-accent/70"
    >
      <div className="flex items-start gap-3">
        <ItemIcon icon={item.icon} bg={item.icon_bg} colors={itemColors(item)} />
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-semibold">{itemTitle(item)}</p>
          <p className="truncate font-mono text-xs text-muted">
            espaço {item.slot} · {item.class}
          </p>
        </div>
        {item.count > 1 && <span className="text-sm font-semibold">×{item.count.toLocaleString('pt-BR')}</span>}
      </div>
      <div className="flex flex-wrap gap-1">
        {item.equipped_part != null && (
          <Badge tone="accent">
            <Shirt className="size-3" /> vestido
          </Badge>
        )}
        {item.attributes.length > 0 && (
          <Badge>
            <Sparkles className="size-3" /> {item.attributes.length} atributo(s)
          </Badge>
        )}
        {item.expires_at != null && (
          <Badge tone="danger">
            <Clock className="size-3" /> expira
          </Badge>
        )}
        {!item.tradable && <Badge>vinculado</Badge>}
      </div>
    </button>
    </WithItemTooltip>
  );
}

function ItemModal({
  item,
  cid,
  canManage,
  onClose,
  onChanged,
}: {
  item: CharacterItem;
  cid: string;
  canManage: boolean;
  onClose: () => void;
  onChanged: () => void;
}) {
  const [removing, setRemoving] = useState(false);
  return (
    <Modal open title={itemTitle(item)} subtitle={item.class} onClose={onClose} width="max-w-2xl">
      {removing ? (
        <RemoveItemForm
          cid={cid}
          item={item}
          onCancel={() => setRemoving(false)}
          onDone={() => {
            onChanged();
            onClose();
          }}
        />
      ) : (
      <div className="space-y-4">
        <ItemTooltipBox item={item} />
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
          <Stat label="Quantidade">{item.count.toLocaleString('pt-BR')}</Stat>
          <Stat label="Local">{`${collectionName(item.collection)} · espaço ${item.slot}`}</Stat>
          <Stat label="Vestido">{item.equipped_part != null ? `sim (parte ${item.equipped_part})` : 'não'}</Stat>
          <Stat label="Negociável">{item.tradable ? 'sim' : 'não'}</Stat>
          <Stat label="Expira">{when(item.expires_at) ?? 'não'}</Stat>
          <Stat label="Durabilidade perdida">{item.durability_lost ?? '—'}</Stat>
        </div>
        {item.attributes.length > 0 && (
          <div className="space-y-1">
            <p className="text-sm font-medium">Atributos</p>
            <p className="rounded-md bg-raised px-2 py-1 font-mono text-xs break-all">{classEx(item)}</p>
          </div>
        )}
        <p className="text-xs text-muted">ID do item: {item.id}</p>
        {canManage && (
          <Button variant="danger" onClick={() => setRemoving(true)}>
            <Trash2 /> Remover
          </Button>
        )}
      </div>
      )}
    </Modal>
  );
}

const DRAG_REASON = 'Movido arrastando no painel';
const SORTABLE = (c: number) => isBag(c) || c === 100 || c === 103;

function SlotCell({
  item,
  slot,
  draggable,
  dropping,
  onOpen,
  onDragStart,
  onDragOver,
  onDragLeave,
  onDrop,
}: {
  item?: CharacterItem;
  slot: number;
  draggable: boolean;
  dropping: boolean;
  onOpen: () => void;
  onDragStart: () => void;
  onDragOver: () => void;
  onDragLeave: () => void;
  onDrop: () => void;
}) {
  return (
    <div
      onDragOver={(e) => {
        // A worn item keeps its bag slot; dropping there would swap it out of place.
        if (!draggable || item?.equipped_part != null) return;
        e.preventDefault();
        onDragOver();
      }}
      onDragLeave={onDragLeave}
      onDrop={(e) => {
        e.preventDefault();
        if (item?.equipped_part == null) onDrop();
      }}
      className={cn(
        'relative flex aspect-square items-center justify-center rounded-lg border border-border bg-raised/60',
        dropping && 'border-accent bg-accent-soft',
      )}
    >
      {item ? (
        <WithItemTooltip item={item} className="size-full">
        <button
          type="button"
          draggable={draggable && item.equipped_part == null}
          onDragStart={(e) => {
            e.dataTransfer.effectAllowed = 'move';
            e.dataTransfer.setData('text/plain', item.id);
            onDragStart();
          }}
          onClick={onOpen}
          className={cn('flex size-full items-center justify-center rounded-lg hover:ring-2 hover:ring-accent/60', draggable && 'cursor-grab')}
        >
          <ItemIcon icon={item.icon} bg={item.icon_bg} colors={itemColors(item)} />
          {item.count > 1 && (
            <span className="absolute right-1 bottom-0.5 text-[11px] font-semibold text-fg [text-shadow:0_0_3px_var(--color-surface)]">
              {item.count.toLocaleString('pt-BR')}
            </span>
          )}
          {item.equipped_part != null && <Shirt className="absolute top-1 left-1 size-3 text-accent" />}
        </button>
        </WithItemTooltip>
      ) : (
        <span className="text-[10px] text-muted/50">{slot}</span>
      )}
    </div>
  );
}

function SortModal({ cid, current, onClose, onDone }: { cid: string; current: number | null; onClose: () => void; onDone: () => void }) {
  const [target, setTarget] = useState(-1);
  const [moved, setMoved] = useState<number | null>(null);
  const options = [{ value: -1, label: 'Todas as bolsas' }];
  if (current != null && SORTABLE(current)) options.push({ value: current, label: `Só ${collectionName(current)}` });
  return (
    <Modal
      open
      title="Organizar inventário"
      subtitle="A mesma regra do botão Organizar do jogo. O personagem precisa estar fora do jogo."
      icon={<ArrowDownWideNarrow className="size-5 text-muted" />}
      onClose={onClose}
    >
      {moved == null ? (
        <ActionForm
          submitLabel="Organizar"
          onCancel={onClose}
          onDone={onDone}
          onSubmit={async (op, reason) => setMoved(await sortInventory(cid, target, op, reason))}
        >
          <div role="radiogroup" className="grid gap-1 rounded-lg bg-raised p-1 text-sm">
            {options.map((o) => (
              <button
                key={o.value}
                type="button"
                role="radio"
                aria-checked={target === o.value}
                onClick={() => setTarget(o.value)}
                className={cn('rounded-md py-1.5 text-muted', target === o.value && 'bg-surface font-medium text-fg shadow-sm')}
              >
                {o.label}
              </button>
            ))}
          </div>
        </ActionForm>
      ) : (
        <p className="text-sm">{moved} item(ns) mudaram de lugar.</p>
      )}
    </Modal>
  );
}

function Inventory({ sheet, canManage, onChanged }: { sheet: CharacterSheet; canManage: boolean; onChanged: () => void }) {
  const cid = sheet.summary.cid;
  const [filter, setFilter] = useState('');
  const [open, setOpen] = useState<CharacterItem | null>(null);
  const [sorting, setSorting] = useState(false);
  const [dragged, setDragged] = useState<CharacterItem | null>(null);
  const [over, setOver] = useState<string | null>(null);
  const [moveError, setMoveError] = useState<unknown>(null);
  const [busy, setBusy] = useState(false);
  const tabs = useMemo(() => {
    const map = new Map<number, CharacterItem[]>();
    for (const item of sheet.items) map.set(item.collection, [...(map.get(item.collection) ?? []), item]);
    for (const items of map.values()) items.sort((x, y) => x.slot - y.slot);
    return [...map.entries()].sort((a, b) => collectionOrder(a[0]) - collectionOrder(b[0]));
  }, [sheet.items]);
  const [tab, setTab] = useState<number | null>(null);
  const term = filter.trim().toLowerCase();
  const matches = (i: CharacterItem) => !term || i.class.toLowerCase().includes(term) || (i.name ?? '').toLowerCase().includes(term);
  const counts = new Map(tabs.map(([c, items]) => [c, items.filter(matches).length]));
  // While searching, stay on a tab that has matches.
  const firstMatch = tabs.find(([c]) => (counts.get(c) ?? 0) > 0)?.[0];
  const current = tab != null && (!term || (counts.get(tab) ?? 0) > 0) ? tab : (term ? firstMatch : tabs[0]?.[0]) ?? null;
  const all = tabs.find(([c]) => c === current)?.[1] ?? [];
  const items = all.filter(matches);
  const grid = current != null && isBag(current) && !term;
  const move = async (item: CharacterItem, storage: number, slot: number) => {
    setOver(null);
    setDragged(null);
    if (item.collection === storage && item.slot === slot) return;
    setBusy(true);
    setMoveError(null);
    try {
      await moveItem(cid, item.id, storage, slot, crypto.randomUUID(), DRAG_REASON);
      onChanged();
    } catch (e) {
      setMoveError(e);
    } finally {
      setBusy(false);
    }
  };
  // Dropped on another bag's tab: its first free slot.
  const dropOnTab = (collection: number) => {
    if (!dragged || !isBag(collection)) return;
    const used = new Set((tabs.find(([c]) => c === collection)?.[1] ?? []).map((i) => i.slot));
    const slot = Array.from({ length: BAG_SLOTS }, (_, n) => n).find((n) => !used.has(n));
    if (slot == null) {
      setMoveError(new Error(`${collectionName(collection)} está cheia.`));
      return;
    }
    void move(dragged, collection, slot);
  };
  const bySlot = new Map(all.map((i) => [i.slot, i]));
  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <div className="relative max-w-sm min-w-0 flex-1">
          <Search className="pointer-events-none absolute top-2.5 left-3 size-4 text-muted" />
          <Input
            aria-label="Buscar item em todas as abas"
            placeholder="Buscar em todas as abas (nome ou classe)"
            className="pl-9"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
        </div>
        {canManage && (
          <Button variant="outline" size="sm" onClick={() => setSorting(true)}>
            <ArrowDownWideNarrow /> Organizar
          </Button>
        )}
      </div>
      <div role="tablist" className="flex flex-wrap gap-1 rounded-lg bg-raised p-1 text-sm">
        {tabs.map(([c, list]) => {
          const n = counts.get(c) ?? 0;
          const target = dragged != null && isBag(c) && c !== current;
          return (
            <button
              key={c}
              role="tab"
              type="button"
              aria-selected={current === c}
              onClick={() => setTab(c)}
              onDragOver={(e) => {
                if (!target) return;
                e.preventDefault();
                setOver(`tab-${c}`);
              }}
              onDragLeave={() => setOver(null)}
              onDrop={(e) => {
                e.preventDefault();
                dropOnTab(c);
              }}
              className={cn(
                'rounded-md px-3 py-1.5 text-muted',
                current === c && 'bg-surface font-medium text-fg shadow-sm',
                term && n === 0 && 'opacity-40',
                term && n > 0 && current !== c && 'text-accent',
                target && 'ring-1 ring-accent/50',
                over === `tab-${c}` && 'bg-accent-soft text-accent',
              )}
            >
              {collectionName(c)} <span className="text-xs">({term ? `${n}/${list.length}` : list.length})</span>
            </button>
          );
        })}
      </div>
      {term && (
        <p className="text-sm text-muted">
          {[...counts.values()].reduce((a, b) => a + b, 0)} item(ns) em{' '}
          {tabs.filter(([c]) => (counts.get(c) ?? 0) > 0).map(([c]) => collectionName(c)).join(', ') || 'nenhuma aba'}
        </p>
      )}
      {canManage && grid && (
        <p className="text-xs text-muted">
          Arraste um item para outro espaço (troca de lugar ou junta a pilha, como no jogo) ou para a aba de outra bolsa. Só
          com o personagem fora do jogo.
        </p>
      )}
      {moveError != null && <p className="text-sm text-danger">{errorText(moveError)}</p>}
      {grid ? (
        <div className={cn('grid max-w-3xl grid-cols-6 gap-1.5 sm:grid-cols-8', busy && 'pointer-events-none opacity-60')}>
          {Array.from({ length: BAG_SLOTS }, (_, slot) => (
            <SlotCell
              key={slot}
              slot={slot}
              item={bySlot.get(slot)}
              draggable={canManage}
              dropping={over === `slot-${slot}`}
              onOpen={() => {
                const item = bySlot.get(slot);
                if (item) setOpen(item);
              }}
              onDragStart={() => setDragged(bySlot.get(slot) ?? null)}
              onDragOver={() => setOver(`slot-${slot}`)}
              onDragLeave={() => setOver(null)}
              onDrop={() => {
                if (dragged && current != null) void move(dragged, current, slot);
              }}
            />
          ))}
        </div>
      ) : (
        <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
          {items.map((item) => (
            <ItemCard key={item.id} item={item} onOpen={() => setOpen(item)} />
          ))}
        </div>
      )}
      {!grid && items.length === 0 && <p className="text-sm text-muted">Nenhum item.</p>}
      {open && (
        <ItemModal
          item={open}
          cid={cid}
          canManage={canManage}
          onClose={() => setOpen(null)}
          onChanged={onChanged}
        />
      )}
      {sorting && (
        <SortModal
          cid={cid}
          current={current}
          onClose={() => setSorting(false)}
          onDone={() => {
            onChanged();
          }}
        />
      )}
    </div>
  );
}

function SimpleList({ rows, empty }: { rows: { key: string; title: string; detail: string; badge?: string }[]; empty: string }) {
  if (rows.length === 0) return <p className="text-sm text-muted">{empty}</p>;
  return (
    <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
      {rows.map((r) => (
        <div key={r.key} className="rounded-xl border border-border bg-surface p-3">
          <div className="flex items-start gap-2">
            <p className="min-w-0 flex-1 truncate text-sm font-semibold">{r.title}</p>
            {r.badge && <Badge tone="accent">{r.badge}</Badge>}
          </div>
          <p className="truncate font-mono text-xs text-muted">{r.detail}</p>
        </div>
      ))}
    </div>
  );
}

export function CharacterPage() {
  const { cid } = useParams({ strict: false }) as { cid: string };
  const query = useQuery({ queryKey: ['character', cid], queryFn: () => characterSheet(cid) });
  const [tab, setTab] = useState<Tab>('items');
  const [edit, setEdit] = useState<'item' | 'gold' | 'cash' | 'progression' | null>(null);
  const [removeEffect, setRemoveEffect] = useState<{ kind: string; name?: string | null } | null>(null);
  const queryClient = useQueryClient();
  const { data: session } = useQuery(sessionQuery);
  const canManage = session?.role === 'admin' || session?.role === 'gm';
  const changed = () => {
    void queryClient.invalidateQueries({ queryKey: ['character', cid] });
    void queryClient.invalidateQueries({ queryKey: ['audit'] });
    setEdit(null);
  };
  const sheet = query.data;
  if (!sheet) return <p className="text-sm text-muted">{query.error ? errorText(query.error) : 'Carregando…'}</p>;
  const s = sheet.summary;
  const tabs: { id: Tab; label: string; count: number }[] = [
    { id: 'items', label: 'Inventário', count: sheet.items.length },
    { id: 'skills', label: 'Skills', count: sheet.skills.length },
    { id: 'effects', label: 'Efeitos', count: sheet.status_effects.length },
    { id: 'titles', label: 'Títulos', count: sheet.titles.filter((t) => t.acquired).length },
  ];
  return (
    <section className="space-y-5">
      <button type="button" onClick={() => history.back()} className="flex items-center gap-1 text-sm text-muted hover:text-fg">
        <ArrowLeft className="size-4" /> Voltar
      </button>
      <Card>
        <div className="flex flex-wrap items-center gap-4">
          <Avatar name={s.name || '?'} size="lg" />
          <div className="min-w-0 flex-1">
            <p className="text-lg font-semibold">{s.name}</p>
            <p className="text-sm text-muted">
              {className(s.class)} · nível {s.level} · conta de jogo {sheet.owner}
            </p>
          </div>
          <div className="flex items-center gap-2 rounded-lg bg-accent-soft px-3 py-2 font-semibold" title="Ouro">
            <Coins className="size-4 text-accent" /> {gold(sheet.gold)}
          </div>
          <div className="flex items-center gap-2 rounded-lg bg-raised px-3 py-2 font-semibold" title="Cash da conta">
            <Gem className="size-4 text-muted" /> {gold(sheet.cash)}
          </div>
          {s.connected ? <Badge tone="ok">online</Badge> : <Badge>offline</Badge>}
        </div>
        {canManage && (
          <div className="mt-4 flex flex-wrap gap-2">
            <Button variant="outline" onClick={() => setEdit('item')}>
              <PackagePlus /> Dar item
            </Button>
            <Button variant="outline" onClick={() => setEdit('gold')}>
              <Coins /> Ouro
            </Button>
            <Button variant="outline" onClick={() => setEdit('cash')}>
              <Gem /> Cash
            </Button>
            <Button variant="outline" onClick={() => setEdit('progression')}>
              <TrendingUp /> Progressão
            </Button>
            {s.connected && (
              <span className="self-center text-sm text-danger">Online: itens, ouro e progressão só mudam com a personagem fora do jogo.</span>
            )}
          </div>
        )}
        <div className="mt-5 grid grid-cols-2 gap-2 sm:grid-cols-4 lg:grid-cols-6">
          <Stat label="EXP">{s.exp.toLocaleString('pt-BR')}</Stat>
          <Stat label="AP">{`${s.ap} (usados ${sheet.stats.total_used_ap})`}</Stat>
          <Stat label="Força">{sheet.stats.str}</Stat>
          <Stat label="Agilidade">{sheet.stats.dex}</Stat>
          <Stat label="Inteligência">{sheet.stats.int}</Stat>
          <Stat label="Vontade">{sheet.stats.will}</Stat>
          <Stat label="Sorte">{sheet.stats.luck}</Stat>
          <Stat label="HP">{sheet.stats.hp}</Stat>
          <Stat label="Stamina">{sheet.stats.stamina}</Stat>
          <Stat label="Tempo de jogo">{`${Math.round(sheet.stats.play_time / 3600)} h`}</Stat>
          <Stat label="CID">{s.cid}</Stat>
        </div>
      </Card>
      <div role="tablist" className="flex flex-wrap gap-1 rounded-lg bg-raised p-1 text-sm sm:inline-flex">
        {tabs.map((t) => (
          <button
            key={t.id}
            role="tab"
            type="button"
            aria-selected={tab === t.id}
            onClick={() => setTab(t.id)}
            className={cn('rounded-md px-3 py-1.5 text-muted', tab === t.id && 'bg-surface font-medium text-fg shadow-sm')}
          >
            {t.label} <span className="text-xs">({t.count})</span>
          </button>
        ))}
      </div>
      {tab === 'items' && <Inventory sheet={sheet} canManage={canManage} onChanged={changed} />}
      {edit === 'item' && <GiveItemModal cid={s.cid} onClose={() => setEdit(null)} onDone={changed} />}
      {edit === 'gold' && <GoldModal cid={s.cid} current={sheet.gold} onClose={() => setEdit(null)} onDone={changed} />}
      {edit === 'cash' && <CashModal cid={s.cid} current={sheet.cash} onClose={() => setEdit(null)} onDone={changed} />}
      {edit === 'progression' && (
        <ProgressionModal cid={s.cid} level={s.level} onClose={() => setEdit(null)} onDone={changed} />
      )}
      {removeEffect && (
        <RemoveEffectModal
          cid={s.cid}
          effect={removeEffect}
          onClose={() => setRemoveEffect(null)}
          onDone={() => {
            setRemoveEffect(null);
            changed();
          }}
        />
      )}
      {tab === 'skills' && (
        <SimpleList
          empty="Nenhuma skill."
          rows={sheet.skills.map((k) => ({
            key: k.skill,
            title: k.name ?? k.skill,
            detail: k.skill,
            badge: k.locked ? 'bloqueada' : `rank ${k.rank}`,
          }))}
        />
      )}
      {tab === 'effects' && canManage && sheet.status_effects.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {sheet.status_effects.map((e, i) => (
            <Button key={`${e.kind}-${i}`} variant="danger" size="sm" onClick={() => setRemoveEffect(e)}>
              <Trash2 /> Remover {e.name ?? e.kind}
            </Button>
          ))}
        </div>
      )}
      {tab === 'effects' && (
        <SimpleList
          empty="Nenhum efeito."
          rows={sheet.status_effects.map((e, i) => ({
            key: `${e.kind}-${i}`,
            title: e.name ?? e.kind,
            detail: `${e.kind} · ${when(e.expires_at) ? `até ${when(e.expires_at)}` : 'sem prazo'}`,
            badge: `nível ${e.level}`,
          }))}
        />
      )}
      {tab === 'titles' && (
        <SimpleList
          empty="Nenhum título."
          rows={sheet.titles
            .filter((t) => t.acquired)
            .map((t) => ({
              key: String(t.title_id),
              title: `Título ${t.title_id}`,
              detail: t.selected ? 'em uso' : 'conquistado',
              badge: t.selected ? 'em uso' : undefined,
            }))}
        />
      )}
    </section>
  );
}
