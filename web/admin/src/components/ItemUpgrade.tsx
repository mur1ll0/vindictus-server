import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { ArrowRight } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { CharacterItem } from '@/api/characters';
import { api, type components, fail } from '@/api/client';
import { ActionForm, errorText } from '@/components/forms';
import { ItemTooltipBox } from '@/components/ItemTooltip';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

type Options = components['schemas']['UpgradeOptions'];
type EnchantOption = components['schemas']['EnchantOption'];
type Values = components['schemas']['UpgradeValues'];

/** EnchantLevel → the rank the client shows (GAMEUI_HEROES_SKILL_RANK_1…9). */
const RANK = ['', 'F', 'E', 'D', 'C', 'B', 'A', '9', '8', '7'];

export function rankLabel(level: number) {
  return RANK[level] ?? String(level);
}

async function upgradeOptions(cid: string, item: string): Promise<Options> {
  const { data, error, response } = await api.GET('/api/characters/{cid}/items/{item}/upgrade', { params: { path: { cid, item } } });
  if (!data) fail(response, error);
  return data;
}

async function preview(cid: string, item: string, body: Values) {
  const { data, error, response } = await api.POST('/api/characters/{cid}/items/{item}/upgrade/preview', {
    params: { path: { cid, item } },
    body,
  });
  if (!data) fail(response, error);
  return data;
}

async function upgrade(cid: string, item: string, body: components['schemas']['UpgradeChange']) {
  const { data, error, response } = await api.POST('/api/characters/{cid}/items/{item}/upgrade', {
    params: { path: { cid, item } },
    body,
  });
  if (!data) fail(response, error);
  return data;
}

type Choice = { cls: string; arg: string };

function EnchantField({
  id,
  label,
  options,
  value,
  onChange,
}: {
  id: string;
  label: string;
  options: EnchantOption[];
  value: Choice;
  onChange: (v: Choice) => void;
}) {
  const [filter, setFilter] = useState('');
  const term = filter.trim().toLowerCase();
  const shown = options.filter(
    (o) => o.class === value.cls || !term || o.name.toLowerCase().includes(term) || o.class.includes(term) || rankLabel(o.rank).toLowerCase() === term,
  );
  const picked = options.find((o) => o.class === value.cls);
  const ranks = [...new Set(shown.map((o) => o.rank))];
  return (
    <div className="space-y-1.5">
      <Label htmlFor={id}>{label}</Label>
      <div className="grid grid-cols-[1fr_8rem] gap-2">
        <select
          id={id}
          className="h-9 w-full rounded-md border border-border bg-surface px-2 text-sm"
          value={value.cls}
          onChange={(e) => {
            const next = options.find((o) => o.class === e.target.value);
            onChange({ cls: e.target.value, arg: String(next?.max_arg ?? 0) });
          }}
        >
          <option value="">Nenhum</option>
          {ranks.map((rank) => (
            <optgroup key={rank} label={`Rank ${rankLabel(rank)}`}>
              {shown
                .filter((o) => o.rank === rank)
                .map((o) => (
                  <option key={o.class} value={o.class}>
                    {o.name} — {o.lines.join('; ') || o.class}
                  </option>
                ))}
            </optgroup>
          ))}
        </select>
        <Input aria-label={`Filtrar ${label.toLowerCase()}`} placeholder="Filtrar ou rank" value={filter} onChange={(e) => setFilter(e.target.value)} />
      </div>
      {picked && picked.max_arg > picked.min_arg && (
        <div className="flex items-center gap-2 text-sm">
          <Label htmlFor={`${id}-arg`}>
            Valor ({picked.min_arg}–{picked.max_arg})
          </Label>
          <Input
            id={`${id}-arg`}
            className="w-24"
            inputMode="numeric"
            value={value.arg}
            onChange={(e) => onChange({ ...value, arg: e.target.value.replace(/[^0-9-]/g, '') })}
          />
        </div>
      )}
      {picked && <p className="text-xs text-muted">{picked.lines.join(' · ')}</p>}
    </div>
  );
}

