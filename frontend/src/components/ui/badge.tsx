import type { HTMLAttributes } from 'react';

import { cn } from '@/lib/cn';

type Variant = 'default' | 'success' | 'warning' | 'danger' | 'muted';

const variantClasses: Record<Variant, string> = {
  default: 'bg-accent text-accent-foreground',
  success: 'bg-success/15 text-success',
  warning: 'bg-primary/15 text-primary-strong',
  danger: 'bg-danger/15 text-danger',
  muted: 'bg-muted text-muted-foreground',
};

export interface BadgeProps extends HTMLAttributes<HTMLSpanElement> {
  variant?: Variant;
}

export function Badge({ className, variant = 'default', ...props }: BadgeProps) {
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-medium',
        variantClasses[variant],
        className,
      )}
      {...props}
    />
  );
}
