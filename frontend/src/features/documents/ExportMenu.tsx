import { useState } from 'react';
import { ChevronDown, Download } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { exportDownload } from './api';
import type { ExportFormat, ExportSource } from './types';

interface Props {
  content: string;
  source: ExportSource;
  filename: string;
}

const FORMATS: { target: ExportFormat; label: string }[] = [
  { target: 'pdf', label: 'PDF' },
  { target: 'docx', label: 'Word (.docx)' },
  { target: 'html', label: 'HTML' },
  { target: 'markdown', label: 'Markdown' },
];

/** Reusable export dropdown. Calls the shared `POST /export` pathway. */
export function ExportMenu({ content, source, filename }: Props) {
  const [open, setOpen] = useState(false);
  const [pending, setPending] = useState<ExportFormat | null>(null);
  const [error, setError] = useState<string | null>(null);

  const run = async (target: ExportFormat) => {
    setOpen(false);
    setError(null);
    setPending(target);
    try {
      await exportDownload({ content, source, target, filename });
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Export failed');
    } finally {
      setPending(null);
    }
  };

  return (
    <div className="relative">
      <Button variant="outline" onClick={() => setOpen((v) => !v)} disabled={pending !== null}>
        <Download className="h-4 w-4" aria-hidden="true" />
        {pending ? `Exporting ${pending}…` : 'Export'}
        <ChevronDown className="h-4 w-4" aria-hidden="true" />
      </Button>

      {open && (
        <>
          <button
            type="button"
            className="fixed inset-0 z-10 cursor-default"
            aria-label="Close export menu"
            onClick={() => setOpen(false)}
          />
          <ul className="absolute right-0 z-20 mt-1 w-44 overflow-hidden rounded-md border border-border bg-card shadow-lg">
            {FORMATS.map((f) => (
              <li key={f.target}>
                <button
                  type="button"
                  onClick={() => void run(f.target)}
                  className="w-full px-3 py-2 text-left text-sm hover:bg-muted"
                >
                  {f.label}
                </button>
              </li>
            ))}
          </ul>
        </>
      )}

      {error && (
        <p className="absolute right-0 top-full mt-1 w-64 rounded-md border border-danger/30 bg-danger/10 px-2 py-1 text-xs text-danger" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