function values(enhance: string, prefix: Choice, suffix: Choice): Values {
  const choice = (c: Choice) => (c.cls ? { class: c.cls, arg: Number(c.arg) || 0 } : null);
  return { enhance: Number(enhance) || 0, prefix: choice(prefix), suffix: choice(suffix) };
}

/**
 * Enhance level and prefix/suffix enchants of one equipment item, limited to what the game
 * could produce for it, with the tooltip before → after. The character must be offline.
 */
export function ItemUpgradeForm({ cid, item, onCancel, onDone }: { cid: string; item: CharacterItem; onCancel: () => void; onDone: () => void }) {
  const options = useQuery({ queryKey: ['item-upgrade', cid, item.id], queryFn: () => upgradeOptions(cid, item.id) });
  const [enhance, setEnhance] = useState('0');
  const [prefix, setPrefix] = useState<Choice>({ cls: '', arg: '0' });
  const [suffix, setSuffix] = useState<Choice>({ cls: '', arg: '0' });
  const data = options.data;
  useEffect(() => {
    if (!data) return;
    setEnhance(String(data.enhance));
    setPrefix({ cls: data.prefix ?? '', arg: String(data.prefix_arg) });
    setSuffix({ cls: data.suffix ?? '', arg: String(data.suffix_arg) });
  }, [data]);
  const body = values(enhance, prefix, suffix);
  const shown = useQuery({
    queryKey: ['item-upgrade-preview', cid, item.id, body],
    queryFn: () => preview(cid, item.id, body),
    enabled: !!data,
    placeholderData: keepPreviousData,
    retry: false,
  });

  if (options.error) return <p className="text-sm text-danger">{errorText(options.error)}</p>;
  if (!data) return <p className="text-sm text-muted">Carregando opções…</p>;
  return (
    <ActionForm
      submitLabel="Aplicar refino e scrolls"
      onCancel={onCancel}
      onDone={onDone}
      onSubmit={async (operation_id) => {
        await upgrade(cid, item.id, { operation_id, ...body });
      }}
    >
      <p className="text-sm text-muted">
        Só os valores que o jogo produziria para este item: níveis do tipo de refino e scrolls que aceitam a classe. Refino 9+ ou scroll de rank 7–9 deixam o item
        intransferível, como no jogo. O personagem precisa estar fora do jogo.
      </p>
      <div className="grid gap-3 sm:grid-cols-[8rem_1fr]">
        <div className="space-y-1.5">
          <Label htmlFor="upgrade-enhance">Refino (0–{data.enhance_max})</Label>
          <select
            id="upgrade-enhance"
            className="h-9 w-full rounded-md border border-border bg-surface px-2 text-sm"
            value={enhance}
            disabled={data.enhance_max === 0}
            onChange={(e) => setEnhance(e.target.value)}
          >
            {Array.from({ length: data.enhance_max + 1 }, (_, level) => (
              <option key={level} value={level}>
                {level === 0 ? 'sem refino' : `+${level}`}
              </option>
            ))}
          </select>
        </div>
        {!data.enchantable && <p className="self-end text-sm text-muted">Este item não aceita encantamento.</p>}
      </div>
      {data.enchantable && (
        <div className="grid gap-3 sm:grid-cols-2">
          <EnchantField id="upgrade-prefix" label="Prefixo" options={data.prefixes} value={prefix} onChange={setPrefix} />
          <EnchantField id="upgrade-suffix" label="Sufixo" options={data.suffixes} value={suffix} onChange={setSuffix} />
        </div>
      )}
      {shown.error && <p className="text-sm text-danger">{errorText(shown.error)}</p>}
      {shown.data && (
        <div className="flex flex-wrap items-start gap-3">
          <ItemTooltipBox item={{ ...item, tooltip: shown.data.before }} />
          <ArrowRight className="mt-16 size-5 text-muted" aria-label="depois" />
          <ItemTooltipBox item={{ ...item, tooltip: shown.data.after }} />
        </div>
      )}
    </ActionForm>
  );
}
