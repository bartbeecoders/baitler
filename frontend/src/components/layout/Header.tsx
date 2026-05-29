import { Menu } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { ApiStatusBadge } from './ApiStatusBadge';
import { ThemeToggle } from './ThemeToggle';
import { UserMenu } from './UserMenu';

/** Top bar: mobile menu trigger, live API status, theme toggle, account slot. */
export function Header({ onMenuClick }: { onMenuClick: () => void }) {
  return (
    <header className="sticky top-0 z-20 flex h-16 items-center gap-3 border-b border-border bg-background/80 px-4 backdrop-blur sm:px-6 lg:px-8">
      <Button
        variant="ghost"
        size="icon"
        className="md:hidden"
        onClick={onMenuClick}
        aria-label="Open navigation"
      >
        <Menu className="h-5 w-5" />
      </Button>

      <div className="flex flex-1 items-center">
        <ApiStatusBadge />
      </div>

      <ThemeToggle />
      <UserMenu />
    </header>
  );
}
