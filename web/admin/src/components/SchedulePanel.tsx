import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { CalendarClock, Play, Plus, Trash2 } from 'lucide-react';
import { useState } from 'react';
import { api, type components, fail } from '@/api/client';
import { errorText } from '@/components/forms';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { cn } from '@/lib/utils';

type Schedule = components['schemas']['ScheduleView'];
type Request = components['schemas']['ScheduleRequest'];

async function schedule(): Promise<Schedule> {
  const { data, error, response } = await api.GET('/api/tuning/schedule');
  if (!data) fail(response, error);
  return data;
}
async function register(body: Request): Promise<Schedule> {
  const { data, error, response } = await api.POST('/api/tuning/schedule', { body });
  if (!data) fail(response, error);
  return data;
}
async function remove(name: string): Promise<Schedule> {
  const { data, error, response } = await api.DELETE('/api/tuning/schedule/{name}', {
    params: { path: { name }, query: { operation_id: crypto.randomUUID() } },
  });
  if (!data) fail(response, error);
  return data;
}

/** datetime-local (browser time) ⇄ UTC Unix seconds. */
const toSeconds = (v: string) => (v ? Math.floor(new Date(v).getTime() / 1000) : null);
const fromSeconds = (s: number | null | undefined) => {
  if (s == null) return '—';
  return new Date(s * 1000).toLocaleString('pt-BR');
};
/** HH:MM of the server's day ⇄ seconds. */
const toDaySeconds = (v: string) => {
  if (!v) return null;
  const [h = 0, m = 0] = v.split(':').map(Number);
  return h * 3600 + m * 60;
};
const fromDaySeconds = (s: number | null | undefined) =>
  s == null ? '' : `${String(Math.floor(s / 3600)).padStart(2, '0')}:${String(Math.floor((s % 3600) / 60)).padStart(2, '0')}`;

const empty = {
  name: '',
  template: '',
  feature: '',
  start_script: '',
  end_script: '',
  start: '',
  end: '',
  period_begin: '',
  period_end: '',
  start_message: '',
  notify_message: '',
  end_message: '',
  notify_interval: '',
};

