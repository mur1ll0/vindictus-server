import { Coins, Gem, PackagePlus } from 'lucide-react';
import { useState } from 'react';
import { type CharacterItem, gold } from '@/api/characters';
import { api, type components, fail } from '@/api/client';
import { ActionForm } from '@/components/forms';
import { ItemClassField } from '@/components/ItemPicker';
import { Modal } from '@/components/Modal';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { cn } from '@/lib/utils';

type Result = components['schemas']['ChangeResult'];

async function changeItem(cid: string, body: components['schemas']['ItemChange']): Promise<Result> {
  const { data, error, response } = await api.POST('/api/characters/{cid}/items', { params: { path: { cid } }, body });
  if (!data) fail(response, error);
  return data;
}

async function changeCash(cid: string, body: components['schemas']['CashChange']): Promise<Result> {
  const { data, error, response } = await api.POST('/api/characters/{cid}/cash', { params: { path: { cid } }, body });
  if (!data) fail(response, error);
  return data;
}

function Amount({ value, onChange, label = 'Quantidade' }: { value: string; onChange: (v: string) => void; label?: string }) {
  return (
    <div className="space-y-1.5">
      <Label htmlFor="amount">{label}</Label>
      <Input
        id="amount"
        required
        inputMode="numeric"
        pattern="[0-9]{1,10}"
        value={value}
        onChange={(e) => onChange(e.target.value.replace(/[^0-9]/g, ''))}
      />
    </div>
  );
}

function Direction({ give, onChange, labels }: { give: boolean; onChange: (v: boolean) => void; labels: [string, string] }) {
  return (
    <div role="radiogroup" className="grid grid-cols-2 rounded-lg bg-raised p-1 text-sm">
      {[true, false].map((g, i) => (
        <button
          key={labels[i]}
          type="button"
          role="radio"
          aria-checked={give === g}
          onClick={() => onChange(g)}
          className={cn('rounded-md py-1.5 text-muted', give === g && 'bg-surface font-medium text-fg shadow-sm')}
        >
          {labels[i]}
        </button>
      ))}
    </div>
  );
}

const OFFLINE_NOTE = 'O personagem precisa estar fora do jogo; a mudança aparece ao selecioná-lo de novo.';

export function GiveItemModal({
  cid,
  characterClass,
  onClose,
  onDone,
}: {
  cid: string;
  characterClass?: number;
  onClose: () => void;
  onDone: () => void;
}) {
  const [item, setItem] = useState('');
  const [count, setCount] = useState('1');
  return (
    <Modal open title="Dar item" subtitle={OFFLINE_NOTE} icon={<PackagePlus className="size-5 text-muted" />} onClose={onClose} width="max-w-xl">
      <ActionForm
        submitLabel="Dar item"
        onCancel={onClose}
        onDone={onDone}
        onSubmit={async (operation_id) => {
          await changeItem(cid, { operation_id, item, count: Number(count), action: 'give' });
        }}
      >
        <ItemClassField
          id="item"
          label="Item (classe, com atributos opcionais: classe[ENHANCE:6])"
          required
          value={item}
          onChange={(value) => setItem(value)}
          characterClass={characterClass}
          pickTitle="Item a dar"
        />
        <Amount value={count} onChange={setCount} />
      </ActionForm>
    </Modal>
  );
}

export function GoldModal({ cid, current, onClose, onDone }: { cid: string; current: number; onClose: () => void; onDone: () => void }) {
  const [give, setGive] = useState(true);
  const [amount, setAmount] = useState('');
  return (
    <Modal open title="Ouro" subtitle={`Atual: ${gold(current)}. ${OFFLINE_NOTE}`} icon={<Coins className="size-5 text-accent" />} onClose={onClose}>
      <ActionForm
        submitLabel={give ? 'Dar ouro' : 'Remover ouro'}
        danger={!give}
        onCancel={onClose}
        onDone={onDone}
        onSubmit={async (operation_id) => {
          await changeItem(cid, { operation_id, item: 'gold', count: Number(amount), action: give ? 'give' : 'remove' });
        }}
      >
        <Direction give={give} onChange={setGive} labels={['Dar', 'Remover']} />
        <Amount value={amount} onChange={setAmount} label="Ouro" />
      </ActionForm>
    </Modal>
  );
}

export function CashModal({ cid, current, onClose, onDone }: { cid: string; current: number; onClose: () => void; onDone: () => void }) {
  const [give, setGive] = useState(true);
  const [amount, setAmount] = useState('');
  return (
    <Modal
      open
      title="Cash"
      subtitle={`Saldo da conta: ${gold(current)}. Vale também com o personagem no jogo (o Depot lê o saldo ao abrir).`}
      icon={<Gem className="size-5 text-muted" />}
      onClose={onClose}
    >
      <ActionForm
        submitLabel={give ? 'Creditar' : 'Debitar'}
        danger={!give}
        onCancel={onClose}
        onDone={onDone}
        onSubmit={async (operation_id) => {
          const value = Number(amount);
          await changeCash(cid, { operation_id, delta: give ? value : -value });
        }}
      >
        <Direction give={give} onChange={setGive} labels={['Creditar', 'Debitar']} />
        <Amount value={amount} onChange={setAmount} label="Cash" />
      </ActionForm>
    </Modal>
  );
}

/** Remove `count` of the item's class (the game's DestroyItem order picks the stacks). */
export function RemoveItemForm({ cid, item, onCancel, onDone }: { cid: string; item: CharacterItem; onCancel: () => void; onDone: () => void }) {
  const [count, setCount] = useState(String(item.count));
  return (
    <ActionForm
      submitLabel="Remover"
      danger
      onCancel={onCancel}
      onDone={onDone}
      onSubmit={async (operation_id) => {
        await changeItem(cid, { operation_id, item: item.class, count: Number(count), action: 'remove' });
      }}
    >
      <p className="text-sm text-muted">
        Remove essa quantidade da classe <span className="font-mono">{item.class}</span>, na ordem do jogo (DestroyItem). {OFFLINE_NOTE}
      </p>
      <Amount value={count} onChange={setCount} />
    </ActionForm>
  );
}
