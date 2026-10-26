import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { api, type components, fail, type Role } from '@/api/client';
import { sessionQuery } from '@/api/session';
import { ActionDialog } from '@/components/ActionDialog';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

type Account = components['schemas']['AccountSummary'];
type Dialog =
  | { kind: 'create' }
  | { kind: 'role' | 'password' | 'status'; account: Account }
  | null;

const ROLES: { value: Role | ''; label: string }[] = [
  { value: '', label: 'Sem acesso ao painel' },
  { value: 'leitura', label: 'Leitura' },
  { value: 'gm', label: 'GM' },
  { value: 'admin', label: 'Admin' },
];

const roleLabel = (role?: string | null) => ROLES.find((r) => r.value === (role ?? ''))?.label ?? role;

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

function RoleSelect({ value, onChange }: { value: Role | ''; onChange: (v: Role | '') => void }) {
  return (
    <div className="space-y-1.5">
      <Label htmlFor="role">Papel no painel</Label>
      <select
        id="role"
        value={value}
        onChange={(e) => onChange(e.target.value as Role | '')}
        className="h-9 w-full rounded-md border border-border bg-surface px-2 text-sm"
      >
        {ROLES.map((r) => (
          <option key={r.value} value={r.value}>
            {r.label}
          </option>
        ))}
      </select>
    </div>
  );
}

function PasswordFields({
  password,
  confirm,
  setPassword,
  setConfirm,
}: {
  password: string;
  confirm: string;
  setPassword: (v: string) => void;
  setConfirm: (v: string) => void;
}) {
  return (
    <>
      <div className="space-y-1.5">
        <Label htmlFor="new-password">Senha (12 a 128 caracteres)</Label>
        <Input
          id="new-password"
          type="password"
          autoComplete="new-password"
          required
          minLength={12}
          maxLength={128}
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
      </div>
      <div className="space-y-1.5">
        <Label htmlFor="confirm-password">Repita a senha</Label>
        <Input
          id="confirm-password"
          type="password"
          autoComplete="new-password"
          required
          value={confirm}
          onChange={(e) => setConfirm(e.target.value)}
          aria-invalid={confirm !== '' && confirm !== password}
        />
        {confirm !== '' && confirm !== password && (
          <p className="text-xs text-danger">As senhas não conferem.</p>
        )}
      </div>
    </>
  );
}

