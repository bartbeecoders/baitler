import { Bot, Menu } from 'lucide-react';
import { useLocation } from 'react-router-dom';

import { Button } from '@/components/ui/button';
import { useAgentDock } from '@/stores/agentDock';
import { ApiStatusBadge } from './ApiStatusBadge';
import { ThemeToggle } from './ThemeToggle';
import { UserMenu } from './UserMenu';

/** Top bar: mobile menu trigger, live API status, agent-pane toggle, theme, account. */
export function Header({ onMenuClick }: { onMenuClick: () => void }) {
  const dockOpen = useAgentDock((s) => s.open);
  const toggleDock = useAgentDock((s) => s.toggle);
  const { pathname } = useLocation();

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

      {/* The standalone /agent route already shows the panel full-width. */}
      {pathname !== '/agent' && (
        <Button
          variant={dockOpen ? 'secondary' : 'ghost'}
          size="icon"
          onClick={toggleDock}
          aria-label="Toggle agent pane"
          aria-pressed={dockOpen}
        >
          <Bot className="h-5 w-5" />
        </Button>
      )}

      <ThemeToggle />
      <UserMenu />
    </header>
  );
}
