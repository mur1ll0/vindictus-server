import { Package } from 'lucide-react';
import { useState } from 'react';
import { cn } from '@/lib/utils';

/** The client's item icon (exported PNG), or a neutral box when there is none. */
export function ItemIcon({ icon, size = 'md' }: { icon?: string | null; size?: 'sm' | 'md' }) {
  const [failed, setFailed] = useState(false);
  const box = size === 'sm' ? 'size-8' : 'size-11';
  if (!icon || failed) {
    return (
      <span aria-hidden className={cn('flex shrink-0 items-center justify-center rounded-md bg-raised text-muted', box)}>
        <Package className="size-4" />
      </span>
    );
  }
  return (
    <img
      src={`/api/icons/item/${icon}.png`}
      alt=""
      loading="lazy"
      onError={() => setFailed(true)}
      className={cn('shrink-0 rounded-md bg-raised object-contain', box)}
    />
  );
}
