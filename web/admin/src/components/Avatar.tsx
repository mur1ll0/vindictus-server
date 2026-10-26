import { cn } from '@/lib/utils';

const HUES = [255, 155, 75, 25, 300, 200];

/** Initial of a name on a color picked from the name, so each account is easy to spot. */
export function Avatar({ name, size = 'md' }: { name: string; size?: 'md' | 'lg' }) {
  const hue = HUES[[...name].reduce((sum, c) => sum + c.charCodeAt(0), 0) % HUES.length];
  return (
    <span
      aria-hidden
      style={{ background: `oklch(0.7 0.13 ${hue})` }}
      className={cn(
        'inline-flex shrink-0 items-center justify-center rounded-lg font-semibold text-white',
        size === 'lg' ? 'size-12 text-lg' : 'size-10',
      )}
    >
      {name.charAt(0).toUpperCase()}
    </span>
  );
}
