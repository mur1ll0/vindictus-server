import { useQuery } from '@tanstack/react-query';
import { CheckCircle2, CircleDashed, Info } from 'lucide-react';
import { type ReactNode, useEffect, useRef, useState } from 'react';
import { api, type components, fail } from '@/api/client';
import { ItemIcon } from '@/components/ItemIcon';
import { cn } from '@/lib/utils';

type Detail = components['schemas']['EventDetailView'];
type Item = components['schemas']['EventItemView'];
type Trade = components['schemas']['EventTradeView'];

async function eventDetail(key: string): Promise<Detail> {
  const { data, error, response } = await api.GET('/api/tuning/events/{key}', { params: { path: { key } } });
  if (!data) fail(response, error);
  return data;
}

/** The table each section of the description reads. */
const SECTION_TABLE = {
  shops: 'ShopInfo',
  coupons: 'CouponShopItemInfo',
  drops: 'EventDropItemInfo',
  quests: 'QuestInfo',
  stories: 'StoryLineInfo',
  titles: 'TitleInfo',
  periodic: 'PeriodicEventInfo',
  goals: 'AllUserGoalInfo',
} as const;

function ItemLine({ item, suffix }: { item: Item; suffix?: ReactNode }) {
  return (
    <span className="flex min-w-0 items-center gap-2">
      <ItemIcon icon={item.icon} size="sm" />
      <span className="min-w-0 flex-1 truncate" title={item.class}>
        {item.name ?? item.class}
        {item.count > 1 && <span className="text-muted"> ×{item.count}</span>}
      </span>
      {suffix}
    </span>
  );
}

function Price({ trade }: { trade: Trade }) {
  return (
    <span className="flex shrink-0 items-center gap-1 text-xs text-muted" title={trade.price.class}>
      <ItemIcon icon={trade.price.icon} size="sm" />
      {trade.price.count} {trade.price.name ?? trade.price.class}
    </span>
  );
}

function Section({ title, works, children }: { title: string; works: boolean; children: ReactNode }) {
  return (
    <section className="space-y-1">
      <h4 className="flex items-center gap-1 text-xs font-semibold tracking-wide uppercase">
        {works ? <CheckCircle2 className="size-3.5 text-ok" /> : <CircleDashed className="size-3.5 text-muted" />}
        {title}
        <span className={cn('font-normal normal-case', works ? 'text-ok' : 'text-muted')}>
          {works ? '· funciona no servidor' : '· ainda não funciona'}
        </span>
      </h4>
      <div className={cn('space-y-1 text-sm', !works && 'opacity-70')}>{children}</div>
    </section>
  );
}

