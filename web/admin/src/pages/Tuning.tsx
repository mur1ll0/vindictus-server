import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { CheckCircle2, Clock, Save, Search, Undo2 } from 'lucide-react';
import { useState } from 'react';
import { api, type components, fail } from '@/api/client';
import { contentState } from '@/api/content';
import { sessionQuery } from '@/api/session';
import { errorText } from '@/components/forms';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { cn } from '@/lib/utils';

type Tuning = components['schemas']['TuningView'];
type EditRequest = components['schemas']['EditRequest'];
type Applied = components['schemas']['TuningApplied'];

async function tuning(): Promise<Tuning> {
  const { data, error, response } = await api.GET('/api/tuning');
  if (!data) fail(response, error);
  return data;
}

async function saveTuning(operation_id: string, edits: EditRequest[]): Promise<Applied> {
  const { data, error, response } = await api.POST('/api/tuning', { body: { operation_id, edits } });
  if (!data) fail(response, error);
  return data;
}

/** FeatureMatrix rates in percent (100 = the original value). */
const FEATURE_LABEL: Record<string, string> = {
  ExpRate_Combat: 'EXP das missões (combate)',
  GoldRate_Combat: 'Ouro das missões (combate)',
  APRate_Combat: 'AP das missões (combate)',
  ExpRate_Story: 'EXP das histórias',
  GoldRate_Story: 'Ouro das histórias',
  APRate_Story: 'AP das histórias',
  ExpRate_Monster: 'EXP por monstro',
};

/** One tunable value as the page edits it: one or two numbers in the field's unit. */
type Item = {
  key: string;
  title: string;
  detail: string;
  unit: string;
  labels: string[];
  max: number;
  step?: string;
  current: number[];
  fromPending: (value: Record<string, number>) => number[];
  build: (values: number[]) => EditRequest;
};

type Tab = 'rates' | 'drops' | 'enhance' | 'enchant';

const round1 = (n: number) => Math.round(n * 10) / 10;
const show = (values: number[], unit: string) => values.map((v) => `${v}${unit}`).join('–');

/** The page's items; `key` is "<edit kind>|<edit target>", as the content edits store it. */
function items(t: Tuning): Record<Tab, Item[]> {
  return {
    rates: t.features.map((f) => ({
      key: `feature|${f.key}`,
      title: FEATURE_LABEL[f.key] ?? f.key,
      detail: f.key,
      unit: '%',
      labels: ['Porcentagem'],
      max: 10000,
      current: [f.value],
      fromPending: (v) => [v.value ?? 0],
      build: ([value]) => ({ kind: 'feature', key: f.key, value: value ?? 0 }),
    })),
    drops: t.quests.map((q) => ({
      key: `quest_drop|${q.quest}`,
      title: q.title ?? q.quest,
      detail: q.quest,
      unit: '',
      labels: ['Mínimo', 'Máximo'],
      max: 100,
      current: [q.min, q.max],
      fromPending: (v) => [v.min ?? 0, v.max ?? 0],
      build: ([min, max]) => ({ kind: 'quest_drop', quest: q.quest, min: min ?? 0, max: max ?? 0 }),
    })),
    enhance: [...t.enhance]
      .sort((a, b) => a.level - b.level)
      .map((e) => ({
        key: `enhance_ratio|${e.enhance_type}#${e.level}`,
        title: `Enhance +${e.level}`,
        detail: `EnhanceServerInfo ${e.enhance_type}`,
        unit: '%',
        labels: ['Chance'],
        max: 100,
        step: '0.1',
        current: [round1(e.ratio * 100)],
        fromPending: (v) => [round1((v.ratio ?? 0) * 100)],
        build: ([value]) => ({ kind: 'enhance_ratio', enhance_type: e.enhance_type, level: e.level, ratio: (value ?? 0) / 100 }),
      })),
    enchant: t.enchants.map((e) => ({
      key: `enchant_ratio|${e.class}`,
      title: `${e.name ?? e.class} (${e.prefix ? 'prefixo' : 'sufixo'}, rank ${e.rank})`,
      detail: e.class,
      unit: '%',
      labels: ['Mínima', 'Máxima'],
      max: 100,
      current: [e.min, e.max],
      fromPending: (v) => [v.min ?? 0, v.max ?? 0],
      build: ([min, max]) => ({ kind: 'enchant_ratio', class: e.class, min: min ?? 0, max: max ?? 0 }),
    })),
  };
}

