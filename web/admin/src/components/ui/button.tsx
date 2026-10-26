import { cva, type VariantProps } from 'class-variance-authority';
import type { ButtonHTMLAttributes } from 'react';
import { cn } from '@/lib/utils';

const button = cva(
  'inline-flex items-center justify-center gap-2 rounded-lg text-sm font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent disabled:pointer-events-none disabled:opacity-50 [&_svg]:size-4',
  {
    variants: {
      variant: {
        default: 'bg-accent text-accent-fg hover:brightness-105',
        outline: 'border border-border bg-surface hover:bg-raised',
        ghost: 'text-muted hover:bg-raised hover:text-fg',
        danger: 'border border-border bg-surface text-danger hover:bg-danger-soft',
      },
      size: { default: 'h-9 px-4', sm: 'h-8 px-3', lg: 'h-11 px-4' },
    },
    defaultVariants: { variant: 'default', size: 'default' },
  },
);

export function Button({
  className,
  variant,
  size,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & VariantProps<typeof button>) {
  return <button className={cn(button({ variant, size }), className)} {...props} />;
}
