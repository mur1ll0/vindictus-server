import { useMutation } from '@tanstack/react-query';
import { type FormEvent, type ReactNode, useEffect, useRef, useState } from 'react';
import { ApiFailure } from '@/api/client';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

/**
 * Modal for one audited change: the caller's fields plus the mandatory reason. A new
 * operation id is made each time the dialog opens, so a double submit is refused by the
 * server instead of applied twice.
 */
export function ActionDialog({
  open,
  title,
  description,
  submitLabel,
  children,
  onSubmit,
  onClose,
  onDone,
}: {
  open: boolean;
  title: string;
  description?: ReactNode;
  submitLabel: string;
  children?: ReactNode;
  onSubmit: (operationId: string, reason: string) => Promise<void>;
  onClose: () => void;
  onDone: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const [reason, setReason] = useState('');
  const [operationId, setOperationId] = useState('');
  const mutation = useMutation({
    mutationFn: () => onSubmit(operationId, reason),
    onSuccess: () => {
      onDone();
      onClose();
    },
  });
  const { reset } = mutation;
  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) {
      setReason('');
      setOperationId(crypto.randomUUID());
      reset();
      dialog.showModal();
    } else if (!open && dialog.open) {
      dialog.close();
    }
  }, [open, reset]);
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
    <dialog
      ref={ref}
      onClose={onClose}
      className="m-auto w-full max-w-md rounded-lg border border-border bg-surface p-6 text-fg shadow-lg backdrop:bg-black/40"
    >
      <form onSubmit={submit} className="space-y-4">
        <h3 className="text-lg font-semibold">{title}</h3>
        {description && <div className="text-sm text-muted">{description}</div>}
        {children}
        <div className="space-y-1.5">
          <Label htmlFor="action-reason">Motivo (fica na auditoria)</Label>
          <Input
            id="action-reason"
            required
            maxLength={500}
            value={reason}
            onChange={(e) => setReason(e.target.value)}
          />
        </div>
        <p role="alert" aria-live="polite" className="min-h-5 text-sm text-danger">
          {error}
        </p>
        <div className="flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={onClose}>
            Cancelar
          </Button>
          <Button type="submit" disabled={mutation.isPending}>
            {mutation.isPending ? 'Aplicando…' : submitLabel}
          </Button>
        </div>
      </form>
    </dialog>
  );
}
