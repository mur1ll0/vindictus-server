import { useMutation } from '@tanstack/react-query';
import { type FormEvent, type ReactNode, useState } from 'react';
import { ApiFailure, type Role } from '@/api/client';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

export const ROLE_LABEL: Record<Role, string> = { admin: 'Admin', gm: 'GM', leitura: 'Leitura' };

export function roleLabel(role?: string | null) {
  return role ? (ROLE_LABEL[role as Role] ?? role) : 'Jogador';
}

export function errorText(error: unknown) {
  if (error instanceof ApiFailure) return error.message;
  if (error instanceof Error) return error.message;
  return error ? 'Falha de conexão.' : '';
}

/** Form of one audited change: the caller's fields and one operation id per form, so a double submit is refused by the server instead of applied twice. */
export function ActionForm({
  submitLabel,
  danger,
  children,
  onSubmit,
  onCancel,
  onDone,
}: {
  submitLabel: string;
  danger?: boolean;
  children?: ReactNode;
  onSubmit: (operationId: string) => Promise<void>;
  onCancel: () => void;
  onDone: () => void;
}) {
  const [operationId] = useState(() => crypto.randomUUID());
  const mutation = useMutation({ mutationFn: () => onSubmit(operationId), onSuccess: onDone });
  const submit = (event: FormEvent) => {
    event.preventDefault();
    mutation.mutate();
  };
  return (
    <form onSubmit={submit} className="space-y-4">
      {children}
      <p role="alert" aria-live="polite" className="min-h-5 text-sm text-danger">
        {errorText(mutation.error)}
      </p>
      <div className="flex justify-end gap-2">
        <Button type="button" variant="outline" onClick={onCancel}>
          Voltar
        </Button>
        <Button type="submit" variant={danger ? 'danger' : 'default'} disabled={mutation.isPending}>
          {mutation.isPending ? 'Aplicando…' : submitLabel}
        </Button>
      </div>
    </form>
  );
}

export function PasswordFields({
  password,
  confirm,
  setPassword,
  setConfirm,
  label = 'Senha (12 a 128 caracteres)',
}: {
  password: string;
  confirm: string;
  setPassword: (v: string) => void;
  setConfirm: (v: string) => void;
  label?: string;
}) {
  return (
    <>
      <div className="space-y-1.5">
        <Label htmlFor="new-password">{label}</Label>
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

export function RoleSelect({ value, onChange }: { value: Role | ''; onChange: (v: Role | '') => void }) {
  return (
    <div className="space-y-1.5">
      <Label htmlFor="role">Papel no painel</Label>
      <select
        id="role"
        value={value}
        onChange={(e) => onChange(e.target.value as Role | '')}
        className="h-9 w-full rounded-md border border-border bg-surface px-2 text-sm"
      >
        <option value="">Jogador (só a própria conta)</option>
        <option value="leitura">Leitura</option>
        <option value="gm">GM</option>
        <option value="admin">Admin</option>
      </select>
    </div>
  );
}
