import { Link2 } from 'lucide-react';

import { StatusBadge } from './StatusBadge';
import type { Idea } from './types';

interface Props {
  idea: Idea;
  onOpen: () => void;
  showStatus?: boolean;
}

/** Strip common Markdown punctuation for a plain-text snippet. */
function snippet(body: string): string {
  return body
    .replace(/[#*`>_~[\]()-]/g, '')
    .replace(/\s+/g, ' ')
    .trim()
    .slice(0, 140);
}

export function IdeaCard({ idea, onOpen, showStatus = true }: Props) {
  const preview = snippet(idea.body);
  return (
    <button
      type="button"
      onClick={onOpen}
      aria-label={`Open idea ${idea.title}`}
      className="flex w-full flex-col gap-2 rounded-lg border border-border bg-card p-4 text-left transition-colors hover:border-primary/50"
    >
      <div className="flex items-start justify-between gap-2">
        <h3 className="font-medium leading-tight">{idea.title}</h3>
        {showStatus && <StatusBadge status={idea.status} />}
      </div>
      {preview && <p className="line-clamp-2 text-sm text-muted-foreground">{preview}</p>}
      {(idea.tags.length > 0 || idea.links.length > 0) && (
        <div className="flex flex-wrap items-center gap-1">
          {idea.tags.map((tag) => (
            <span
              key={tag}
              className="rounded bg-accent px-1.5 py-0.5 text-xs text-accent-foreground"
            >
              {tag}
            </span>
          ))}
          {idea.links.length > 0 && (
            <span className="ml-auto inline-flex items-center gap-1 text-xs text-muted-foreground">
              <Link2 className="h-3 w-3" aria-hidden="true" />
              {idea.links.length}
            </span>
          )}
        </div>
      )}
    </button>
  );
}
