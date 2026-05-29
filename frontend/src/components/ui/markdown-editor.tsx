import { useState } from 'react';

import { cn } from '@/lib/cn';
import { Markdown } from './markdown';
import { Textarea } from './textarea';

interface Props {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  rows?: number;
}

/** A Markdown textarea with Write/Preview tabs. Reused by the document editor. */
export function MarkdownEditor({ value, onChange, placeholder, rows = 12 }: Props) {
  const [tab, setTab] = useState<'write' | 'preview'>('write');

  return (
    <div className="rounded-md border border-border">
      <div className="flex gap-1 border-b border-border p-1">
        <TabButton active={tab === 'write'} onClick={() => setTab('write')}>
          Write
        </TabButton>
        <TabButton active={tab === 'preview'} onClick={() => setTab('preview')}>
          Preview
        </TabButton>
      </div>
      {tab === 'write' ? (
        <Textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          rows={rows}
          aria-label="Markdown content"
          className="resize-y rounded-none border-0 font-mono focus-visible:outline-none"
        />
      ) : (
        <div className="min-h-40 p-3">
          {value.trim() ? (
            <Markdown>{value}</Markdown>
          ) : (
            <p className="text-sm text-muted-foreground">Nothing to preview.</p>
          )}
        </div>
      )}
    </div>
  );
}

function TabButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={active}
      className={cn(
        'rounded px-3 py-1 text-sm font-medium',
        active ? 'bg-muted text-foreground' : 'text-muted-foreground hover:bg-muted/60',
      )}
    >
      {children}
    </button>
  );
}
