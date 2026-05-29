import { User } from 'lucide-react';

/**
 * Placeholder account control. OAuth2 sign-in is the final phase; until then
 * this is a non-interactive slot so the header layout is auth-ready.
 */
export function UserMenu() {
  return (
    <button
      type="button"
      disabled
      title="Sign in — coming soon"
      aria-label="Account — sign in coming soon"
      className="grid h-9 w-9 place-items-center rounded-full bg-muted text-muted-foreground"
    >
      <User className="h-5 w-5" aria-hidden="true" />
    </button>
  );
}
