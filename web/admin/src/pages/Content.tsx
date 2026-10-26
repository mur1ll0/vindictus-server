import { useQuery, useQueryClient } from '@tanstack/react-query';
import { CheckCircle2, Clock, FileStack, TimerReset, Undo2, Upload } from 'lucide-react';
import { useEffect, useState } from 'react';
import { cancelMaintenance, type ContentEdit, contentState, KIND_LABEL, publish, withdrawEdit } from '@/api/content';
import { sessionQuery } from '@/api/session';
import { ActionForm, errorText } from '@/components/forms';
import { Modal } from '@/components/Modal';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

function clock(seconds: number) {
  const s = Math.max(0, seconds);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}

/** Seconds left, counted down locally from the last read of the server. */
function useCountdown(remaining: number | undefined, readAt: number) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (remaining == null) return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [remaining]);
  return remaining == null ? undefined : remaining - Math.floor((now - readAt) / 1000);
}

function describe(json?: string | null) {
  if (!json) return '—';
  try {
    return Object.entries(JSON.parse(json) as Record<string, unknown>)
      .map(([k, v]) => `${k}: ${v}`)
      .join(', ');
  } catch {
    return json;
  }
}

const STATE: Record<string, { label: string; tone: 'accent' | 'ok' | 'neutral' }> = {
  pending: { label: 'pendente', tone: 'accent' },
  published: { label: 'publicada', tone: 'ok' },
  withdrawn: { label: 'retirada', tone: 'neutral' },
};

