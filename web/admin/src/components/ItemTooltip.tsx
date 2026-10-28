import { type ReactNode, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import type { CharacterItem } from '@/api/characters';
import { ItemIcon } from '@/components/ItemIcon';
import { cn } from '@/lib/utils';

/** Rarity text colors of the tooltip (panel approximation of ItemToolTip.gfx). */
const RARITY_COLOR: Record<number, string> = { 1: '#e5e5e5', 2: '#aaed7d', 3: '#7cc4ff', 4: '#d58cff', 5: '#ff9b3d' };

const hex = (c: number) => `#${c.toString(16).padStart(6, '0')}`;

export function itemColors(item: CharacterItem) {
  return item.tooltip.dye_mask ? (item.tooltip.colors ?? null) : null;
}

/** The item name as the game composes it: +enhance, prefix, name, suffix. */
export function itemTitle(item: CharacterItem) {
  const t = item.tooltip;
  const prefix = t.enchants.find((e) => e.prefix)?.name;
  const suffix = t.enchants.find((e) => !e.prefix)?.name;
  return [t.enhance_level > 0 ? `+${t.enhance_level}` : null, prefix, item.name ?? item.class, suffix].filter(Boolean).join(' ');
}

function Section({ children }: { children: ReactNode }) {
  return <div className="space-y-0.5 border-t border-white/10 pt-2">{children}</div>;
}

/** Durability in the units the client shows (game value / 100, rounded up). */
function durability(value: number) {
  return Math.ceil(value / 100);
}

export function ItemTooltipBox({ item }: { item: CharacterItem }) {
  const t = item.tooltip;
  const color = RARITY_COLOR[t.rarity ?? 1] ?? RARITY_COLOR[1];
  const [current = 0, maximum = 0] = t.durability ?? [];
  return (
    <div className="w-80 space-y-2 rounded-lg border border-[#5a4a2a] bg-[#14110c]/95 p-3 text-[12px] leading-snug text-[#cccccc] shadow-2xl">
      <div className="flex gap-3">
        <div className="rounded-md border border-[#5a4a2a] bg-black/40 p-0.5">
          <ItemIcon icon={item.icon} bg={item.icon_bg} colors={itemColors(item)} size="lg" />
        </div>
        <div className="min-w-0 flex-1">
          <p className="text-[14px] font-semibold" style={{ color }}>
            {itemTitle(item)}
          </p>
          <p className="text-[#acacac]">
            {[t.category, t.rarity_name].filter(Boolean).join(' · ')}
            {item.count > 1 && ` · ×${item.count.toLocaleString('pt-BR')}`}
          </p>
          {t.unique && <p className="text-[#eed16a]">Unique Item</p>}
          {item.equipped_part != null && <p className="text-[#aaed7d]">(Equipping)</p>}
        </div>
      </div>
      {t.stats.length > 0 && (
        <Section>
          <div className="grid grid-cols-2 gap-x-4">
            {t.stats.map((s) => (
              <p key={s.key} className="flex justify-between">
                <span className="text-[#acacac]">{s.label}</span>
                <span className="font-semibold text-white">{s.value.toLocaleString('pt-BR')}</span>
              </p>
            ))}
          </div>
        </Section>
      )}
      {((t.quality ?? 0) > 0 || t.enchants.length > 0) && (
        <Section>
          {(t.quality ?? 0) > 0 && <p className="text-[#eed16a]">{'★'.repeat(Math.min(5, t.quality ?? 0))} Quality</p>}
          {t.enchants.map((e) => (
            <div key={e.prefix ? 'p' : 's'}>
              <p className="text-[#ffe788]">
                {e.prefix ? 'Prefix' : 'Suffix'}: {e.name}
                {e.rank != null && <span className="text-[#acacac]"> (Rank {e.rank})</span>}
              </p>
              {e.lines.map((l) => (
                <p key={l.text} className={cn('pl-3', l.ok ? 'text-[#aaed7d]' : 'text-[#777]')}>
                  {l.text}
                </p>
              ))}
            </div>
          ))}
        </Section>
      )}
      {t.durability && (
        <Section>
          <div className="flex items-center gap-2">
            <span className="text-[#acacac]">Durability</span>
            <div className="h-1.5 flex-1 overflow-hidden rounded bg-white/10">
              <div className="h-full bg-[#c4a157]" style={{ width: `${(100 * current) / Math.max(1, maximum)}%` }} />
            </div>
            <span className="text-white">
              {durability(current)}/{durability(maximum)}
            </span>
          </div>
        </Section>
      )}
      {t.constraints.length > 0 && (
        <Section>
          {t.constraints.map((c) => (
            <p key={c.text} className={c.ok ? 'text-[#cccccc]' : 'text-[#c23a30]'}>
              {c.text}
            </p>
          ))}
        </Section>
      )}
      {item.description && (
        <Section>
          <p className="whitespace-pre-line text-[#acacac]">{item.description}</p>
        </Section>
      )}
      {t.set_name && (
        <Section>
          <p className="text-[#aaed7d]">{t.set_name}</p>
          {t.set_bonuses.map((b) => (
            <p key={b.text} className={b.ok ? 'text-[#aaed7d]' : 'text-[#777]'}>
              {b.text}
            </p>
          ))}
        </Section>
      )}
      <Section>
        {t.colors && t.dyeable && (
          <p className="flex items-center gap-2 text-[#acacac]">
            {t.colors.map((c, i) => (
              <span key={i} className="flex items-center gap-1">
                Color {i + 1}
                <span className="inline-block size-3 rounded-sm border border-white/30" style={{ background: hex(c) }} />
              </span>
            ))}
          </p>
        )}
        {item.expires_at != null && (
          <p className="text-[#ffff00]">Expires: {new Date(item.expires_at * 1000).toLocaleString('pt-BR')}</p>
        )}
        {t.trade.map((x) => (
          <p key={x} className="text-[#c25631]">
            {x}
          </p>
        ))}
        <div className="flex justify-between text-[#acacac]">
          {t.weight != null ? <span>{t.weight}</span> : <span />}
          {t.sell_price != null && (
            <span>
              Sell Price <span className="text-[#eed16a]">{t.sell_price.toLocaleString('pt-BR')}</span>
            </span>
          )}
        </div>
      </Section>
    </div>
  );
}

/** Shows the item tooltip next to the pointer while hovering `children`. */
export function WithItemTooltip({ item, children, className }: { item?: CharacterItem; children: ReactNode; className?: string }) {
  const [at, setAt] = useState<{ x: number; y: number } | null>(null);
  // Inside a modal <dialog> (the top layer) the box must be drawn in the dialog itself.
  const [host, setHost] = useState<Element>(document.body);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);
  const box = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (!at || !box.current) return;
    const r = box.current.getBoundingClientRect();
    const left = at.x + 16 + r.width > window.innerWidth ? Math.max(8, at.x - 16 - r.width) : at.x + 16;
    const top = Math.max(8, Math.min(at.y + 12, window.innerHeight - r.height - 8));
    setPos({ left, top });
  }, [at]);
  return (
    <div
      className={className}
      onMouseMove={(e) => {
        if (!item) return;
        const dialog = e.currentTarget.closest('dialog') ?? document.body;
        if (dialog !== host) setHost(dialog);
        setAt({ x: e.clientX, y: e.clientY });
      }}
      onMouseLeave={() => {
        setAt(null);
        setPos(null);
      }}
      onDragStart={() => setAt(null)}
    >
      {children}
      {item &&
        at &&
        createPortal(
          <div
            ref={box}
            className="pointer-events-none fixed z-50"
            style={{ left: pos?.left ?? at.x + 16, top: pos?.top ?? at.y + 12, visibility: pos ? 'visible' : 'hidden' }}
          >
            <ItemTooltipBox item={item} />
          </div>,
          host,
        )}
    </div>
  );
}
