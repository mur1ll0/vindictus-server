import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Ban, CircleCheck, KeyRound, Lock, Plus, Search, ShieldCheck } from 'lucide-react';
import { type FormEvent, useState } from 'react';
import { api, type components, fail, type Role } from '@/api/client';
import { accountCharacters } from '@/api/characters';
import { sessionQuery } from '@/api/session';
import { CharacterCards } from '@/components/CharacterCards';
import { Avatar } from '@/components/Avatar';
import { ActionForm, errorText, PasswordFields, RoleSelect, roleLabel } from '@/components/forms';
import { Modal } from '@/components/Modal';
import { Stat } from '@/components/Stat';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

type Account = components['schemas']['AccountSummary'];
type Action = 'details' | 'role' | 'password' | 'status';

async function list(search: string): Promise<Account[]> {
  const { data, error, response } = await api.GET('/api/accounts', {
    params: { query: { limit: 100, search: search || undefined } },
  });
  if (!data) fail(response, error);
  return data;
}

async function check(promise: Promise<{ response: Response; error?: components['schemas']['ApiError'] }>) {
  const { response, error } = await promise;
  if (!response.ok) fail(response, error);
}

function mismatch(password: string, confirm: string) {
  if (password !== confirm) throw new Error('As senhas não conferem.');
}

function gameLine(account: Account) {
  if (account.game_uid == null) return 'nunca entrou no jogo';
  return `UID ${account.game_uid} · ${account.characters} personagem(ns)`;
}

function AccountCard({ account, onOpen }: { account: Account; onOpen: () => void }) {
  return (
    <button
      type="button"
      onClick={onOpen}
      className="group w-full rounded-xl border border-border bg-surface p-4 text-left shadow-sm transition-colors hover:border-accent/70 focus-visible:outline-2 focus-visible:outline-accent"
    >
      <div className="flex items-center gap-3">
        <Avatar name={account.username} />
        <div className="min-w-0">
          <p className="truncate font-semibold">{account.username}</p>
          <p className="truncate text-xs text-muted">{gameLine(account)}</p>
        </div>
      </div>
      <div className="mt-4 flex flex-wrap items-center gap-2 border-t border-border pt-3">
        <Badge tone={account.panel_role ? 'accent' : 'neutral'}>{roleLabel(account.panel_role)}</Badge>
        {account.status === 'active' ? (
          <Badge tone="ok">Ativa</Badge>
        ) : (
          <Badge tone="danger">Desativada</Badge>
        )}
        {account.locked && (
          <Badge tone="danger">
            <Lock className="size-3" /> Bloqueada
          </Badge>
        )}
        {account.game_name && <span className="ml-auto text-xs text-muted">jogo: {account.game_name}</span>}
      </div>
    </button>
  );
}

function AccountModal({
  account,
  isAdmin,
  isSelf,
  onClose,
  onChanged,
}: {
  account: Account;
  isAdmin: boolean;
  isSelf: boolean;
  onClose: () => void;
  onChanged: () => void;
}) {
  const [action, setAction] = useState<Action>('details');
  const characters = useQuery({
    queryKey: ['accounts', account.username, 'characters'],
    queryFn: () => accountCharacters(account.username),
  });
  const [role, setRole] = useState<Role | ''>((account.panel_role as Role | null) ?? '');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const active = account.status === 'active';
  // A GM changes only accounts without a panel role (the server enforces it too).
  const mayChange = isAdmin || !account.panel_role;
  const done = () => {
    onChanged();
    setAction('details');
  };
  const path = { params: { path: { username: account.username } } };
  return (
    <Modal
      open
      title={account.username}
      subtitle={`${roleLabel(account.panel_role)} · ${active ? 'ativa' : 'desativada'}`}
      icon={<Avatar name={account.username} />}
      onClose={onClose}
    >
      {action === 'details' && (
        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
            <Stat label="Papel">{roleLabel(account.panel_role)}</Stat>
            <Stat label="Situação">{active ? 'Ativa' : 'Desativada'}</Stat>
            <Stat label="Criada">{new Date(account.created_at).toLocaleDateString('pt-BR')}</Stat>
            <Stat label="Nome no jogo">{account.game_name ?? account.username}</Stat>
            <Stat label="UID">{account.game_uid ?? '—'}</Stat>
            <Stat label="Personagens">{account.characters}</Stat>
          </div>
          <div className="space-y-2">
            <p className="text-sm font-medium">Personagens</p>
            <CharacterCards
              characters={characters.data ?? []}
              empty={characters.isPending ? 'Carregando…' : 'Nenhum personagem.'}
            />
          </div>
          {account.locked && (
            <p className="rounded-lg bg-danger-soft px-3 py-2 text-sm text-danger">
              Login bloqueado por tentativas erradas. Uma nova senha libera.
            </p>
          )}
          <div className="grid grid-cols-2 gap-2">
            {isAdmin && (
              <Button variant="outline" size="lg" onClick={() => setAction('role')}>
                <ShieldCheck /> Papel
              </Button>
            )}
            {mayChange && (
              <Button variant="outline" size="lg" onClick={() => setAction('password')}>
                <KeyRound /> Senha
              </Button>
            )}
            {mayChange && !isSelf && (
              <Button variant={active ? 'danger' : 'outline'} size="lg" onClick={() => setAction('status')}>
                {active ? <Ban /> : <CircleCheck />} {active ? 'Desativar' : 'Ativar'}
              </Button>
            )}
          </div>
        </div>
      )}
      {action === 'role' && (
        <ActionForm
          submitLabel="Salvar papel"
          onCancel={() => setAction('details')}
          onDone={done}
          onSubmit={(operation_id, reason) =>
            check(api.POST('/api/accounts/{username}/role', { ...path, body: { operation_id, reason, panel_role: role || null } }))
          }
        >
          <RoleSelect value={role} onChange={setRole} />
        </ActionForm>
      )}
      {action === 'password' && (
        <ActionForm
          submitLabel="Trocar senha"
          onCancel={() => setAction('details')}
          onDone={done}
          onSubmit={async (operation_id, reason) => {
            mismatch(password, confirm);
            await check(api.POST('/api/accounts/{username}/password', { ...path, body: { operation_id, reason, password } }));
          }}
        >
          <p className="text-sm text-muted">As sessões do painel dessa conta são encerradas.</p>
          <PasswordFields password={password} confirm={confirm} setPassword={setPassword} setConfirm={setConfirm} />
        </ActionForm>
      )}
      {action === 'status' && (
        <ActionForm
          submitLabel={active ? 'Desativar' : 'Ativar'}
          danger={active}
          onCancel={() => setAction('details')}
          onDone={done}
          onSubmit={(operation_id, reason) =>
            check(api.POST('/api/accounts/{username}/status', { ...path, body: { operation_id, reason, active: !active } }))
          }
        >
          <p className="text-sm text-muted">
            {active
              ? 'A conta deixa de entrar no painel e perde as sessões abertas.'
              : 'A conta volta a poder entrar.'}
          </p>
        </ActionForm>
      )}
    </Modal>
  );
}

