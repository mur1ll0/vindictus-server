import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { KeyRound } from 'lucide-react';
import { type FormEvent, useState } from 'react';
import { api, fail } from '@/api/client';
import { myCharacters } from '@/api/characters';
import { Avatar } from '@/components/Avatar';
import { CharacterCards } from '@/components/CharacterCards';
import { errorText, PasswordFields, roleLabel } from '@/components/forms';
import { Modal } from '@/components/Modal';
import { Stat } from '@/components/Stat';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

async function me() {
  const { data, error, response } = await api.GET('/api/me');
  if (!data) fail(response, error);
  return data;
}

function PasswordModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [current, setCurrent] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const mutation = useMutation({
    mutationFn: async () => {
      if (password !== confirm) throw new Error('As senhas não conferem.');
      const { error, response } = await api.POST('/api/me/password', {
        body: { current_password: current, new_password: password },
      });
      if (!response.ok) fail(response, error);
    },
    // Every session of the account ends with the new password: log in again.
    onSuccess: () => {
      queryClient.clear();
      void navigate({ to: '/login', search: { aviso: 'senha' } });
    },
  });
  const submit = (event: FormEvent) => {
    event.preventDefault();
    mutation.mutate();
  };
  return (
    <Modal open title="Trocar senha" subtitle="Depois de trocar, entre de novo com a senha nova." icon={<KeyRound className="size-5 text-muted" />} onClose={onClose}>
      <form onSubmit={submit} className="space-y-4">
        <div className="space-y-1.5">
          <Label htmlFor="current-password">Senha atual</Label>
          <Input
            id="current-password"
            type="password"
            autoComplete="current-password"
            required
            maxLength={128}
            value={current}
            onChange={(e) => setCurrent(e.target.value)}
          />
        </div>
        <PasswordFields
          label="Senha nova (12 a 128 caracteres)"
          password={password}
          confirm={confirm}
          setPassword={setPassword}
          setConfirm={setConfirm}
        />
        <p role="alert" aria-live="polite" className="min-h-5 text-sm text-danger">
          {errorText(mutation.error)}
        </p>
        <div className="flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={onClose}>
            Cancelar
          </Button>
          <Button type="submit" disabled={mutation.isPending}>
            {mutation.isPending ? 'Trocando…' : 'Trocar senha'}
          </Button>
        </div>
      </form>
    </Modal>
  );
}

export function MyAccountPage() {
  const query = useQuery({ queryKey: ['me'], queryFn: me });
  const characters = useQuery({ queryKey: ['me', 'characters'], queryFn: myCharacters });
  const [changing, setChanging] = useState(false);
  const account = query.data;
  if (!account) {
    return <p className="text-sm text-muted">{query.error ? errorText(query.error) : 'Carregando…'}</p>;
  }
  return (
    <section className="max-w-3xl space-y-4">
      <Card>
        <div className="flex flex-wrap items-center gap-4">
          <Avatar name={account.username} size="lg" />
          <div className="min-w-0 flex-1">
            <p className="text-lg font-semibold">{account.username}</p>
            <div className="mt-1 flex flex-wrap gap-2">
              <Badge tone={account.panel_role ? 'accent' : 'neutral'}>{roleLabel(account.panel_role)}</Badge>
              <Badge tone={account.status === 'active' ? 'ok' : 'danger'}>
                {account.status === 'active' ? 'Ativa' : 'Desativada'}
              </Badge>
            </div>
          </div>
          <Button variant="outline" onClick={() => setChanging(true)}>
            <KeyRound /> Trocar senha
          </Button>
        </div>
        <div className="mt-5 grid grid-cols-2 gap-2 sm:grid-cols-3">
          <Stat label="Nome no jogo">{account.game_name ?? account.username}</Stat>
          <Stat label="UID no jogo">{account.game_uid ?? 'ainda não entrou'}</Stat>
          <Stat label="Personagens">{account.characters}</Stat>
          <Stat label="Criada em">{new Date(account.created_at).toLocaleDateString('pt-BR')}</Stat>
          <Stat label="Papel no painel">{roleLabel(account.panel_role)}</Stat>
        </div>
      </Card>
      <Card>
        <h2 className="mb-3 font-semibold">Meus personagens</h2>
        <CharacterCards characters={characters.data ?? []} empty={characters.isPending ? 'Carregando…' : 'Nenhum personagem ainda: entre no jogo e crie um.'} />
      </Card>
      {changing && <PasswordModal onClose={() => setChanging(false)} />}
    </section>
  );
}
