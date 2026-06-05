import { FileText, ImageOff } from 'lucide-react';

import { Markdown } from '@/components/ui/markdown';
import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/lib/cn';
import type { ResolvedBlockPayload } from './types';

function humanSize(bytes?: number | null): string {
  if (bytes == null) return '';
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let v = bytes / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`;
}

/**
 * Render stored, server-sanitized HTML in an isolated browsing context.
 * `sandbox=""` forbids scripts and same-origin access, matching the security
 * posture used for hosted-page previews — HTML never enters the app DOM.
 */
function HtmlFrame({ html, height }: { html: string; height: number | string }) {
  const srcDoc = `<!doctype html><html><head><meta charset="utf-8"><base target="_blank"><style>html,body{margin:0}body{font:14px/1.6 system-ui,-apple-system,Segoe UI,sans-serif;color:#1a1a1a;padding:12px}img,video,table{max-width:100%}a{color:#2563eb}</style></head><body>${
    html || '<p style="color:#888">Empty.</p>'
  }</body></html>`;
  return (
    <iframe
      title="Embedded content"
      sandbox=""
      srcDoc={srcDoc}
      className="w-full rounded-md border border-border bg-white"
      style={{ height }}
    />
  );
}

/** Read-only render of one embed's resolved payload. */
export function EmbedPreview({
  payload,
  height = 240,
  fill = false,
}: {
  payload?: ResolvedBlockPayload;
  height?: number;
  /** Stretch to the host's height (used inside resizable canvas cards). */
  fill?: boolean;
}) {
  if (!payload) {
    return (
      <div className="grid place-items-center py-6">
        <Spinner label="Loading preview" />
      </div>
    );
  }
  if (payload.missing) {
    return (
      <p className="text-sm text-danger">
        Item not found — it may have been deleted or is no longer shared.
      </p>
    );
  }

  switch (payload.item_type) {
    case 'idea':
      return (
        <div className={cn('overflow-auto', fill ? 'h-full' : 'max-h-72')}>
          {payload.body?.trim() ? (
            <Markdown>{payload.body}</Markdown>
          ) : (
            <p className="text-sm text-muted-foreground">This idea has no body yet.</p>
          )}
        </div>
      );

    case 'document':
    case 'page':
      return <HtmlFrame html={payload.body ?? ''} height={fill ? '100%' : height} />;

    case 'file':
      return (
        <div className="flex items-center gap-3 rounded-md border border-border bg-muted/30 p-3">
          <FileText className="h-8 w-8 shrink-0 text-muted-foreground" aria-hidden />
          <div className="min-w-0 text-sm">
            <p className="truncate font-medium">{payload.name ?? payload.title}</p>
            <p className="text-xs text-muted-foreground">
              {[payload.mime, humanSize(payload.size)].filter(Boolean).join(' · ')}
            </p>
          </div>
        </div>
      );

    case 'mindmap': {
      const nodes = payload.graph?.nodes ?? [];
      return (
        <div className="space-y-2">
          <p className="text-xs text-muted-foreground">{nodes.length} nodes</p>
          <div className="flex flex-wrap gap-1.5">
            {nodes.slice(0, 12).map((n) => (
              <span
                key={n.id}
                className="rounded-full border border-border bg-muted/40 px-2 py-0.5 text-xs"
              >
                {n.label || n.id}
              </span>
            ))}
            {nodes.length > 12 && (
              <span className="px-1 py-0.5 text-xs text-muted-foreground">
                +{nodes.length - 12} more
              </span>
            )}
            {nodes.length === 0 && (
              <span className="text-sm text-muted-foreground">Empty mindmap.</span>
            )}
          </div>
        </div>
      );
    }

    case 'diagram':
      return payload.preview ? (
        <div
          className={cn(
            'grid place-items-center overflow-auto rounded-md border border-border bg-white p-2',
            fill ? 'h-full' : 'max-h-72',
          )}
        >
          <img
            src={payload.preview}
            alt={payload.title ?? 'Diagram'}
            className="max-h-64 max-w-full object-contain"
          />
        </div>
      ) : (
        <div className="flex items-center gap-2 rounded-md border border-dashed border-border p-4 text-sm text-muted-foreground">
          <ImageOff className="h-4 w-4" aria-hidden />
          {payload.has_xml ? 'No preview rendered yet — open to draw.' : 'Empty diagram.'}
        </div>
      );

    default:
      return <p className="text-sm text-muted-foreground">{payload.title ?? 'Unknown item.'}</p>;
  }
}
