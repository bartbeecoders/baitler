import type { InputHTMLAttributes } from 'react';

import { cn } from '@/lib/cn';

export function Input({ className, ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      className={cn(
        'h-10 w-full rounded-md border border-input bg-background px-3 text-sm',
        'placeholder:text-muted-foreground disabled:opacity-50',
        className,
      )}
      {...props}
    />
  );
}
