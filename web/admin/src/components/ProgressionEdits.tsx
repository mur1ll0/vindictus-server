import { useQuery } from '@tanstack/react-query';
import { Search, TrendingUp } from 'lucide-react';
import { useState } from 'react';
import { api, type components, fail } from '@/api/client';
import { ActionForm } from '@/components/forms';
import { Modal } from '@/components/Modal';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { cn } from '@/lib/utils';

type Kind = components['schemas']['ProgressionKind'];
type CatalogEffect = components['schemas']['CatalogEffect'];
type Mode = 'reach' | 'level' | 'exp' | 'ap' | 'title' | 'effect';
type ReachPlan = components['schemas']['ReachPlan'];

const MODES: { id: Mode; label: string }[] = [
  { id: 'reach', label: 'Até o nível' },
  { id: 'level', label: 'Nível' },
  { id: 'exp', label: 'EXP' },
  { id: 'ap', label: 'AP' },
  { id: 'title', label: 'Título' },
  { id: 'effect', label: 'Efeito' },
];

const OFFLINE_NOTE =
  'A personagem precisa estar fora do jogo. Segue as regras do jogo: subir de nível dá os títulos e abre as histórias do nível, que aparecem na próxima seleção.';

export async function changeProgression(cid: string, operation_id: string, change: Kind) {
  const { data, error, response } = await api.POST('/api/characters/{cid}/progression', {
    params: { path: { cid } },
    body: { operation_id, change },
  });
  if (!data) fail(response, error);
  return data;
}

async function reachPlan(cid: string, level: number): Promise<ReachPlan> {
  const { data, error, response } = await api.GET('/api/characters/{cid}/progression/plan', {
    params: { path: { cid }, query: { level } },
  });
  if (!data) fail(response, error);
  return data;
}

const SKIP_REASON: Record<string, string> = {
  feature: 'desligada nesta região',
  special: 'quadro especial (teste, PvP, caça)',
  no_mission: 'sem dados de missão',
  difficulty: 'nenhuma dificuldade abre neste nível',
  requirement: 'depende de quest fora do plano',
};

/** The quests "reach a level" would complete, before applying. */
function ReachPlanView({ cid, level }: { cid: string; level: number }) {
  const plan = useQuery({ queryKey: ['reach-plan', cid, level], queryFn: () => reachPlan(cid, level), enabled: level >= 1 });
  if (level < 1) return null;
  if (plan.error) return <p className="text-sm text-danger">{String(plan.error.message)}</p>;
  if (!plan.data) return <p className="text-sm text-muted">Calculando o plano…</p>;
  return (
    <div className="space-y-2 text-sm">
      <p>
        {plan.data.quests.length} quest(s) de missão concluída(s) até o nível {plan.data.level}. As histórias não são concluídas: elas avançam ao falar com os
        NPCs, porque leem as quests concluídas. Recompensas de EXP, ouro e itens das quests não são dadas.
      </p>
      <ul className="max-h-48 space-y-0.5 overflow-y-auto rounded-lg border border-border p-2">
        {plan.data.quests.map((q) => (
          <li key={q.id} className="flex justify-between gap-2">
            <span className="truncate">{q.title ?? q.id}</span>
            <span className="shrink-0 font-mono text-xs text-muted">
              nv {q.level} · dif. {q.difficulty}
            </span>
          </li>
        ))}
      </ul>
      {plan.data.skipped.length > 0 && (
        <details>
          <summary className="cursor-pointer text-muted">{plan.data.skipped.length} quest(s) até o nível ficam de fora</summary>
          <ul className="mt-1 max-h-32 space-y-0.5 overflow-y-auto text-xs text-muted">
            {plan.data.skipped.map((q) => (
              <li key={q.id}>
                {q.title ?? q.id}: {SKIP_REASON[q.reason] ?? q.reason}
              </li>
            ))}
          </ul>
        </details>
      )}
    </div>
  );
}

async function effects(search: string): Promise<CatalogEffect[]> {
  const { data, error, response } = await api.GET('/api/status-effects', { params: { query: { search } } });
  if (!data) fail(response, error);
  return data;
}

function Number_({
  id,
  label,
  value,
  onChange,
  optional,
}: {
  id: string;
  label: string;
  value: string;
  onChange: (v: string) => void;
  optional?: boolean;
}) {
  return (
    <div className="space-y-1.5">
      <Label htmlFor={id}>{label}</Label>
      <Input
        id={id}
        required={!optional}
        inputMode="numeric"
        pattern="[0-9]{1,10}"
        value={value}
        onChange={(e) => onChange(e.target.value.replace(/[^0-9]/g, ''))}
      />
    </div>
  );
}

function days(seconds?: number | null) {
  if (seconds == null || seconds < 0) return 'sem prazo';
  if (seconds >= 86400) return `${Math.round(seconds / 86400)} dia(s)`;
  return `${Math.round(seconds / 60)} min`;
}

