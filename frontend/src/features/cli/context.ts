import { navItems } from '@/config/navigation';

/** Resolve the current route to a human page label + an orienting context string
 * for the agent (so it acts on content relevant to what the user is viewing). */
export function pageContext(pathname: string): { label: string; context: string } {
  const match = navItems
    .filter((i) => i.path !== '/' && (pathname === i.path || pathname.startsWith(`${i.path}/`)))
    .sort((a, b) => b.path.length - a.path.length)[0];
  const label = match?.label ?? 'Butler';
  const context =
    `You are the Baitler agent, embedded in the app. The user is currently viewing the ` +
    `"${label}" page. When a request is ambiguous, prefer acting on content related to ${label}. ` +
    `Use the Baitler MCP tools (knowledge_search, ideas_*, documents_*, files_*, pages_*, ` +
    `projects_*) to read and update their data; your writes are saved as drafts for review.`;
  return { label, context };
}
