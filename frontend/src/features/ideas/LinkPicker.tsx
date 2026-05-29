import { useState } from 'react';

import { Input } from '@/components/ui/input';
import { useIdeas } from './api';

interface Props {
  excludeIds: string[];
  onPick: (id: string) => void;
}

/** Search the owner's ideas and pick one to link (excludes self + already-linked). */
export function LinkPicker({ excludeIds, onPick }: Props) {
  const [q, setQ] = useState('');
  const { data: ideas = [] } = useIdeas({ status: null, tag: null, q });

  const candidates = ideas.filter((i) => !excludeIds.includes(i.id)).slice(0, 8);

  return (
    <div className="flex flex-col gap-2">
      <Input
        value={q}
        onChange={(e) => setQ(e.target.value)}
        placeholder="Search ideas to link…"
        aria-label="Search ideas to link"
      />
      {q.trim() !== '' && (
        <ul className="max-h-40 overflow-auto rounded-md border border-border">
          {candidates.length === 0 ? (
            <li className="px-3 py-2 text-sm text-muted-foreground">No matching ideas</li>
          ) : (
            candidates.map((idea) => (
              <li key={idea.id}>
                <button
                  type="button"
                  onClick={() => {
                    onPick(idea.id);
                    setQ('');
                  }}
                  className="w-full px-3 py-2 text-left text-sm hover:bg-muted"
                >
                  {idea.title}
                </button>
              </li>
            ))
          )}
        </ul>
      )}
    </div>
  );
}