function Body({ detail }: { detail: Detail }) {
  const works = (table: string) => detail.migrated.includes(table);
  const empty =
    detail.npcs.length +
      detail.shops.length +
      detail.coupons.length +
      detail.drops.length +
      detail.quests.length +
      detail.stories.length +
      detail.titles.length +
      detail.periodic.length +
      detail.goals.length ===
    0;
  return (
    <div className="space-y-3">
      <p className="flex flex-wrap gap-1 text-xs">
        {detail.uses.map((u) => (
          <span key={u.table} className={cn('rounded px-1.5 py-0.5', u.migrated ? 'bg-raised text-fg' : 'text-muted line-through')}>
            {u.table} {u.rows}
          </span>
        ))}
      </p>
      {empty && <p className="text-sm text-muted">Só itens ou falas do conteúdo; nada para listar aqui.</p>}
      {detail.npcs.length > 0 && (
        <section className="space-y-1">
          <h4 className="text-xs font-semibold tracking-wide uppercase">NPCs</h4>
          <p className="flex flex-wrap gap-1 text-sm">
            {detail.npcs.map((n) => (
              <span key={n.npc} className="rounded bg-raised px-1.5 py-0.5" title={n.npc}>
                {n.name ?? n.npc}
                {n.place && <span className="text-muted"> · {n.place}</span>}
              </span>
            ))}
          </p>
        </section>
      )}
      {detail.shops.length > 0 && (
        <Section title="Loja do evento" works={works(SECTION_TABLE.shops)}>
          {detail.shops.map((t, i) => (
            <ItemLine key={i} item={t.item} suffix={<Price trade={t} />} />
          ))}
        </Section>
      )}
      {detail.coupons.length > 0 && (
        <Section title="Loja de cupom (troca)" works={works(SECTION_TABLE.coupons)}>
          {detail.coupons.map((t, i) => (
            <ItemLine key={i} item={t.item} suffix={<Price trade={t} />} />
          ))}
        </Section>
      )}
      {detail.drops.length > 0 && (
        <Section title="Drops de chefe" works={works(SECTION_TABLE.drops)}>
          {detail.drops.map((d, i) => (
            <ItemLine
              key={i}
              item={d.item}
              suffix={
                <span className="shrink-0 text-xs text-muted">
                  {d.monster ?? 'todo chefe'} · peso {d.probability}
                  {d.condition ? ` · ${d.condition}` : ''}
                </span>
              }
            />
          ))}
        </Section>
      )}
      {detail.quests.length > 0 && (
        <Section title="Quests" works={works(SECTION_TABLE.quests)}>
          {detail.quests.map((q) => (
            <p key={q.id} className="truncate" title={q.id}>
              {q.title ?? q.id}
            </p>
          ))}
        </Section>
      )}
      {detail.stories.length > 0 && (
        <Section title="Histórias e recompensas" works={works(SECTION_TABLE.stories)}>
          {detail.stories.map((s) => (
            <div key={s.id} className="rounded border border-border px-2 py-1">
              <p className="truncate font-mono text-xs" title={s.id}>
                {s.id}
                {s.npc && <span className="font-sans text-muted"> · {s.npc.name ?? s.npc.npc}</span>}
              </p>
              <p className="text-xs text-muted">
                {[s.exp > 0 && `${s.exp} EXP`, s.gold > 0 && `${s.gold} ouro`, s.ap > 0 && `${s.ap} AP`, s.title > 0 && `título ${s.title}`]
                  .filter(Boolean)
                  .join(' · ') || 'sem recompensa fixa'}
              </p>
              {s.item && <ItemLine item={s.item} />}
            </div>
          ))}
        </Section>
      )}
      {detail.titles.length > 0 && (
        <Section title="Títulos" works={works(SECTION_TABLE.titles)}>
          <p className="text-xs text-muted">{detail.titles.join(', ')}</p>
        </Section>
      )}
      {detail.periodic.length > 0 && (
        <Section title="Recompensas periódicas (a cada 30 min online)" works={works(SECTION_TABLE.periodic)}>
          {detail.periodic.map((p, i) =>
            p.item && p.kind.toLowerCase() === 'item' ? (
              <ItemLine key={i} item={p.item} suffix={<span className="shrink-0 text-xs text-muted">peso {p.rate}</span>} />
            ) : (
              <p key={i} className="text-xs">
                {p.amount} {p.kind} <span className="text-muted">· peso {p.rate}</span>
              </p>
            ),
          )}
        </Section>
      )}
      {detail.goals.length > 0 && (
        <Section title="Metas coletivas" works={works(SECTION_TABLE.goals)}>
          {detail.goals.map((g, i) => (
            <div key={i} className="rounded border border-border px-2 py-1 text-xs">
              <p className="truncate font-mono" title={g.goal ?? ''}>
                {g.goal} ×{g.count}
              </p>
              <p className="text-muted">
                {g.start} → {g.end}
              </p>
              {g.reward && <ItemLine item={g.reward} />}
            </div>
          ))}
        </Section>
      )}
    </div>
  );
}

/** The "what this event brings" box: opens on hover with a mouse, on tap or click otherwise. */
export function EventDetail({ eventKey }: { eventKey: string }) {
  const [open, setOpen] = useState(false);
  const [pinned, setPinned] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  const timer = useRef<number | null>(null);
  const query = useQuery({ queryKey: ['tuning', 'event', eventKey], queryFn: () => eventDetail(eventKey), enabled: open, staleTime: 60_000 });
  useEffect(() => {
    if (!pinned) return;
    const close = (e: PointerEvent) => {
      if (box.current && !box.current.contains(e.target as Node)) {
        setPinned(false);
        setOpen(false);
      }
    };
    document.addEventListener('pointerdown', close);
    return () => document.removeEventListener('pointerdown', close);
  }, [pinned]);
  const hover = (on: boolean) => (e: React.PointerEvent) => {
    if (e.pointerType !== 'mouse' || pinned) return;
    if (timer.current) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setOpen(on), on ? 250 : 150);
  };
  return (
    <div ref={box} className="relative" onPointerEnter={hover(true)} onPointerLeave={hover(false)}>
      <button
        type="button"
        aria-label={`O que o evento ${eventKey} traz`}
        aria-expanded={open}
        className="rounded p-1 text-muted hover:bg-raised hover:text-fg"
        onClick={() => {
          setPinned(!pinned);
          setOpen(!pinned);
        }}
      >
        <Info className="size-4" />
      </button>
      {open && (
        <div
          role="dialog"
          aria-label={`Detalhes de ${eventKey}`}
          className="absolute top-full right-0 z-40 mt-1 max-h-[70vh] w-[min(28rem,calc(100vw-2rem))] overflow-y-auto rounded-lg border border-border bg-surface p-3 shadow-lg"
        >
          <p className="mb-2 truncate font-mono text-xs text-muted">{eventKey}</p>
          {query.isPending && <p className="text-sm text-muted">Carregando…</p>}
          {query.error && <p className="text-sm text-danger">Não foi possível carregar a descrição.</p>}
          {query.data && <Body detail={query.data} />}
        </div>
      )}
    </div>
  );
}
