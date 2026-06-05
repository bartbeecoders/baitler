import { useEffect, useState } from 'react';
import { Download, ExternalLink, FileText, ImageOff } from 'lucide-react';

import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/lib/cn';
import { fetchContent } from '@/features/files/api';
import type { MindmapGraph } from '@/features/mindmaps/types';
import { humanSize, safeUrl } from './partUtil';

/**
 * Render stored, server-sanitized HTML in an isolated browsing context.
 * `sandbox=""` forbids scripts and same-origin access — HTML never enters the
 * app DOM (matches the hosted-page preview posture).
 */
export function HtmlFrame({ html }: { html: string }) {
  const srcDoc = `<!doctype html><html><head><meta charset="utf-8"><base target="_blank"><style>html,body{margin:0}body{font:14px/1.6 system-ui,-apple-system,Segoe UI,sans-serif;color:#1a1a1a;padding:12px}img,video,table{max-width:100%}a{color:#2563eb}</style></head><body>${
    html || '<p style="color:#888">Empty.</p>'
  }</body></html>`;
  return (
    <iframe
      title="Embedded content"
      sandbox=""
      srcDoc={srcDoc}
      className="h-full w-full rounded-md border border-border bg-white"
    />
  );
}

/**
 * Embed an arbitrary external page. Sandboxed WITHOUT `allow-same-origin`, so
 * the frame runs in an opaque origin (can't touch our cookies/APIs). Many sites
 * block framing via X-Frame-Options/CSP — we always offer a fallback link.
 */
export function ExternalFrame({ url, title }: { url: string; title?: string }) {
  const safe = safeUrl(url);
  if (!safe) {
    return (
      <p className="text-sm text-danger">Only http(s) URLs can be embedded.</p>
    );
  }
  return (
    <div className="flex h-full flex-col gap-1">
      <iframe
        src={safe}
        title={title || 'External page'}
        sandbox="allow-scripts allow-popups allow-popups-to-escape-sandbox allow-forms"
        referrerPolicy="no-referrer"
        loading="lazy"
        allow=""
        className="min-h-0 flex-1 rounded-md border border-border bg-white"
      />
      <a
        href={safe}
        target="_blank"
        rel="noreferrer noopener"
        className="inline-flex items-center gap-1 self-start text-xs text-muted-foreground hover:text-foreground"
      >
        <ExternalLink className="h-3 w-3" aria-hidden /> Open in new tab
      </a>
    </div>
  );
}

export function FileCard({
  name,
  mime,
  size,
  onDownload,
}: {
  name: string;
  mime?: string | null;
  size?: number | null;
  onDownload?: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onDownload}
      disabled={!onDownload}
      className={cn(
        'flex w-full items-center gap-3 rounded-md border border-border bg-muted/30 p-3 text-left',
        onDownload && 'hover:bg-muted',
      )}
    >
      <FileText className="h-8 w-8 shrink-0 text-muted-foreground" aria-hidden />
      <div className="min-w-0 text-sm">
        <p className="truncate font-medium">{name}</p>
        <p className="text-xs text-muted-foreground">
          {[mime, humanSize(size)].filter(Boolean).join(' · ')}
        </p>
      </div>
      {onDownload && <Download className="ml-auto h-4 w-4 shrink-0 text-muted-foreground" aria-hidden />}
    </button>
  );
}

export function MindmapChips({ graph }: { graph?: MindmapGraph }) {
  const nodes = graph?.nodes ?? [];
  return (
    <div className="space-y-2">
      <p className="text-xs text-muted-foreground">{nodes.length} nodes</p>
      <div className="flex flex-wrap gap-1.5">
        {nodes.slice(0, 16).map((n) => (
          <span
            key={n.id}
            className="rounded-full border border-border bg-muted/40 px-2 py-0.5 text-xs"
          >
            {n.label || n.id}
          </span>
        ))}
        {nodes.length > 16 && (
          <span className="px-1 py-0.5 text-xs text-muted-foreground">+{nodes.length - 16}</span>
        )}
        {nodes.length === 0 && <span className="text-sm text-muted-foreground">Empty mindmap.</span>}
      </div>
    </div>
  );
}

export function DiagramImg({
  preview,
  hasXml,
  title,
}: {
  preview?: string | null;
  hasXml?: boolean;
  title?: string | null;
}) {
  if (preview) {
    return (
      <div className="grid h-full place-items-center overflow-auto rounded-md border border-border bg-white p-2">
        <img src={preview} alt={title ?? 'Diagram'} className="max-h-full max-w-full object-contain" />
      </div>
    );
  }
  return (
    <div className="flex items-center gap-2 rounded-md border border-dashed border-border p-4 text-sm text-muted-foreground">
      <ImageOff className="h-4 w-4" aria-hidden />
      {hasXml ? 'No preview rendered yet — open to draw.' : 'Empty diagram.'}
    </div>
  );
}

/** Load an uploaded file's bytes via the credentialed API and show it as an image. */
export function FileImage({ fileId, alt }: { fileId: string; alt?: string }) {
  const [url, setUrl] = useState<string | null>(null);
  const [error, setError] = useState(false);

  // Mounted fresh per file id (the caller keys on it), so no in-effect reset is
  // needed — state starts null and is only set from the async result.
  useEffect(() => {
    let revoked: string | null = null;
    let cancelled = false;
    fetchContent(fileId)
      .then((blob) => {
        if (cancelled) return;
        const objectUrl = URL.createObjectURL(blob);
        revoked = objectUrl;
        setUrl(objectUrl);
      })
      .catch(() => !cancelled && setError(true));
    return () => {
      cancelled = true;
      if (revoked) URL.revokeObjectURL(revoked);
    };
  }, [fileId]);

  if (error) return <p className="text-sm text-danger">Could not load image.</p>;
  if (!url)
    return (
      <div className="grid h-full place-items-center">
        <Spinner label="Loading image" />
      </div>
    );
  return (
    <div className="grid h-full place-items-center overflow-hidden">
      <img src={url} alt={alt ?? ''} className="max-h-full max-w-full object-contain" />
    </div>
  );
}
