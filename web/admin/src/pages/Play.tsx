import { useMutation } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { type FormEvent, useState } from 'react';
import { api, ApiFailure, fail } from '@/api/client';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

async function requestCode(username: string, password: string) {
  const { data, error, response } = await api.POST('/api/play/code', { body: { username, password } });
  if (!data) fail(response, error);
  return data;
}

/** Game login, step one: login and password give a one-time code for the client's ID field. */
export function PlayPage() {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [copied, setCopied] = useState(false);
  const mutation = useMutation({
    mutationFn: () => requestCode(username, password),
    onSuccess: () => {
      setPassword('');
      setCopied(false);
    },
  });
  const submit = (event: FormEvent) => {
    event.preventDefault();
    mutation.mutate();
  };
  const copy = async (code: string) => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };
  const error =
    mutation.error instanceof ApiFailure
      ? mutation.error.message
      : mutation.error
        ? 'Falha de conexão.'
        : '';
  const code = mutation.data;
  return (
    <main className="flex min-h-screen items-center justify-center px-4">
      <Card className="w-full max-w-sm">
        <h1 className="mb-1 text-xl font-semibold">Entrar no jogo</h1>
        {code ? (
          <div className="space-y-4">
            <p className="text-sm text-muted">
              Na tela de login do jogo, digite este código no campo de ID e deixe a senha em branco.
              Vale uma vez, por {Math.round(code.expires_in_seconds / 60)} minutos.
            </p>
            <p
              aria-label="Código de jogo"
              className="select-all rounded-md border border-border bg-bg py-3 text-center font-mono text-2xl tracking-widest"
            >
              {code.code}
            </p>
            <div className="flex gap-2">
              <Button className="flex-1" onClick={() => void copy(code.code)}>
                {copied ? 'Copiado' : 'Copiar'}
              </Button>
              <Button className="flex-1" variant="outline" onClick={() => mutation.reset()}>
                Outro código
              </Button>
            </div>
          </div>
        ) : (
          <form onSubmit={submit} className="space-y-4">
            <p className="text-sm text-muted">Use o login e a senha da sua conta.</p>
            <div className="space-y-1.5">
              <Label htmlFor="play-username">Login</Label>
              <Input
                id="play-username"
                autoComplete="username"
                required
                maxLength={32}
                value={username}
                onChange={(e) => setUsername(e.target.value)}
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="play-password">Senha</Label>
              <Input
                id="play-password"
                type="password"
                autoComplete="current-password"
                required
                maxLength={128}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
            </div>
            <p role="alert" aria-live="polite" className="min-h-5 text-sm text-danger">
              {error}
            </p>
            <Button type="submit" className="w-full" disabled={mutation.isPending}>
              {mutation.isPending ? 'Gerando…' : 'Gerar código'}
            </Button>
          </form>
        )}
        <p className="mt-6 text-center text-sm">
          <Link to="/login" className="text-muted hover:text-fg">
            Painel de administração
          </Link>
        </p>
      </Card>
    </main>
  );
}
