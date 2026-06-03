import { AgentPanel } from './AgentPanel';

/** Full-page Agent console (the `/agent` route). The same panel also docks as a
 * right-hand pane on every feature page — see `AgentDock`. */
export function AgentPage() {
  return <AgentPanel />;
}
