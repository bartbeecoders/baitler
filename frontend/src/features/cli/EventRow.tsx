import { Bot, Wrench } from 'lucide-react';

import { Badge } from '@/components/ui/badge';
import { Markdown } from '@/components/ui/markdown';
import type { RunEvent } from './types';

/** Render one streamed run event (assistant prose, tool use/result, terminal). */
export function EventRow({ event }: { event: RunEvent }) {
  switch (event.type) {
    case 'init':
      return (
        <p className="text-xs text-muted-foreground">
          Session started{event.model ? ` · ${event.model}` : ''}
        </p>
      );
    case 'assistant':
      return (
        <div className="flex gap-2">
          <Bot className="mt-0.5 h-4 w-4 shrink-0 text-primary" aria-hidden="true" />
          <div className="min-w-0 text-sm">
            <Markdown>{event.text}</Markdown>
          </div>
        </div>
      );
    case 'tool_use':
      return (
        <div className="flex items-center gap-2 text-xs">
          <Wrench className="h-3.5 w-3.5 text-muted-foreground" aria-hidden="true" />
          <span className="font-mono">{event.name}</span>
          {event.summary && <span className="text-muted-foreground">— {event.summary}</span>}
        </div>
      );
    case 'tool_result':
      return (
        <div className="pl-5 text-xs text-muted-foreground">
          <Badge variant={event.ok ? 'success' : 'danger'}>{event.ok ? 'ok' : 'error'}</Badge>{' '}
          {event.summary}
        </div>
      );
    case 'result':
      return (
        <p className="text-xs text-muted-foreground">
          Done · {event.num_turns} turn{event.num_turns === 1 ? '' : 's'}
          {event.cost_usd != null ? ` · $${event.cost_usd.toFixed(4)}` : ''}
        </p>
      );
    case 'error':
      return (
        <p className="rounded-md border border-danger/30 bg-danger/10 px-2 py-1 text-xs text-danger">
          {event.message}
        </p>
      );
    default:
      return null;
  }
}