export function ContentPage() {
  const queryClient = useQueryClient();
  const { data: session } = useQuery(sessionQuery);
  const isAdmin = session?.role === 'admin';
  const canEdit = isAdmin || session?.role === 'gm';
  const state = useQuery({
    queryKey: ['content'],
    queryFn: contentState,
    // While a restart is scheduled; the panel is down for a moment when the game restarts.
    refetchInterval: (q) => (q.state.data?.maintenance ? 5000 : false),
    retry: true,
  });
  const [action, setAction] = useState<'publish' | 'cancel' | ContentEdit | null>(null);
  const [minutes, setMinutes] = useState('5');
  const maintenance = state.data?.maintenance;
  const left = useCountdown(maintenance?.remaining_seconds, state.dataUpdatedAt);
  const pending = (state.data?.edits ?? []).filter((e) => e.state === 'pending');
  const others = (state.data?.edits ?? []).filter((e) => e.state !== 'pending');
  const latest = state.data?.versions[0];
  const done = () => {
    setAction(null);
    void queryClient.invalidateQueries({ queryKey: ['content'] });
    void queryClient.invalidateQueries({ queryKey: ['audit'] });
  };
  const card = (e: ContentEdit) => (
    <li key={e.id} className="flex gap-3 rounded-xl border border-border bg-surface p-4 shadow-sm">
      <div className="min-w-0 flex-1 space-y-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="font-medium">{KIND_LABEL[e.kind] ?? e.kind}</span>
          <Badge tone={STATE[e.state]?.tone ?? 'neutral'}>{STATE[e.state]?.label ?? e.state}</Badge>
          {e.version != null && <Badge>v{e.version}</Badge>}
          <span className="ml-auto text-xs text-muted">{new Date(e.created_at).toLocaleString('pt-BR')}</span>
        </div>
        <p className="font-mono text-sm">{e.target}</p>
        <p className="rounded-md bg-raised px-2 py-1 font-mono text-xs break-all">{describe(e.value)}</p>
        <p className="text-sm text-muted">
          {e.created_by} · {e.reason}
        </p>
      </div>
      {e.state === 'pending' && canEdit && (
        <Button variant="ghost" size="sm" onClick={() => setAction(e)} aria-label="Retirar edição">
          <Undo2 />
        </Button>
      )}
    </li>
  );
  return (
    <section className="space-y-5">
      <Card>
        <div className="flex flex-wrap items-center gap-4">
          <span className="flex size-10 items-center justify-center rounded-lg bg-accent-soft text-accent">
            <FileStack className="size-5" />
          </span>
          <div className="min-w-0 flex-1">
            <p className="font-semibold">{pending.length} edição(ões) pendente(s)</p>
            <p className="text-sm text-muted">
              {latest
                ? `Última versão v${latest.version} (${latest.edits} edição(ões)) ${latest.applied_at ? `aplicada em ${new Date(latest.applied_at).toLocaleString('pt-BR')}` : 'aplicada no próximo reinício'}.`
                : 'Nenhuma versão publicada.'}
            </p>
          </div>
          {isAdmin && (
            <Button disabled={pending.length === 0 || maintenance != null} onClick={() => setAction('publish')}>
              <Upload /> Publicar versão
            </Button>
          )}
        </div>
        <p className="mt-4 text-xs text-muted">
          Publicar agenda uma manutenção: todos no jogo recebem o aviso de desligamento do jogo original, são desconectados
          no fim do prazo e o servidor reinicia sozinho já com a versão. A cópia do cliente é atualizada ao abrir o jogo
          pelo <span className="font-mono">.cache/play-client/start-lab.bat</span>.
        </p>
      </Card>
      {maintenance && (
        <Card className="border-accent/60">
          <div className="flex flex-wrap items-center gap-4">
            <span className="flex size-10 items-center justify-center rounded-lg bg-accent-soft text-accent">
              <TimerReset className="size-5" />
            </span>
            <div className="min-w-0 flex-1">
              <p className="font-semibold">
                Manutenção agendada: reinício em {left != null && left > 0 ? clock(left) : 'instantes'}
              </p>
              <p className="text-sm text-muted">
                Aplica a v{maintenance.version} · agendada por {maintenance.requested_by} · aviso de{' '}
                {clock(maintenance.delay_seconds)} enviado a todos no jogo
              </p>
            </div>
            {isAdmin && (
              <Button variant="outline" onClick={() => setAction('cancel')}>
                Cancelar manutenção
              </Button>
            )}
          </div>
        </Card>
      )}
      {pending.length > 0 && (
        <div className="space-y-2">
          <h3 className="text-sm font-semibold text-muted">Pendentes</h3>
          <ul className="space-y-2">{pending.map(card)}</ul>
        </div>
      )}
      {(state.data?.versions ?? []).length > 0 && (
        <div className="space-y-2">
          <h3 className="text-sm font-semibold text-muted">Versões</h3>
          <ul className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
            {state.data?.versions.map((v) => (
              <li key={v.version} className="rounded-xl border border-border bg-surface p-4 shadow-sm">
                <div className="flex items-center gap-2">
                  <span className="font-semibold">v{v.version}</span>
                  {v.applied_at ? (
                    <Badge tone="ok">
                      <CheckCircle2 className="size-3" /> aplicada
                    </Badge>
                  ) : (
                    <Badge tone="accent">
                      <Clock className="size-3" /> a aplicar
                    </Badge>
                  )}
                  <span className="ml-auto text-xs text-muted">{v.edits} edição(ões)</span>
                </div>
                <p className="mt-1 text-sm text-muted">
                  {v.published_by} · {new Date(v.published_at).toLocaleString('pt-BR')}
                </p>
                <p className="text-sm">{v.reason}</p>
              </li>
            ))}
          </ul>
        </div>
      )}
      {others.length > 0 && (
        <div className="space-y-2">
          <h3 className="text-sm font-semibold text-muted">Histórico</h3>
          <ul className="space-y-2">{others.map(card)}</ul>
        </div>
      )}
      {state.error && <p className="text-sm text-danger">{errorText(state.error)}</p>}
      {action === 'publish' && (
        <Modal open title="Publicar versão" subtitle={`${pending.length} edição(ões) pendente(s) entram na próxima versão.`} onClose={() => setAction(null)}>
          <ActionForm
            submitLabel={Number(minutes) === 0 ? 'Publicar e reiniciar agora' : 'Publicar e agendar'}
            onCancel={() => setAction(null)}
            onDone={done}
            onSubmit={async (op, reason) => void (await publish(op, reason, Number(minutes)))}
          >
            <div className="space-y-1.5">
              <Label htmlFor="minutes">Minutos até reiniciar o servidor (0 = agora)</Label>
              <Input
                id="minutes"
                required
                inputMode="numeric"
                pattern="[0-9]{1,2}"
                value={minutes}
                onChange={(e) => setMinutes(e.target.value.replace(/[^0-9]/g, '').slice(0, 2))}
              />
              <p className="text-xs text-muted">
                Até 60. Todos no jogo recebem o aviso de manutenção com a contagem; no fim são desconectados (o progresso é
                salvo) e o servidor volta com a versão em cerca de um minuto.
              </p>
            </div>
          </ActionForm>
        </Modal>
      )}
      {action === 'cancel' && (
        <Modal open title="Cancelar manutenção" subtitle="A versão continua publicada e entra no próximo reinício." onClose={() => setAction(null)}>
          <ActionForm submitLabel="Cancelar manutenção" danger onCancel={() => setAction(null)} onDone={done} onSubmit={(op, reason) => cancelMaintenance(op, reason)} />
        </Modal>
      )}
      {action && action !== 'publish' && action !== 'cancel' && (
        <Modal open title="Retirar edição" subtitle={`${KIND_LABEL[action.kind] ?? action.kind} · ${action.target}`} onClose={() => setAction(null)}>
          <ActionForm submitLabel="Retirar" danger onCancel={() => setAction(null)} onDone={done} onSubmit={(op, reason) => withdrawEdit(action.id, op, reason)} />
        </Modal>
      )}
    </section>
  );
}
