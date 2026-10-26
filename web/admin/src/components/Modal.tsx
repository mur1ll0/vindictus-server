import { X } from 'lucide-react';
import { type ReactNode, useEffect, useRef } from 'react';

/** Accessible modal on the native <dialog>: Esc and the close button call onClose. */
export function Modal({
  open,
  title,
  subtitle,
  icon,
  onClose,
  children,
  width = 'max-w-lg',
}: {
  open: boolean;
  title: ReactNode;
  subtitle?: ReactNode;
  icon?: ReactNode;
  onClose: () => void;
  children: ReactNode;
  width?: string;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);
  return (
    <dialog
      ref={ref}
      onClose={onClose}
      onClick={(e) => {
        // A click on the backdrop (the dialog element itself) closes it.
        if (e.target === ref.current) onClose();
      }}
      className={`m-auto w-[calc(100%-2rem)] ${width} rounded-xl border border-border bg-surface p-0 text-fg shadow-2xl backdrop:bg-black/50`}
    >
      {open && (
        <div className="p-5">
          <div className="mb-4 flex items-start gap-3">
            {icon}
            <div className="min-w-0 flex-1">
              <h3 className="truncate text-lg font-semibold">{title}</h3>
              {subtitle && <p className="text-sm text-muted">{subtitle}</p>}
            </div>
            <button
              type="button"
              aria-label="Fechar"
              onClick={onClose}
              className="rounded-md p-1.5 text-muted hover:bg-raised hover:text-fg"
            >
              <X className="size-4" />
            </button>
          </div>
          {children}
        </div>
      )}
    </dialog>
  );
}