/** Why a draft cannot be saved, if it cannot. */
function invalid(item: Item, values: number[]) {
  if (values.some((v) => !Number.isFinite(v) || v < 0 || v > item.max)) return `${item.title}: use 0 a ${item.max}.`;
  if (values.length === 2 && (values[0] ?? 0) > (values[1] ?? 0)) return `${item.title}: o mínimo passa do máximo.`;
  return null;
}

function Row({
  item,
  draft,
  pending,
  editable,
  onChange,
}: {
  item: Item;
  draft?: string[];
  pending?: number[];
  editable: boolean;
  onChange: (values: string[]) => void;
}) {
  const values = draft ?? item.current.map(String);
  const changed = draft != null && draft.some((v, i) => Number(v) !== item.current[i]);
  return (
    <div className={cn('rounded-lg border border-border bg-surface px-3 py-2', changed && 'border-accent')}>
      <div className="flex items-center gap-3">
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium">{item.title}</p>
          <p className="truncate font-mono text-xs text-muted">{item.detail}</p>
        </div>
        <div className="flex items-center gap-1.5">
          {values.map((v, i) => (
            <div key={item.labels[i]} className="flex items-center gap-1">
              {i > 0 && <span className="text-muted">–</span>}
              <Input
                aria-label={`${item.title}: ${item.labels[i]}`}
                title={item.labels[i]}
                type="number"
                min={0}
                max={item.max}
                step={item.step ?? '1'}
                disabled={!editable}
                value={v}
                onChange={(e) => onChange(values.map((x, k) => (k === i ? e.target.value : x)))}
                className="h-8 w-20 text-right"
              />
              {item.unit && <span className="text-sm text-muted">{item.unit}</span>}
            </div>
          ))}
        </div>
      </div>
      {changed && (
        <p className="mt-1 text-xs">
          Atual {show(item.current, item.unit)} → novo <span className="font-semibold">{show(values.map(Number), item.unit)}</span>{' '}
          (clique em Salvar para aplicar)
        </p>
      )}
      {pending && (
        <p className="mt-1 flex items-center gap-1 text-xs text-accent">
          <Clock className="size-3.5 shrink-0" />
          <span>
            Pendente: {show(item.current, item.unit)} → <span className="font-semibold">{show(pending, item.unit)}</span>; publique a
            versão em Conteúdo para valer
          </span>
        </p>
      )}
    </div>
  );
}