function CreateModal({ isAdmin, onClose, onChanged }: { isAdmin: boolean; onClose: () => void; onChanged: () => void }) {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [role, setRole] = useState<Role | ''>('');
  return (
    <Modal open title="Nova conta" subtitle="Login e senha valem para o painel e para o jogo." icon={<Avatar name={username || '+'} />} onClose={onClose}>
      <ActionForm
        submitLabel="Criar conta"
        onCancel={onClose}
        onDone={() => {
          onChanged();
          onClose();
        }}
        onSubmit={async (operation_id, reason) => {
          mismatch(password, confirm);
          await check(api.POST('/api/accounts', { body: { operation_id, reason, username, password, panel_role: role || null } }));
        }}
      >
        <div className="space-y-1.5">
          <Label htmlFor="new-username">Login (3 a 24 letras, números ou _)</Label>
          <Input
            id="new-username"
            required
            autoComplete="off"
            pattern="[A-Za-z0-9_]{3,24}"
            maxLength={24}
            value={username}
            onChange={(e) => setUsername(e.target.value)}
          />
        </div>
        <PasswordFields password={password} confirm={confirm} setPassword={setPassword} setConfirm={setConfirm} />
        {isAdmin && <RoleSelect value={role} onChange={setRole} />}
      </ActionForm>
    </Modal>
  );
}

export function AccountsPage() {
  const queryClient = useQueryClient();
  const { data: session } = useQuery(sessionQuery);
  const isAdmin = session?.role === 'admin';
  const canManage = isAdmin || session?.role === 'gm';
  const [draft, setDraft] = useState('');
  const [search, setSearch] = useState('');
  const query = useQuery({ queryKey: ['accounts', search], queryFn: () => list(search) });
  const [selected, setSelected] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ['accounts'] });
    void queryClient.invalidateQueries({ queryKey: ['audit'] });
  };
  const submit = (event: FormEvent) => {
    event.preventDefault();
    setSearch(draft);
  };
  const accounts = query.data ?? [];
  const current = accounts.find((a) => a.username === selected);
  return (
    <section className="space-y-5">
      <div className="flex flex-wrap items-center gap-2">
        <form onSubmit={submit} className="flex min-w-0 flex-1 gap-2">
          <div className="relative min-w-0 flex-1 sm:max-w-md">
            <Search className="pointer-events-none absolute top-2.5 left-3 size-4 text-muted" />
            <Input
              aria-label="Buscar por login"
              placeholder="Buscar por login"
              className="pl-9"
              maxLength={32}
              value={draft}
              onChange={(e) => setDraft(e.target.value.replace(/[^A-Za-z0-9_]/g, ''))}
            />
          </div>
          <Button type="submit">Buscar</Button>
        </form>
        {canManage && (
          <Button variant="outline" onClick={() => setCreating(true)}>
            <Plus /> Nova conta
          </Button>
        )}
      </div>
      <p className="text-sm text-muted">
        {query.isPending ? 'Carregando…' : `${accounts.length} conta(s)${accounts.length === 100 ? ' (mostrando as 100 primeiras)' : ''}`}
      </p>
      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">
        {accounts.map((account) => (
          <AccountCard key={account.username} account={account} onOpen={() => setSelected(account.username)} />
        ))}
      </div>
      {query.error && (
        <p role="alert" className="text-sm text-danger">
          {errorText(query.error)}
        </p>
      )}
      {current && (
        <AccountModal
          key={current.username}
          account={current}
          isAdmin={isAdmin}
          isSelf={current.username.toLowerCase() === session?.username.toLowerCase()}
          onClose={() => setSelected(null)}
          onChanged={refresh}
        />
      )}
      {creating && <CreateModal isAdmin={isAdmin} onClose={() => setCreating(false)} onChanged={refresh} />}
    </section>
  );
}