/** The event schedule (laboratory extension of the AdminClient events, B5 eventos). */
export function SchedulePanel({ editable }: { editable: boolean }) {
  const queryClient = useQueryClient();
  const data = useQuery({ queryKey: ['tuning', 'schedule'], queryFn: schedule, refetchInterval: 30_000 });
  const [form, setForm] = useState(empty);
  const [search, setSearch] = useState('');
  const set = (k: keyof typeof empty) => (e: { target: { value: string } }) => setForm({ ...form, [k]: e.target.value });
  const template = data.data?.templates.find((t) => t.name === form.template);
  const done = (view: Schedule) => queryClient.setQueryData(['tuning', 'schedule'], view);
  const save = useMutation({
    mutationFn: () =>
      register({
        operation_id: crypto.randomUUID(),
        name: form.name.trim(),
        template: form.template || null,
        feature: form.feature || null,
        start_script: form.start_script || null,
        end_script: form.end_script || null,
        start_at: toSeconds(form.start),
        end_at: toSeconds(form.end),
        period_begin: toDaySeconds(form.period_begin),
        period_end: toDaySeconds(form.period_end),
        start_message: form.start_message || null,
        notify_message: form.notify_message || null,
        end_message: form.end_message || null,
        notify_interval: form.notify_interval ? Number(form.notify_interval) * 60 : null,
      }),
    onSuccess: (view) => {
      done(view);
      setForm(empty);
      void queryClient.invalidateQueries({ queryKey: ['audit'] });
    },
  });
  const drop = useMutation({ mutationFn: remove, onSuccess: done });
  const templates = (data.data?.templates ?? []).filter((t) => !search || t.name.toLowerCase().includes(search.toLowerCase()));
  const offsetHours = (data.data?.offset_seconds ?? 0) / 3600;
  return (
    <div className="space-y-3">
      <Card>
        <p className="flex items-start gap-2 text-sm text-muted">
          <CalendarClock className="mt-0.5 size-4 shrink-0 text-accent" />
          <span>
            Agenda de eventos (como o AdminClient do original): no horário, o servidor liga as chaves do modelo (<code>Chave</code> ou{' '}
            <code>Chave[valor]</code>), roda <code>droptable mod/reset</code> e recarrega o conteúdo; no fim, desfaz. As mensagens
            de início, aviso e fim vão para todos os jogadores. O período diário usa o horário do servidor (UTC
            {offsetHours >= 0 ? '+' : ''}
            {offsetHours}). A verificação roda a cada minuto. É um recurso do laboratório.
          </span>
        </p>
      </Card>
      {data.data && (Object.keys(data.data.overrides).length > 0 || Object.keys(data.data.drop_modifiers).length > 0) && (
        <Card>
          <p className="mb-1 text-sm font-medium">Em vigor agora</p>
          <p className="flex flex-wrap gap-1 text-xs">
            {Object.entries(data.data.overrides).map(([k, v]) => (
              <span key={k} className="rounded bg-raised px-1.5 py-0.5 font-mono">
                {k}={v}
              </span>
            ))}
            {Object.entries(data.data.drop_modifiers).map(([k, v]) => (
              <span key={k} className="rounded bg-accent-soft px-1.5 py-0.5 font-mono">
                drop {k} {v}%
              </span>
            ))}
          </p>
        </Card>
      )}
      <div className="grid gap-2 lg:grid-cols-2">
        {(data.data?.events ?? []).map((e) => (
          <div key={e.name} className={cn('rounded-lg border border-border bg-surface px-3 py-2', e.running && 'border-accent')}>
            <div className="flex items-center gap-2">
              {e.running && <Play className="size-4 text-ok" />}
              <span className="min-w-0 flex-1 truncate font-mono text-sm" title={e.name}>
                {e.name}
              </span>
              <span className={cn('text-xs', e.running ? 'text-ok' : 'text-muted')}>{e.running ? 'Rodando' : 'Aguardando'}</span>
              {editable && (
                <button
                  type="button"
                  aria-label={`Remover ${e.name}`}
                  className="rounded p-1 text-muted hover:bg-raised hover:text-danger"
                  disabled={drop.isPending}
                  onClick={() => drop.mutate(e.name)}
                >
                  <Trash2 className="size-4" />
                </button>
              )}
            </div>
            <p className="text-xs text-muted">
              {fromSeconds(e.start_at)} → {fromSeconds(e.end_at)}
              {e.period_begin != null && ` · diário ${fromDaySeconds(e.period_begin)}–${fromDaySeconds(e.period_end) || '24:00'}`}
              {e.template && ` · modelo ${e.template}`}
            </p>
            {e.feature && <p className="truncate font-mono text-xs" title={e.feature}>{e.feature}</p>}
            {e.unsupported.length > 0 && (
              <p className="text-xs text-danger" title={e.unsupported.join('; ')}>
                Sem efeito no laboratório: {e.unsupported.join('; ')}
              </p>
            )}
          </div>
        ))}
        {data.data?.events.length === 0 && <p className="text-sm text-muted">Nenhum evento agendado.</p>}
      </div>
      {editable && (
        <Card>
          <p className="mb-2 flex items-center gap-2 text-sm font-medium">
            <Plus className="size-4" /> Agendar evento
          </p>
          <div className="grid gap-2 sm:grid-cols-2">
            <label className="text-xs">
              Nome
              <Input value={form.name} onChange={set('name')} placeholder="ex.: natal_2026" />
            </label>
            <label className="text-xs">
              Modelo ({templates.length})
              <Input value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Buscar modelo" />
              <select className="mt-1 w-full rounded-md border border-border bg-surface px-2 py-1 text-sm" value={form.template} onChange={set('template')}>
                <option value="">Sem modelo</option>
                {templates.slice(0, 400).map((t) => (
                  <option key={t.name} value={t.name}>
                    {t.name}
                  </option>
                ))}
              </select>
            </label>
            {template && (
              <p className="text-xs text-muted sm:col-span-2">
                Chaves: <code>{template.feature || '—'}</code> · início: <code>{template.start_script || '—'}</code> · fim:{' '}
                <code>{template.end_script || '—'}</code>
              </p>
            )}
            <label className="text-xs sm:col-span-2">
              Chaves (substitui as do modelo)
              <Input value={form.feature} onChange={set('feature')} placeholder="ExpRate_Combat[200];Event_xmas_2012" />
            </label>
            <label className="text-xs">
              Script de início
              <Input value={form.start_script} onChange={set('start_script')} placeholder="droptable mod gold_ore 300;" />
            </label>
            <label className="text-xs">
              Script de fim
              <Input value={form.end_script} onChange={set('end_script')} placeholder="droptable reset gold_ore;" />
            </label>
            <label className="text-xs">
              Início
              <Input type="datetime-local" value={form.start} onChange={set('start')} />
            </label>
            <label className="text-xs">
              Fim
              <Input type="datetime-local" value={form.end} onChange={set('end')} />
            </label>
            <label className="text-xs">
              Período diário: das (horário do servidor)
              <Input type="time" value={form.period_begin} onChange={set('period_begin')} />
            </label>
            <label className="text-xs">
              até
              <Input type="time" value={form.period_end} onChange={set('period_end')} />
            </label>
            <label className="text-xs">
              Mensagem de início
              <Input value={form.start_message} onChange={set('start_message')} />
            </label>
            <label className="text-xs">
              Mensagem de fim
              <Input value={form.end_message} onChange={set('end_message')} />
            </label>
            <label className="text-xs">
              Aviso enquanto roda (também ao entrar no jogo)
              <Input value={form.notify_message} onChange={set('notify_message')} />
            </label>
            <label className="text-xs">
              Aviso a cada (minutos, padrão 30)
              <Input type="number" min={0} value={form.notify_interval} onChange={set('notify_interval')} />
            </label>
          </div>
          <div className="mt-3 flex flex-wrap items-center gap-3">
            <Button type="button" onClick={() => save.mutate()} disabled={!form.name.trim() || save.isPending}>
              <CalendarClock className="size-4" /> {save.isPending ? 'Agendando…' : 'Agendar'}
            </Button>
            {(save.error || drop.error) && <p className="text-sm text-danger">{errorText(save.error ?? drop.error)}</p>}
          </div>
        </Card>
      )}
    </div>
  );
}