export function TuningPage() {
  const queryClient = useQueryClient();
  const { data: session } = useQuery(sessionQuery);
  const editable = session?.role === 'admin';
  const data = useQuery({ queryKey: ['tuning'], queryFn: tuning });
  const content = useQuery({ queryKey: ['content'], queryFn: contentState });
  const [tab, setTab] = useState<Tab>('rates');
  const [filter, setFilter] = useState('');
  const [drafts, setDrafts] = useState<Record<string, string[]>>({});
  const [operationId, setOperationId] = useState(() => crypto.randomUUID());
  const [result, setResult] = useState<Applied | null>(null);
  const term = filter.trim().toLowerCase();
  const all = data.data ? items(data.data) : null;
  const byKey = new Map(Object.values(all ?? {}).flat().map((i) => [i.key, i]));

  // Tuning edits made through the pending cycle (or before live saving): not in the game yet.
  const pending = new Map<string, number[]>();
  for (const e of content.data?.edits ?? []) {
    const item = byKey.get(`${e.kind}|${e.target}`);
    if (e.state !== 'pending' || !item || !e.value || pending.has(item.key)) continue;
    try {
      pending.set(item.key, item.fromPending(JSON.parse(e.value) as Record<string, number>));
    } catch {
      // A value this page cannot read stays visible in Conteúdo.
    }
  }

  const changes = Object.entries(drafts).flatMap(([key, values]) => {
    const item = byKey.get(key);
    const numbers = values.map(Number);
    return item && numbers.some((v, i) => v !== item.current[i]) ? [{ item, numbers }] : [];
  });
  const problem = changes.map((c) => invalid(c.item, c.numbers)).find(Boolean) ?? null;

  const save = useMutation({
    mutationFn: () =>
      saveTuning(
        operationId,
        changes.map((c) => c.item.build(c.numbers)),
      ),
    onSuccess: (applied) => {
      setResult(applied);
      setDrafts({});
      setOperationId(crypto.randomUUID());
      void queryClient.invalidateQueries({ queryKey: ['tuning'] });
      void queryClient.invalidateQueries({ queryKey: ['content'] });
      void queryClient.invalidateQueries({ queryKey: ['audit'] });
    },
  });

  const tabs = [
    { id: 'rates', label: 'Taxas', n: all?.rates.length },
    { id: 'drops', label: 'Drops por fase', n: all?.drops.length },
    { id: 'enhance', label: 'Enhance', n: all?.enhance.length },
    { id: 'enchant', label: 'Enchant', n: all?.enchant.length },
  ] as const;
  const shown = (all?.[tab] ?? [])
    .filter((i) => tab === 'rates' || tab === 'enhance' || !term || `${i.title} ${i.detail}`.toLowerCase().includes(term))
    .slice(0, 300);

  return (
    <section className="space-y-4 pb-24">
      <Card>
        <p className="text-sm text-muted">
          Valores que o jogo está usando. Mude na própria caixa e clique em <span className="font-medium text-fg">Salvar</span>: o
          jogo aplica na hora, sem reiniciar. Cada salvamento vira uma versão já aplicada em Conteúdo e fica na auditoria.
          {!editable && ' Só um admin pode salvar.'}
        </p>
      </Card>
      {pending.size > 0 && (
        <Card className="border-accent">
          <p className="flex items-center gap-2 text-sm">
            <Clock className="size-4 shrink-0 text-accent" />
            <span>
              {pending.size} ajuste(s) pendente(s) ainda não valem no jogo. Publique a versão em{' '}
              <Link to="/conteudo" className="font-medium text-accent underline">
                Conteúdo
              </Link>{' '}
              (com reinício) ou salve o valor aqui para aplicar na hora.
            </span>
          </p>
        </Card>
      )}
      {result && (
        <Card className={result.live ? 'border-ok' : 'border-accent'}>
          <p className="flex items-start gap-2 text-sm">
            <CheckCircle2 className={cn('mt-0.5 size-4 shrink-0', result.live ? 'text-ok' : 'text-accent')} />
            <span>
              {result.live
                ? `Aplicado no jogo agora (versão ${result.version}), sem reiniciar.`
                : `Versão ${result.version} publicada, mas o jogo não recarregou: ela vale no próximo reinício (Conteúdo → manutenção).`}
              {result.client_refresh &&
                ' O servidor já usa as chances novas de Enhance/Enchant; as janelas do cliente mostram o valor novo depois de abri-lo pelo start-lab.bat.'}
            </span>
          </p>
        </Card>
      )}
      <div role="tablist" className="flex flex-wrap gap-1 rounded-lg bg-raised p-1 text-sm">
        {tabs.map((x) => (
          <button
            key={x.id}
            role="tab"
            type="button"
            aria-selected={tab === x.id}
            onClick={() => setTab(x.id)}
            className={cn('rounded-md px-3 py-1.5 text-muted', tab === x.id && 'bg-surface font-medium text-fg shadow-sm')}
          >
            {x.label} {x.n != null && <span className="text-xs">({x.n})</span>}
          </button>
        ))}
      </div>
      {tab !== 'rates' && tab !== 'enhance' && (
        <div className="relative max-w-sm">
          <Search className="pointer-events-none absolute top-2.5 left-3 size-4 text-muted" />
          <Input aria-label="Buscar" placeholder="Buscar (nome ou ID)" className="pl-9" value={filter} onChange={(e) => setFilter(e.target.value)} />
        </div>
      )}
      {data.error && <p className="text-sm text-danger">{errorText(data.error)}</p>}
      <div className={cn('grid gap-2', tab === 'enhance' ? 'sm:grid-cols-2 xl:grid-cols-3' : 'lg:grid-cols-2')}>
        {shown.map((item) => (
          <Row
            key={item.key}
            item={item}
            draft={drafts[item.key]}
            pending={pending.get(item.key)}
            editable={editable}
            onChange={(values) => setDrafts({ ...drafts, [item.key]: values })}
          />
        ))}
      </div>
      {changes.length > 0 && (
        <div className="fixed right-4 bottom-4 left-4 z-30 mx-auto flex max-w-3xl flex-wrap items-center gap-3 rounded-xl border border-accent bg-surface px-4 py-3 shadow-lg">
          <p className="flex-1 text-sm">
            <span className="font-medium">{changes.length} alteração(ões)</span>
            <span className="text-muted"> · o jogo aplica na hora, sem reiniciar</span>
          </p>
          <Button type="button" variant="outline" onClick={() => setDrafts({})} disabled={save.isPending}>
            <Undo2 className="size-4" /> Descartar
          </Button>
          <Button type="button" onClick={() => save.mutate()} disabled={save.isPending || problem != null}>
            <Save className="size-4" /> {save.isPending ? 'Aplicando…' : 'Salvar'}
          </Button>
          {(problem || save.error) && <p className="w-full text-sm text-danger">{problem ?? errorText(save.error)}</p>}
        </div>
      )}
    </section>
  );
}
