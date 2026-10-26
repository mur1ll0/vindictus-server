import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate, useSearch } from '@tanstack/react-router';
import { Shield } from 'lucide-react';
import { type FormEvent, useState } from 'react';
import { homeOf, login, register, sessionQuery } from '@/api/session';
import { errorText, PasswordFields } from '@/components/forms';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { cn } from '@/lib/utils';

type Mode = 'login' | 'register';

export function LoginPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const search = useSearch({ strict: false }) as { aviso?: string };
  const [mode, setMode] = useState<Mode>('login');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const mutation = useMutation({
    mutationFn: async () => {
      if (mode === 'login') return login(username, password);
      if (password !== confirm) throw new Error('As senhas não conferem.');
      return register(username, password);
    },
    onSuccess: (session) => {
      setPassword('');
      setConfirm('');
      // Data and errors of an earlier session do not carry over.
      queryClient.clear();
      queryClient.setQueryData(sessionQuery.queryKey, session);
      void navigate({ to: homeOf(session) });
    },
  });
  const switchTo = (next: Mode) => {
    setMode(next);
    setPassword('');
    setConfirm('');
    mutation.reset();
  };
  const submit = (event: FormEvent) => {
    event.preventDefault();
    mutation.mutate();
  };
  return (
    <main className="flex min-h-screen items-center justify-center px-4">
      <Card className="w-full max-w-sm">
        <div className="mb-5 flex items-center gap-3">
          <span className="flex size-10 items-center justify-center rounded-lg bg-accent-soft text-accent">
            <Shield className="size-5" />
          </span>
          <div>
            <h1 className="text-lg font-semibold leading-tight">Vindictus</h1>
            <p className="text-sm text-muted">Painel da conta e administração</p>
          </div>
        </div>
        <div role="tablist" className="mb-5 grid grid-cols-2 rounded-lg bg-raised p-1 text-sm">
          {(['login', 'register'] as const).map((m) => (
            <button
              key={m}
              role="tab"
              type="button"
              aria-selected={mode === m}
              onClick={() => switchTo(m)}
              className={cn('rounded-md py-1.5 text-muted', mode === m && 'bg-surface font-medium text-fg shadow-sm')}
            >
              {m === 'login' ? 'Entrar' : 'Criar conta'}
            </button>
          ))}
        </div>
        {search.aviso === 'senha' && mode === 'login' && (
          <p className="mb-4 rounded-lg bg-ok-soft px-3 py-2 text-sm text-ok">Senha trocada. Entre com a senha nova.</p>
        )}
        <form onSubmit={submit} className="space-y-4">
          <div className="space-y-1.5">
            <Label htmlFor="username">Login{mode === 'register' && ' (3 a 24 letras, números ou _)'}</Label>
            <Input
              id="username"
              autoComplete="username"
              required
              maxLength={mode === 'register' ? 24 : 32}
              pattern={mode === 'register' ? '[A-Za-z0-9_]{3,24}' : undefined}
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
          </div>
          {mode === 'login' ? (
            <div className="space-y-1.5">
              <Label htmlFor="password">Senha</Label>
              <Input
                id="password"
                type="password"
                autoComplete="current-password"
                required
                maxLength={128}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
            </div>
          ) : (
            <PasswordFields password={password} confirm={confirm} setPassword={setPassword} setConfirm={setConfirm} />
          )}
          <p role="alert" aria-live="polite" className="min-h-5 text-sm text-danger">
            {errorText(mutation.error)}
          </p>
          <Button type="submit" className="w-full" disabled={mutation.isPending}>
            {mutation.isPending ? 'Aguarde…' : mode === 'login' ? 'Entrar' : 'Criar conta e entrar'}
          </Button>
          {mode === 'register' && (
            <p className="text-xs text-muted">
              A conta nova vê só os próprios dados. No jogo, digite o login no campo ID e deixe a senha em branco.
            </p>
          )}
        </form>
      </Card>
    </main>
  );
}
