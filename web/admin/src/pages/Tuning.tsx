import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { CalendarHeart, CheckCircle2, Clock, Info, Save, Search, Store, Undo2 } from 'lucide-react';
import { useState } from 'react';
import { api, type components, fail } from '@/api/client';
import { contentState } from '@/api/content';
import { sessionQuery } from '@/api/session';
import { EventDetail } from '@/components/EventDetail';
import { errorText } from '@/components/forms';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { cn } from '@/lib/utils';

type Tuning = components['schemas']['TuningView'];
type EditRequest = components['schemas']['EditRequest'];
type Applied = components['schemas']['TuningApplied'];
type MarketAuto = components['schemas']['MarketAutoView'];
type EventView = components['schemas']['EventView'];

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

async function marketAuto(): Promise<MarketAuto> {
  const { data, error, response } = await api.GET('/api/tuning/market-auto');
  if (!data) fail(response, error);
  return data;
}

async function saveMarketAuto(body: components['schemas']['MarketAutoRequest']): Promise<MarketAuto> {
  const { data, error, response } = await api.POST('/api/tuning/market-auto', { body });
  if (!data) fail(response, error);
  return data;
}

/** The server's own marketplace listings (laboratory extension, not in the original game). */
function MarketAutoPanel({ editable }: { editable: boolean }) {
  const queryClient = useQueryClient();
  const data = useQuery({ queryKey: ['market-auto'], queryFn: marketAuto });
  const [draft, setDraft] = useState<{ enabled: boolean; interval_hours: string; items: string; days: string } | null>(null);
  const [operationId, setOperationId] = useState(() => crypto.randomUUID());
  const current = data.data;
  const form = draft ?? (current && { enabled: current.enabled, interval_hours: String(current.interval_hours), items: String(current.items), days: String(current.days) });
  const save = useMutation({
    mutationFn: () =>
      saveMarketAuto({
        operation_id: operationId,
        enabled: form?.enabled ?? false,
        interval_hours: Number(form?.interval_hours),
        items: Number(form?.items),
        days: Number(form?.days),
      }),
    onSuccess: (saved) => {
      queryClient.setQueryData(['market-auto'], saved);
      setDraft(null);
      setOperationId(crypto.randomUUID());
      void queryClient.invalidateQueries({ queryKey: ['audit'] });
    },
  });
  if (data.error) return <p className="text-sm text-danger">{errorText(data.error)}</p>;
  if (!form || !current) return <p className="text-sm text-muted">Carregando…</p>;
  const fields = [
    { key: 'interval_hours', label: 'Intervalo (horas)', min: 1, max: 168 },
    { key: 'items', label: 'Itens por vez', min: 1, max: 1000 },
    { key: 'days', label: 'Dias no mercado', min: 1, max: 30 },
  ] as const;
  const problem = fields.map((f) => {
    const v = Number(form[f.key]);
    return Number.isInteger(v) && v >= f.min && v <= f.max ? null : `${f.label}: use ${f.min} a ${f.max}.`;
  }).find(Boolean) ?? null;
  const last = current.last_run_unix != null ? new Date(current.last_run_unix * 1000).toLocaleString('pt-BR') : null;
  return (
    <Card className="space-y-4">
      <div className="flex items-start gap-3">
        <Store className="mt-0.5 size-5 shrink-0 text-accent" />
        <p className="text-sm text-muted">
          O próprio servidor publica itens aleatórios no mercado, periodicamente, pelo preço médio das vendas (ou o preço padrão do
          item), como o vendedor <span className="font-mono text-fg">Marketplace</span>. Os jogadores compram normalmente; o ouro pago
          não vai para ninguém e os itens que vencem somem. É um recurso do laboratório, que não existe no jogo original. Vale no
          próximo minuto, sem reiniciar.
        </p>
      </div>
      <label className="flex items-center gap-2 text-sm font-medium">
        <input type="checkbox" checked={form.enabled} disabled={!editable} onChange={(e) => setDraft({ ...form, enabled: e.target.checked })} />
        Gerar itens automaticamente no mercado
      </label>
      <div className="grid gap-3 sm:grid-cols-3">
        {fields.map((f) => (
          <label key={f.key} className="space-y-1 text-sm">
            <span className="text-muted">{f.label}</span>
            <Input
              type="number"
              min={f.min}
              max={f.max}
              disabled={!editable}
              value={form[f.key]}
              onChange={(e) => setDraft({ ...form, [f.key]: e.target.value })}
            />
          </label>
        ))}
      </div>
      <p className="text-xs text-muted">
        {current.candidates != null ? `${current.candidates} itens do conteúdo podem ser publicados. ` : ''}
        {last ? `Última geração: ${last} (${current.last_listed} itens).` : 'Ainda não gerou.'}
      </p>
      {editable && (
        <div className="flex flex-wrap items-center gap-3">
          <Button type="button" onClick={() => save.mutate()} disabled={draft == null || save.isPending || problem != null}>
            <Save className="size-4" /> {save.isPending ? 'Salvando…' : 'Salvar'}
          </Button>
          {draft != null && (
            <Button type="button" variant="outline" onClick={() => setDraft(null)} disabled={save.isPending}>
              <Undo2 className="size-4" /> Descartar
            </Button>
          )}
          {(problem || save.error) && <p className="w-full text-sm text-danger">{problem ?? errorText(save.error)}</p>}
        </div>
      )}
    </Card>
  );
}

