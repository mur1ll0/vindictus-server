import type { ReactNode } from 'react';

/** One labelled value in a small box, for detail grids. */
export function Stat({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="rounded-lg border border-border bg-raised px-3 py-2">
      <div className="text-xs text-muted">{label}</div>
      <div className="truncate text-sm font-semibold">{children}</div>
    </div>
  );
}