export function AccountsPage() {
  const queryClient = useQueryClient();
  const { data: session } = useQuery(sessionQuery);
  const isAdmin = session?.role === 'admin';
  const canManage = session?.role === 'admin' || session?.role === 'gm';
  const [search, setSearch] = useState('');
  const query = useQuery({ queryKey: ['accounts', search], queryFn: () => list(search) });
  const [dialog, setDialog] = useState<Dialog>(null);
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [role, setRole] = useState<Role | ''>('');

  const open = (next: Dialog) => {
    setUsername('');
    setPassword('');
    setConfirm('');
    setRole(next && next.kind === 'role' ? ((next.account.panel_role as Role | null) ?? '') : '');
    setDialog(next);
  };
  const close = () => setDialog(null);
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ['accounts'] });
    void queryClient.invalidateQueries({ queryKey: ['audit'] });
  };
  const mismatch = () => {
    if (password !== confirm) throw new Error('As senhas não conferem.');
  };
  const account = dialog && dialog.kind !== 'create' ? dialog.account : null;

  return (
    <section className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2 className="text-lg font-semibold">Contas</h2>
        <div className="flex items-center gap-2">
          <Input
            aria-label="Buscar login"
            placeholder="Buscar login"
            className="w-48"
            maxLength={32}
            value={search}
            onChange={(e) => setSearch(e.target.value.replace(/[^A-Za-z0-9_]/g, ''))}
          />
          {canManage && <Button onClick={() => open({ kind: 'create' })}>Criar conta</Button>}
        </div>
      </div>
      <Card className="overflow-x-auto p-0">
        <table className="w-full text-left text-sm">
          <thead className="border-b border-border text-muted">
            <tr>
              <th className="px-4 py-2 font-medium">Login</th>
              <th className="px-4 py-2 font-medium">Painel</th>
              <th className="px-4 py-2 font-medium">Situação</th>
              <th className="px-4 py-2 font-medium">Jogo</th>
              <th className="px-4 py-2 font-medium">Criada</th>
              <th className="px-4 py-2 font-medium">Ações</th>
            </tr>
          </thead>
          <tbody>
            {query.data?.map((row) => (
              <tr key={row.username} className="border-b border-border align-top last:border-0">
                <td className="px-4 py-2 font-medium">
                  {row.username}
                  {row.game_name && <span className="block text-xs text-muted">no jogo: {row.game_name}</span>}
                </td>
                <td className="px-4 py-2">{roleLabel(row.panel_role)}</td>
                <td className="px-4 py-2">
                  {row.status === 'active' ? 'Ativa' : <span className="text-danger">Desativada</span>}
                  {row.locked && <span className="ml-2 text-xs text-danger">login bloqueado</span>}
                </td>
                <td className="px-4 py-2">
                  {row.game_uid != null ? `UID ${row.game_uid}, ${row.characters} personagem(ns)` : 'nunca entrou'}
                </td>
                <td className="whitespace-nowrap px-4 py-2">{new Date(row.created_at).toLocaleDateString('pt-BR')}</td>
                <td className="px-4 py-2">
                  <div className="flex flex-wrap gap-1">
                    {isAdmin && (
                      <Button size="sm" variant="outline" onClick={() => open({ kind: 'role', account: row })}>
                        Papel
                      </Button>
                    )}
                    {canManage && (
                      <Button size="sm" variant="outline" onClick={() => open({ kind: 'password', account: row })}>
                        Nova senha
                      </Button>
                    )}
                    {canManage && row.username.toLowerCase() !== session?.username.toLowerCase() && (
                      <Button size="sm" variant="outline" onClick={() => open({ kind: 'status', account: row })}>
                        {row.status === 'active' ? 'Desativar' : 'Ativar'}
                      </Button>
                    )}
                  </div>
                </td>
              </tr>
            ))}
            {query.data?.length === 0 && (
              <tr>
                <td colSpan={6} className="px-4 py-6 text-center text-muted">
                  Nenhuma conta.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </Card>
      {query.error && (
        <p role="alert" className="text-sm text-danger">
          {query.error.message}
        </p>
      )}

      <ActionDialog
        open={dialog?.kind === 'create'}
        title="Criar conta"
        description="Login e senha valem para o jogo e, com um papel, para o painel."
        submitLabel="Criar"
        onClose={close}
        onDone={refresh}
        onSubmit={async (operation_id, reason) => {
          mismatch();
          await check(
            api.POST('/api/accounts', {
              body: { operation_id, reason, username, password, panel_role: role || null },
            }),
          );
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
      </ActionDialog>

      <ActionDialog
        open={dialog?.kind === 'role'}
        title={`Papel de ${account?.username ?? ''}`}
        submitLabel="Salvar papel"
        onClose={close}
        onDone={refresh}
        onSubmit={async (operation_id, reason) => {
          if (!account) return;
          await check(
            api.POST('/api/accounts/{username}/role', {
              params: { path: { username: account.username } },
              body: { operation_id, reason, panel_role: role || null },
            }),
          );
        }}
      >
        <RoleSelect value={role} onChange={setRole} />
      </ActionDialog>

      <ActionDialog
        open={dialog?.kind === 'password'}
        title={`Nova senha de ${account?.username ?? ''}`}
        description="As sessões do painel dessa conta são encerradas."
        submitLabel="Trocar senha"
        onClose={close}
        onDone={refresh}
        onSubmit={async (operation_id, reason) => {
          if (!account) return;
          mismatch();
          await check(
            api.POST('/api/accounts/{username}/password', {
              params: { path: { username: account.username } },
              body: { operation_id, reason, password },
            }),
          );
        }}
      >
        <PasswordFields password={password} confirm={confirm} setPassword={setPassword} setConfirm={setConfirm} />
      </ActionDialog>

      <ActionDialog
        open={dialog?.kind === 'status'}
        title={`${account?.status === 'active' ? 'Desativar' : 'Ativar'} ${account?.username ?? ''}`}
        description={
          account?.status === 'active'
            ? 'A conta deixa de entrar no painel (e, a partir do B4.2, no jogo).'
            : 'A conta volta a poder entrar.'
        }
        submitLabel={account?.status === 'active' ? 'Desativar' : 'Ativar'}
        onClose={close}
        onDone={refresh}
        onSubmit={async (operation_id, reason) => {
          if (!account) return;
          await check(
            api.POST('/api/accounts/{username}/status', {
              params: { path: { username: account.username } },
              body: { operation_id, reason, active: account.status !== 'active' },
            }),
          );
        }}
      />
    </section>
  );
}
