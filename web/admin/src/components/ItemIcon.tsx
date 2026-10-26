import { Package } from 'lucide-react';
import { useEffect, useState } from 'react';
import { cn } from '@/lib/utils';

const painted = new Map<string, Promise<string>>();

function load(src: string) {
  return new Promise<HTMLImageElement>((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = reject;
    image.src = src;
  });
}

/**
 * Equipment icons are dye masks: red, green and blue mark the parts painted with the item's
 * color 1, 2 and 3 (0xRRGGBB); grey pixels (metal, skin) keep their shade. IconBG is drawn
 * underneath. The mix is the panel's approximation of the client's.
 */
async function paint(icon: string, bg: string | null, colors: number[] | null): Promise<string> {
  const top = await load(`/api/icons/item/${icon}.png`);
  const canvas = document.createElement('canvas');
  canvas.width = top.naturalWidth;
  canvas.height = top.naturalHeight;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  if (!ctx) return top.src;
  if (bg) {
    try {
      ctx.drawImage(await load(`/api/icons/item/${bg}.png`), 0, 0, canvas.width, canvas.height);
    } catch {
      // No background icon exported.
    }
  }
  const layer = document.createElement('canvas');
  layer.width = canvas.width;
  layer.height = canvas.height;
  const lctx = layer.getContext('2d', { willReadFrequently: true });
  if (!lctx) return top.src;
  lctx.drawImage(top, 0, 0);
  if (colors && colors.length === 3) {
    const [c1, c2, c3] = colors.map((c) => [(c >> 16) & 255, (c >> 8) & 255, c & 255] as const) as [
      readonly [number, number, number],
      readonly [number, number, number],
      readonly [number, number, number],
    ];
    const data = lctx.getImageData(0, 0, layer.width, layer.height);
    const p = data.data;
    for (let i = 0; i < p.length; i += 4) {
      if (p[i + 3] === 0) continue;
      const r = p[i] ?? 0;
      const g = p[i + 1] ?? 0;
      const b = p[i + 2] ?? 0;
      if (Math.max(r, g, b) - Math.min(r, g, b) < 24) continue;
      for (let k = 0; k < 3; k++) {
        p[i + k] = Math.min(255, Math.round((r * c1[k]! + g * c2[k]! + b * c3[k]!) / 255));
      }
    }
    lctx.putImageData(data, 0, 0);
  }
  ctx.drawImage(layer, 0, 0);
  return canvas.toDataURL();
}

function usePainted(icon: string | null | undefined, bg: string | null | undefined, colors: number[] | null | undefined) {
  const [src, setSrc] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const key = icon ? `${icon}|${bg ?? ''}|${colors?.join(',') ?? ''}` : '';
  useEffect(() => {
    if (!icon) return;
    let live = true;
    let job = painted.get(key);
    if (!job) {
      job = paint(icon, bg ?? null, colors ?? null);
      painted.set(key, job);
    }
    job.then(
      (s) => live && setSrc(s),
      () => {
        painted.delete(key);
        if (live) setFailed(true);
      },
    );
    return () => {
      live = false;
    };
    // The key covers icon, bg and colors.
  }, [key]);
  return { src, failed };
}

/** The client's item icon (exported PNG), painted with the item's colors when it is a dye mask. */
export function ItemIcon({
  icon,
  bg,
  colors,
  size = 'md',
}: {
  icon?: string | null;
  bg?: string | null;
  colors?: number[] | null;
  size?: 'sm' | 'md' | 'lg';
}) {
  const { src, failed } = usePainted(icon, bg, colors);
  const box = size === 'sm' ? 'size-8' : size === 'lg' ? 'size-14' : 'size-11';
  if (!icon || failed) {
    return (
      <span aria-hidden className={cn('flex shrink-0 items-center justify-center rounded-md bg-raised text-muted', box)}>
        <Package className="size-4" />
      </span>
    );
  }
  return src ? (
    <img src={src} alt="" className={cn('shrink-0 rounded-md bg-raised object-contain', box)} />
  ) : (
    <span aria-hidden className={cn('shrink-0 rounded-md bg-raised', box)} />
  );
}
