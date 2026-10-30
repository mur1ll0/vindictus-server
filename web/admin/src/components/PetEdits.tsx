import { useQuery } from '@tanstack/react-query';
import { PawPrint, Plus, Trash2 } from 'lucide-react';
import { useState } from 'react';
import { api, type components, fail } from '@/api/client';
import { ActionForm, errorText } from '@/components/forms';
import { Modal } from '@/components/Modal';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

type Pet = components['schemas']['PanelPet'];

/** PetStatus (ServiceCore): 1 Expired, 2 Disabled, 3 Useabled, 4 Selected. */
const STATUS: Record<number, string> = { 1: 'vencido', 2: 'cansado', 3: 'disponível', 4: 'invocado' };

async function accountPets(cid: string): Promise<Pet[]> {
  const { data, error, response } = await api.GET('/api/characters/{cid}/pets', { params: { path: { cid } } });
  if (!data) fail(response, error);
  return data;
}

async function petTypes() {
  const { data, error, response } = await api.GET('/api/pets/types');
  if (!data) fail(response, error);
  return data;
}

function hours(seconds: number) {
  return `${Math.floor(seconds / 3600)} h ${Math.floor((seconds % 3600) / 60)} min`;
}

/** The pets of the character's account (pets belong to the account, not the character). */
export function Pets({ cid, canManage, online }: { cid: string; canManage: boolean; online: boolean }) {
  const query = useQuery({ queryKey: ['pets', cid], queryFn: () => accountPets(cid) });
  const [adding, setAdding] = useState(false);
  const [removing, setRemoving] = useState<Pet | null>(null);
  const done = () => {
    void query.refetch();
    setAdding(false);
    setRemoving(null);
  };
  const pets = query.data;
  return (
    <div className="space-y-3">
      <p className="text-sm text-muted">Os pets são da conta: aparecem em todas as personagens dela.</p>
      {canManage && (
        <div className="flex flex-wrap items-center gap-2">
          <Button variant="outline" onClick={() => setAdding(true)} disabled={online}>
            <Plus /> Dar pet
          </Button>
          {online && <span className="text-sm text-danger">Online: pets só mudam com a conta fora do jogo.</span>}
        </div>
      )}
      {!pets && <p className="text-sm text-muted">{query.error ? errorText(query.error) : 'Carregando…'}</p>}
      {pets && pets.length === 0 && <p className="text-sm text-muted">Nenhum pet.</p>}
      {pets?.map((p) => (
        <Card key={p.id}>
          <div className="flex flex-wrap items-center gap-3">
            <PawPrint className="size-5 text-muted" />
            <div className="min-w-0 flex-1">
              <p className="font-semibold">
                {p.name} <span className="text-sm font-normal text-muted">nível {p.level}</span>
              </p>
              <p className="truncate font-mono text-xs text-muted">
                tipo {p.pet_type} · {p.item_class ?? 'sem item'} · slot {p.slot}
              </p>
            </div>
            <Badge tone={p.status === 4 ? 'ok' : undefined}>{STATUS[p.status] ?? `status ${p.status}`}</Badge>
            {canManage && !online && (
              <Button variant="danger" size="sm" onClick={() => setRemoving(p)}>
                <Trash2 /> Remover
              </Button>
            )}
          </div>
          <p className="mt-2 text-sm text-muted">
            EXP {p.exp} · fome {p.hunger}
            {p.max_hunger != null ? `/${p.max_hunger}` : ''} · tempo ativo {hours(p.remain_active_time)}
            {p.cash_skills.length > 0 ? ` · skills de cash ${p.cash_skills.join(', ')}` : ''}
          </p>
        </Card>
      ))}
      {adding && <AddPetModal cid={cid} onClose={() => setAdding(false)} onDone={done} />}
      {removing && (
        <Modal open title="Remover pet" subtitle={`${removing.name} (tipo ${removing.pet_type})`} onClose={() => setRemoving(null)}>
          <ActionForm
            submitLabel="Remover"
            danger
            onCancel={() => setRemoving(null)}
            onDone={done}
            onSubmit={async (operation_id) => {
              const { error, response } = await api.POST('/api/characters/{cid}/pets/{pet}/remove', {
                params: { path: { cid, pet: removing.id } },
                body: { operation_id },
              });
              if (!response.ok) fail(response, error);
            }}
          >
            <p className="text-sm text-muted">O pet e as skills de cash dele são apagados da conta.</p>
          </ActionForm>
        </Modal>
      )}
    </div>
  );
}

function AddPetModal({ cid, onClose, onDone }: { cid: string; onClose: () => void; onDone: () => void }) {
  const types = useQuery({ queryKey: ['pet-types'], queryFn: petTypes });
  const [type, setType] = useState('');
  const [name, setName] = useState('');
  return (
    <Modal
      open
      title="Dar pet"
      subtitle="Mesmas regras do jogo: nome com 4+ letras (um _ opcional), único na conta, até o limite de pets."
      icon={<PawPrint className="size-5 text-muted" />}
      onClose={onClose}
    >
      <ActionForm
        submitLabel="Dar"
        onCancel={onClose}
        onDone={onDone}
        onSubmit={async (operation_id) => {
          const { data, error, response } = await api.POST('/api/characters/{cid}/pets', {
            params: { path: { cid } },
            body: { operation_id, pet_type: Number(type), name },
          });
          if (!data) fail(response, error);
        }}
      >
        <div className="space-y-1.5">
          <Label htmlFor="pet-type">Tipo</Label>
          <select
            id="pet-type"
            required
            className="h-9 w-full rounded-md border border-border bg-surface px-2 text-sm"
            value={type}
            onChange={(e) => setType(e.target.value)}
          >
            <option value="">Escolha…</option>
            {types.data?.map((t) => (
              <option key={t.pet_type} value={t.pet_type}>
                {t.pet_type} · {t.item_class ?? '—'}
              </option>
            ))}
          </select>
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="pet-name">Nome</Label>
          <Input id="pet-name" required maxLength={20} value={name} onChange={(e) => setName(e.target.value)} />
        </div>
      </ActionForm>
    </Modal>
  );
}