export function ProgressionModal({
  cid,
  level,
  onClose,
  onDone,
}: {
  cid: string;
  level: number;
  onClose: () => void;
  onDone: () => void;
}) {
  const [mode, setMode] = useState<Mode>('reach');
  const [value, setValue] = useState('');
  const [search, setSearch] = useState('');
  const [effect, setEffect] = useState<CatalogEffect | null>(null);
  const [days_, setDays] = useState('');
  const term = search.trim();
  const found = useQuery({
    queryKey: ['status-effects', term],
    queryFn: () => effects(term),
    enabled: mode === 'effect' && term.length >= 2,
  });
  const change = (): Kind => {
    const n = Number(value);
    switch (mode) {
      case 'reach':
        return { kind: 'reach', level: n };
      case 'level':
        return { kind: 'level', level: n };
      case 'exp':
        return { kind: 'exp', amount: n };
      case 'ap':
        return { kind: 'ap', amount: n };
      case 'title':
        return { kind: 'title', title_id: n };
      case 'effect':
        if (!effect) throw new Error('Escolha um efeito na busca.');
        return {
          kind: 'add_effect',
          effect: effect.kind,
          level: effect.level,
          seconds: days_ ? Number(days_) * 86400 : null,
        };
    }
  };
  return (
    <Modal open title="Progressão" subtitle={OFFLINE_NOTE} icon={<TrendingUp className="size-5 text-muted" />} onClose={onClose} width="max-w-xl">
      <div role="tablist" className="mb-4 grid grid-cols-6 rounded-lg bg-raised p-1 text-sm">
        {MODES.map((m) => (
          <button
            key={m.id}
            role="tab"
            type="button"
            aria-selected={mode === m.id}
            onClick={() => {
              setMode(m.id);
              setValue('');
            }}
            className={cn('rounded-md py-1.5 text-muted', mode === m.id && 'bg-surface font-medium text-fg shadow-sm')}
          >
            {m.label}
          </button>
        ))}
      </div>
      <ActionForm
        key={mode}
        submitLabel="Aplicar"
        onCancel={onClose}
        onDone={onDone}
        onSubmit={async (operationId) => {
          await changeProgression(cid, operationId, change());
        }}
      >
        {mode === 'reach' && (
          <>
            <Number_ id="reach-level" label={`Nível alvo (atual ${level}; pode ser o atual para só concluir as quests)`} value={value} onChange={setValue} />
            <ReachPlanView cid={cid} level={Number(value) || 0} />
          </>
        )}
        {mode === 'level' && (
          <Number_ id="target-level" label={`Subir até o nível (atual ${level})`} value={value} onChange={setValue} />
        )}
        {mode === 'exp' && <Number_ id="exp" label="EXP a dar" value={value} onChange={setValue} />}
        {mode === 'ap' && <Number_ id="ap" label="AP a dar (com o limite do jogo)" value={value} onChange={setValue} />}
        {mode === 'title' && <Number_ id="title" label="ID do título (TitleInfo)" value={value} onChange={setValue} />}
        {mode === 'effect' && (
          <>
            <div className="space-y-1.5">
              <Label htmlFor="effect-search">Buscar efeito (Type ou nome em inglês, ex.: vip)</Label>
              <div className="relative">
                <Search className="pointer-events-none absolute top-2.5 left-3 size-4 text-muted" />
                <Input id="effect-search" className="pl-9" value={search} onChange={(e) => setSearch(e.target.value)} />
              </div>
              {term.length >= 2 && (
                <ul className="max-h-48 space-y-1 overflow-y-auto rounded-lg border border-border p-1">
                  {(found.data ?? []).map((e) => (
                    <li key={`${e.kind}-${e.level}`}>
                      <button
                        type="button"
                        onClick={() => setEffect(e)}
                        className={cn(
                          'w-full rounded-md px-2 py-1 text-left hover:bg-raised',
                          effect?.kind === e.kind && effect.level === e.level && 'bg-raised',
                        )}
                      >
                        <span className="text-sm font-medium">{e.name ?? e.kind}</span>{' '}
                        <span className="font-mono text-xs text-muted">
                          {e.kind} · nível {e.level} · {days(e.duration)}
                        </span>
                      </button>
                    </li>
                  ))}
                  {found.data?.length === 0 && <li className="px-2 py-1 text-sm text-muted">Nada encontrado.</li>}
                </ul>
              )}
            </div>
            {effect && (
              <p className="text-sm">
                Escolhido: <span className="font-mono">{effect.kind}</span> nível {effect.level}
              </p>
            )}
            <Number_
              id="effect-days"
              label="Duração em dias (vazio = duração do próprio efeito)"
              value={days_}
              onChange={setDays}
              optional
            />
          </>
        )}
      </ActionForm>
    </Modal>
  );
}

/** Remove a status effect by Type (StatusEffectManager.RemoveStatusEffect). */
export function RemoveEffectModal({
  cid,
  effect,
  onClose,
  onDone,
}: {
  cid: string;
  effect: { kind: string; name?: string | null };
  onClose: () => void;
  onDone: () => void;
}) {
  return (
    <Modal open title={`Remover ${effect.name ?? effect.kind}`} subtitle={OFFLINE_NOTE} onClose={onClose}>
      <ActionForm
        submitLabel="Remover efeito"
        danger
        onCancel={onClose}
        onDone={onDone}
        onSubmit={async (operationId) => {
          await changeProgression(cid, operationId, { kind: 'remove_effect', effect: effect.kind });
        }}
      >
        <p className="font-mono text-sm">{effect.kind}</p>
      </ActionForm>
    </Modal>
  );
}