/** Words of the event keys, for the filter chips. */
const EVENT_THEMES: { label: string; words: string[] }[] = [
  { label: 'Natal', words: ['christmas', 'xmas', 'santa'] },
  { label: 'Páscoa', words: ['easter'] },
  { label: 'Halloween', words: ['halloween'] },
  { label: 'SAO (collab)', words: ['sao'] },
  { label: 'Ano novo', words: ['newyear', 'new_year', 'lunar'] },
  { label: 'Verão', words: ['summer', 'beach'] },
  { label: 'Dia dos namorados', words: ['valentine', 'whiteday'] },
];
const theme = (key: string) => {
  const k = key.toLowerCase();
  return EVENT_THEMES.find((t) => t.words.some((w) => k.includes(w)))?.label ?? null;
};

/** FeatureMatrix event switches (laboratory extension): saved as tuning, live on the server. */
function EventsPanel({
  events,
  editable,
  onSaved,
}: {
  events: EventView[];
  editable: boolean;
  onSaved: (applied: Applied) => void;
}) {
  const queryClient = useQueryClient();
  const [drafts, setDrafts] = useState<Record<string, boolean>>({});
  const [filter, setFilter] = useState('');
  const [chip, setChip] = useState<string | null>(null);
  const [onlyOn, setOnlyOn] = useState(false);
  const [operationId, setOperationId] = useState(() => crypto.randomUUID());
  const term = filter.trim().toLowerCase();
  const changes = events.filter((e) => drafts[e.key] != null && drafts[e.key] !== e.enabled);
  const save = useMutation({
    mutationFn: () =>
      saveTuning(
        operationId,
        changes.map((e) => ({ kind: 'event', key: e.key, enabled: drafts[e.key] ?? e.enabled })),
      ),
    onSuccess: (applied) => {
      onSaved(applied);
      setDrafts({});
      setOperationId(crypto.randomUUID());
      void queryClient.invalidateQueries({ queryKey: ['tuning'] });
      void queryClient.invalidateQueries({ queryKey: ['content'] });
      void queryClient.invalidateQueries({ queryKey: ['audit'] });
    },
  });
  const shown = events
    .filter((e) => !term || e.key.toLowerCase().includes(term))
    .filter((e) => !chip || theme(e.key) === chip)
    .filter((e) => !onlyOn || (drafts[e.key] ?? e.enabled));
  return (
    <div className="space-y-3">
      <Card>
        <p className="flex items-start gap-2 text-sm text-muted">
          <CalendarHeart className="mt-0.5 size-4 shrink-0 text-accent" />
          <span>
            Chaves de evento do FeatureMatrix da nossa região (as de outras regiões ficam de fora), mais as chaves gerais BoostEvent
            (recompensas periódicas) e AllUserGoalEvent (metas coletivas, cujas datas passam a começar quando a chave da meta é
            ligada). Ligar vale na hora para lojas, lojas de cupom, drops de chefe, recompensas periódicas, metas coletivas,
            histórias, quests e itens do servidor, e o cliente recebe a chave no próximo login. As
            tabelas riscadas ainda não fazem efeito. Passe o mouse (ou toque) no <Info className="inline size-3.5" /> para ver o que
            cada evento traz. É um recurso do laboratório.
          </span>
        </p>
      </Card>
      <div className="flex flex-wrap items-center gap-2">
        <div className="relative max-w-sm flex-1">
          <Search className="pointer-events-none absolute top-2.5 left-3 size-4 text-muted" />
          <Input aria-label="Buscar evento" placeholder="Buscar evento" className="pl-9" value={filter} onChange={(e) => setFilter(e.target.value)} />
        </div>
        {EVENT_THEMES.map((t) => (
          <button
            key={t.label}
            type="button"
            onClick={() => setChip(chip === t.label ? null : t.label)}
            className={cn('rounded-full border border-border px-3 py-1 text-xs', chip === t.label && 'border-accent bg-raised font-medium')}
          >
            {t.label}
          </button>
        ))}
        <label className="flex items-center gap-1 text-xs">
          <input type="checkbox" checked={onlyOn} onChange={(e) => setOnlyOn(e.target.checked)} /> Só ligados
        </label>
      </div>
      <div className="grid gap-2 lg:grid-cols-2">
        {shown.slice(0, 300).map((e) => {
          const on = drafts[e.key] ?? e.enabled;
          const changed = drafts[e.key] != null && drafts[e.key] !== e.enabled;
          return (
            <div key={e.key} className={cn('rounded-lg border border-border bg-surface px-3 py-2', changed && 'border-accent')}>
              <div className="flex items-center gap-2">
                <label className="flex min-w-0 flex-1 items-center gap-3">
                  <input
                    type="checkbox"
                    aria-label={e.key}
                    checked={on}
                    disabled={!editable}
                    onChange={(x) => setDrafts({ ...drafts, [e.key]: x.target.checked })}
                  />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate font-mono text-sm">{e.key}</span>
                    <span className="block text-xs text-muted">
                      {theme(e.key) ? `${theme(e.key)} · ` : ''}original {e.original ? 'ligado' : 'desligado'}
                    </span>
                  </span>
                </label>
                <EventDetail eventKey={e.key} />
              </div>
              <p className="mt-1 flex flex-wrap gap-1 text-xs">
                {e.uses.map((u) => (
                  <span
                    key={u.table}
                    title={u.migrated ? 'O servidor já lê esta tabela' : 'Ainda não migrado: sem efeito por enquanto'}
                    className={cn('rounded px-1.5 py-0.5', u.migrated ? 'bg-raised text-fg' : 'text-muted line-through')}
                  >
                    {u.table} {u.rows}
                  </span>
                ))}
              </p>
            </div>
          );
        })}
      </div>
      {shown.length > 300 && <p className="text-xs text-muted">Mostrando 300 de {shown.length}; refine a busca.</p>}
      {changes.length > 0 && (
        <div className="fixed right-4 bottom-4 left-4 z-30 mx-auto flex max-w-3xl flex-wrap items-center gap-3 rounded-xl border border-accent bg-surface px-4 py-3 shadow-lg">
          <p className="flex-1 text-sm">
            <span className="font-medium">{changes.length} evento(s) alterado(s)</span>
            <span className="text-muted"> · o servidor aplica na hora; o cliente no próximo login</span>
          </p>
          <Button type="button" variant="outline" onClick={() => setDrafts({})} disabled={save.isPending}>
            <Undo2 className="size-4" /> Descartar
          </Button>
          <Button type="button" onClick={() => save.mutate()} disabled={save.isPending}>
            <Save className="size-4" /> {save.isPending ? 'Aplicando…' : 'Salvar'}
          </Button>
          {save.error && <p className="w-full text-sm text-danger">{errorText(save.error)}</p>}
        </div>
      )}
    </div>
  );
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

type Tab = 'rates' | 'drops' | 'enhance' | 'enchant' | 'market' | 'events';

const round1 = (n: number) => Math.round(n * 10) / 10;
const show = (values: number[], unit: string) => values.map((v) => `${v}${unit}`).join('–');

/** The page's items; `key` is "<edit kind>|<edit target>", as the content edits store it. */
function items(t: Tuning): Record<Exclude<Tab, 'market' | 'events'>, Item[]> {
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
    { id: 'market', label: 'Mercado automático', n: undefined },
    { id: 'events', label: 'Eventos', n: data.data?.events.length },
  ] as const;
  const shown = (tab === 'market' || tab === 'events' ? [] : (all?.[tab] ?? []))
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
              {result.relog && ' Os eventos já valem no servidor; o cliente recebe as chaves no próximo login.'}
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
      {tab === 'market' && <MarketAutoPanel editable={editable} />}
      {tab === 'events' && data.data && <EventsPanel events={data.data.events} editable={editable} onSaved={setResult} />}
      {tab !== 'rates' && tab !== 'enhance' && tab !== 'market' && tab !== 'events' && (
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
