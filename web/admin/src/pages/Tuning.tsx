import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Search, SlidersHorizontal } from 'lucide-react';
import { type ReactNode, useState } from 'react';
import { api, type components, fail } from '@/api/client';
import { createEdit } from '@/api/content';
import { sessionQuery } from '@/api/session';
import { ActionForm, errorText } from '@/components/forms';
import { Modal } from '@/components/Modal';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { cn } from '@/lib/utils';

type Tuning = components['schemas']['TuningView'];
type EditRequest = components['schemas']['EditRequest'];

async function tuning(): Promise<Tuning> {
  const { data, error, response } = await api.GET('/api/tuning');
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

type Edit = {
  title: string;
  subtitle: string;
  fields: { id: string; label: string; value: string; max: number; step?: string }[];
  build: (values: number[]) => EditRequest;
};

const NOTE = 'Fica pendente até um admin publicar a versão; a manutenção agendada reinicia o servidor com ela.';

function EditModal({ edit, onClose, onDone }: { edit: Edit; onClose: () => void; onDone: () => void }) {
  const [values, setValues] = useState(edit.fields.map((f) => f.value));
  return (
    <Modal open title={edit.title} subtitle={`${edit.subtitle} ${NOTE}`} icon={<SlidersHorizontal className="size-5 text-muted" />} onClose={onClose}>
      <ActionForm
        submitLabel="Salvar ajuste"
        onCancel={onClose}
        onDone={onDone}
        onSubmit={async (operationId) => {
          await createEdit(operationId, edit.build(values.map(Number)));
        }}
      >
        <div className="grid grid-cols-2 gap-3">
          {edit.fields.map((f, i) => (
            <div key={f.id} className="space-y-1.5">
              <Label htmlFor={f.id}>{f.label}</Label>
              <Input
                id={f.id}
                required
                type="number"
                min={0}
                max={f.max}
                step={f.step ?? '1'}
                value={values[i]}
                onChange={(e) => setValues(values.map((v, k) => (k === i ? e.target.value : v)))}
              />
            </div>
          ))}
        </div>
      </ActionForm>
    </Modal>
  );
}

function Row({ title, detail, value, onClick }: { title: string; detail?: string; value: ReactNode; onClick?: () => void }) {
  return (
    <button
      type="button"
      disabled={!onClick}
      onClick={onClick}
      className="flex w-full items-center gap-3 rounded-lg border border-border bg-surface px-3 py-2 text-left transition-colors enabled:hover:border-accent/70"
    >
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium">{title}</p>
        {detail && <p className="truncate font-mono text-xs text-muted">{detail}</p>}
      </div>
      <span className="text-sm font-semibold whitespace-nowrap">{value}</span>
    </button>
  );
}

const pct = (r: number) => `${Math.round(r * 1000) / 10}%`;

export function TuningPage() {
  const queryClient = useQueryClient();
  const { data: session } = useQuery(sessionQuery);
  const canEdit = session?.role === 'admin' || session?.role === 'gm';
  const data = useQuery({ queryKey: ['tuning'], queryFn: tuning });
  const [tab, setTab] = useState<'rates' | 'drops' | 'enhance' | 'enchant'>('rates');
  const [filter, setFilter] = useState('');
  const [edit, setEdit] = useState<Edit | null>(null);
  const term = filter.trim().toLowerCase();
  const open = (e: Edit) => (canEdit ? () => setEdit(e) : undefined);
  const done = () => {
    setEdit(null);
    void queryClient.invalidateQueries({ queryKey: ['content'] });
    void queryClient.invalidateQueries({ queryKey: ['audit'] });
  };
  const t = data.data;
  const tabs = [
    { id: 'rates', label: 'Taxas', n: t?.features.length },
    { id: 'drops', label: 'Drops por fase', n: t?.quests.length },
    { id: 'enhance', label: 'Enhance', n: t?.enhance.length },
    { id: 'enchant', label: 'Enchant', n: t?.enchants.length },
  ] as const;
  return (
    <section className="space-y-4">
      <Card>
        <p className="text-sm text-muted">
          Valores que o servidor carregou (versões publicadas incluídas). Cada mudança vira uma edição pendente em Conteúdo e
          entra no jogo quando a versão é publicada e o servidor reinicia. A cópia do cliente recebe os mesmos valores ao abrir
          pelo <span className="font-mono">start-lab.bat</span>.
        </p>
      </Card>
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
      {t && tab === 'rates' && (
        <div className="grid gap-2 sm:grid-cols-2">
          {t.features.map((f) => (
            <Row
              key={f.key}
              title={FEATURE_LABEL[f.key] ?? f.key}
              detail={f.key}
              value={`${f.value}%`}
              onClick={open({
                title: FEATURE_LABEL[f.key] ?? f.key,
                subtitle: `${f.key} do FeatureMatrix, em porcentagem (100 = original).`,
                fields: [{ id: 'value', label: 'Porcentagem (0 a 10000)', value: String(f.value), max: 10000 }],
                build: ([value]) => ({ kind: 'feature', key: f.key, value: value ?? 0 }),
              })}
            />
          ))}
        </div>
      )}
      {t && tab === 'drops' && (
        <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
          {t.quests
            .filter((q) => !term || q.quest.toLowerCase().includes(term) || (q.title ?? '').toLowerCase().includes(term))
            .slice(0, 300)
            .map((q) => (
              <Row
                key={q.quest}
                title={q.title ?? q.quest}
                detail={q.quest}
                value={`${q.min}–${q.max}`}
                onClick={open({
                  title: q.title ?? q.quest,
                  subtitle: 'Quantos monstros comuns da fase recebem drop (sorteio entre o mínimo e o máximo).',
                  fields: [
                    { id: 'min', label: 'Mínimo', value: String(q.min), max: 100 },
                    { id: 'max', label: 'Máximo', value: String(q.max), max: 100 },
                  ],
                  build: ([min, max]) => ({ kind: 'quest_drop', quest: q.quest, min: min ?? 0, max: max ?? 0 }),
                })}
              />
            ))}
        </div>
      )}
      {t && tab === 'enhance' && (
        <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
          {[...t.enhance].sort((a, b) => a.level - b.level).map((e) => (
            <Row
              key={`${e.enhance_type}#${e.level}`}
              title={`Enhance +${e.level}`}
              detail={`EnhanceServerInfo ${e.enhance_type}`}
              value={pct(e.ratio)}
              onClick={open({
                title: `Chance de sucesso do Enhance +${e.level}`,
                subtitle: 'A chance que o servidor usa para qualquer equipamento nesse nível.',
                fields: [{ id: 'ratio', label: 'Chance (%)', value: String(Math.round(e.ratio * 1000) / 10), max: 100, step: '0.1' }],
                build: ([value]) => ({ kind: 'enhance_ratio', enhance_type: e.enhance_type, level: e.level, ratio: (value ?? 0) / 100 }),
              })}
            />
          ))}
        </div>
      )}
      {t && tab === 'enchant' && (
        <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
          {t.enchants
            .filter((e) => !term || e.class.toLowerCase().includes(term) || (e.name ?? '').toLowerCase().includes(term))
            .slice(0, 300)
            .map((e) => (
              <Row
                key={e.class}
                title={`${e.name ?? e.class} (${e.prefix ? 'prefixo' : 'sufixo'}, rank ${e.rank})`}
                detail={e.class}
                value={`${e.min}–${e.max}%`}
                onClick={open({
                  title: e.name ?? e.class,
                  subtitle: 'Chance do encantamento no início e no fim do minijogo (%).',
                  fields: [
                    { id: 'min', label: 'Mínima (%)', value: String(e.min), max: 100 },
                    { id: 'max', label: 'Máxima (%)', value: String(e.max), max: 100 },
                  ],
                  build: ([min, max]) => ({ kind: 'enchant_ratio', class: e.class, min: min ?? 0, max: max ?? 0 }),
                })}
              />
            ))}
        </div>
      )}
      {edit && <EditModal edit={edit} onClose={() => setEdit(null)} onDone={done} />}
    </section>
  );
}
