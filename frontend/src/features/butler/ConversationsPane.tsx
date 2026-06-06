import { Loader2, MessageSquare } from 'lucide-react';

import { useRuns } from '@/features/cli/api';
import { cn } from '@/lib/cn';
import { groupConversations, type Conversation } from './conversations';
import { relativeTime } from './report';

export interface ConversationsPaneProps {
  /** The chat currently open in the composer (highlighted in the list). */
  activeConversationId: string | null;
  /** Re-open a past conversation (disabled mid-stream by the parent). */
  onSelect: (conversation: Conversation) => void;
  disabled?: boolean;
  className?: string;
}

/** The right-hand "previous conversations" pane of the butler home. */
export function ConversationsPane({
  activeConversationId,
  onSelect,
  disabled = false,
  className,
}: ConversationsPaneProps) {
  const { data: runs = [] } = useRuns();
  const conversations = groupConversations(runs);

  if (conversations.length === 0) return null;

  return (
    <aside aria-labelledby="conversations-heading" className={cn('flex flex-col gap-3', className)}>
      <h2
        id="conversations-heading"
        className="text-sm font-semibold uppercase tracking-wide text-muted-foreground"
      >
        Conversations
      </h2>
      <ul className="flex flex-col gap-1">
        {conversations.map((c) => {
          const active = c.id === activeConversationId;
          return (
            <li key={c.id}>
              <button
                type="button"
                onClick={() => onSelect(c)}
                disabled={disabled}
                aria-current={active ? 'true' : undefined}
                className={cn(
                  'flex w-full flex-col gap-1 rounded-md border px-3 py-2 text-left transition-colors',
                  active
                    ? 'border-primary/40 bg-primary/10'
                    : 'border-transparent hover:border-border hover:bg-accent/50',
                  disabled && 'opacity-60',
                )}
              >
                <span className="line-clamp-2 text-sm" title={c.title}>
                  {c.title}
                </span>
                <span className="flex items-center gap-2 text-xs text-muted-foreground">
                  {c.running ? (
                    <>
                      <Loader2
                        className="h-3 w-3 shrink-0 animate-spin text-primary"
                        aria-hidden="true"
                      />
                      <span className="font-medium text-primary-strong">running…</span>
                    </>
                  ) : (
                    <>
                      <MessageSquare className="h-3 w-3 shrink-0" aria-hidden="true" />
                      {c.runCount} message{c.runCount === 1 ? '' : 's'} ·{' '}
                      {relativeTime(c.updatedAt)}
                    </>
                  )}
                </span>
              </button>
            </li>
          );
        })}
      </ul>
    </aside>
  );
}
