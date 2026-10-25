import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { type FormEvent, useState } from 'react';
import { ApiFailure } from '@/api/client';
import { login, sessionQuery } from '@/api/session';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

export function LoginPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const mutation = useMutation({
    mutationFn: () => login(username, password),
    onSuccess: (session) => {
      setPassword('');
      queryClient.setQueryData(sessionQuery.queryKey, session);
      void navigate({ to: '/' });
    },
  });
  const submit = (event: FormEvent) => {
    event.preventDefault();
    mutation.mutate();
  };
  const error =
    mutation.error instanceof ApiFailure
      ? mutation.error.message
      : mutation.error
        ? 'Falha de conexão.'
        : '';
  return (
    <main className="flex min-h-screen items-center justify-center px-4">
      <Card className="w-full max-w-sm">
        <h1 className="mb-1 text-xl font-semibold">Painel Vindictus</h1>
        <p className="mb-6 text-sm text-muted">Entre com o login e a senha da sua conta do jogo.</p>
        <form onSubmit={submit} className="space-y-4">
          <div className="space-y-1.5">
            <Label htmlFor="username">Login</Label>
            <Input
              id="username"
              autoComplete="username"
              required
              maxLength={32}
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
          </div>
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
          <p role="alert" aria-live="polite" className="min-h-5 text-sm text-danger">
            {error}
          </p>
          <Button type="submit" className="w-full" disabled={mutation.isPending}>
            {mutation.isPending ? 'Entrando…' : 'Entrar'}
          </Button>
        </form>
      </Card>
    </main>
  );
}
