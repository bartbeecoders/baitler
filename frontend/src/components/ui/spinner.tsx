import { cn } from '@/lib/cn';

/** Accessible loading spinner. Pass `label` to contextualize the status text. */
export function Spinner({ className, label = 'Loading' }: { className?: string; label?: string }) {
  return (
    <span role="status" aria-label={label}>
      <span
        className={cn(
          'block h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent',
          className,
        )}
      />
    </span>
  );
}
