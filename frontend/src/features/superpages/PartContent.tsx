import { useRef, useState } from 'react';
import { ExternalLink, Sparkles, Upload } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Markdown } from '@/components/ui/markdown';
import { MarkdownEditor } from '@/components/ui/markdown-editor';
import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/lib/cn';
import { EmbedPreview } from './EmbedPreview';
import {
  DiagramImg,
  ExternalFrame,
  FileCard,
  FileImage,
  HtmlFrame,
  MindmapChips,
} from './partRender';
import { safeUrl } from './partUtil';
import type { EmbedItemType, ResolvedBlockPayload, SuperpageBlock } from './types';

/** Per-block actions, bound to a single block by the canvas editor. */
export interface PartActions {
  pick: (itemType: EmbedItemType) => void;
  upload: (file: File, asImage: boolean) => void;
  generateImage: (prompt: string) => void;
  createItem: (type: 'mindmap' | 'diagram' | 'page') => void;
  open: () => void;
  download: () => void;
  busy: boolean;
}

interface Props {
  block: SuperpageBlock;
  payload?: ResolvedBlockPayload;
  editing: boolean;
  onUpdate: (patch: Partial<SuperpageBlock>) => void;
  actions: PartActions;
}

export function PartContent({ block, payload, editing, onUpdate, actions }: Props) {
  switch (block.kind) {
    case 'text':
      return editing ? (
        <MarkdownEditor
          value={block.markdown ?? ''}
          onChange={(v) => onUpdate({ markdown: v })}
          placeholder="Write… (Markdown). The AI can draft or rewrite this."
          rows={6}
        />
      ) : block.markdown?.trim() ? (
        <Markdown>{block.markdown}</Markdown>
      ) : (
        <Empty>Empty text.</Empty>
      );

    case 'code':
      return <CodePart block={block} editing={editing} onUpdate={onUpdate} />;

    case 'image':
      return <ImagePart block={block} editing={editing} onUpdate={onUpdate} actions={actions} />;

    case 'file':
      return <FilePart block={block} payload={payload} editing={editing} actions={actions} />;

    case 'webpage':
      return (
        <WebpagePart
          block={block}
          payload={payload}
          editing={editing}
          onUpdate={onUpdate}
          actions={actions}
        />
      );

    case 'mindmap':
      return (
        <RefPart
          hasRef={Boolean(block.item_id)}
          payload={payload}
          editing={editing}
          empty="mindmap"
          actions={actions}
          itemType="mindmap"
        >
          <MindmapChips graph={payload?.graph} />
        </RefPart>
      );

    case 'diagram':
      return (
        <RefPart
          hasRef={Boolean(block.item_id)}
          payload={payload}
          editing={editing}
          empty="diagram"
          actions={actions}
          itemType="diagram"
        >
          <DiagramImg preview={payload?.preview} hasXml={payload?.has_xml} title={payload?.title} />
        </RefPart>
      );

    // legacy v1 kinds
    case 'note':
      return block.markdown?.trim() ? <Markdown>{block.markdown}</Markdown> : <Empty>Empty note.</Empty>;
    case 'heading':
      return <h2 className="text-lg font-semibold">{block.text || 'Section'}</h2>;
    case 'embed':
      return <EmbedPreview payload={payload} fill />;
    default:
      return <Empty>Unsupported part.</Empty>;
  }
}

function Empty({ children }: { children: React.ReactNode }) {
  return <p className="text-sm text-muted-foreground">{children}</p>;
}

function TitleBar({ title, onOpen }: { title?: string | null; onOpen?: () => void }) {
  if (!title && !onOpen) return null;
  return (
    <div className="flex items-center justify-between gap-2">
      <p className="truncate text-sm font-medium">{title ?? 'Untitled'}</p>
      {onOpen && (
        <button
          type="button"
          onClick={onOpen}
          aria-label="Open in its editor"
          className="shrink-0 text-muted-foreground hover:text-foreground"
        >
          <ExternalLink className="h-3.5 w-3.5" />
        </button>
      )}
    </div>
  );
}

function CodePart({
  block,
  editing,
  onUpdate,
}: {
  block: SuperpageBlock;
  editing: boolean;
  onUpdate: (patch: Partial<SuperpageBlock>) => void;
}) {
  if (editing) {
    return (
      <div className="flex h-full flex-col gap-1.5">
        <Input
          value={block.lang ?? ''}
          onChange={(e) => onUpdate({ lang: e.target.value })}
          placeholder="language (e.g. ts, rust)"
          className="h-7 max-w-[12rem] text-xs"
          data-no-drag
        />
        <textarea
          value={block.text ?? ''}
          onChange={(e) => onUpdate({ text: e.target.value })}
          placeholder="// code"
          spellCheck={false}
          className="min-h-0 flex-1 resize-none rounded-md border border-border bg-background p-2 font-mono text-xs"
          data-no-drag
        />
      </div>
    );
  }
  return (
    <div className="flex h-full flex-col overflow-hidden">
      {block.lang && (
        <span className="mb-1 inline-block self-start rounded bg-muted px-1.5 py-0.5 text-[0.65rem] uppercase tracking-wide text-muted-foreground">
          {block.lang}
        </span>
      )}
      <pre className="min-h-0 flex-1 overflow-auto rounded-md bg-muted/50 p-3 text-xs">
        <code className="font-mono">{block.text || '// empty'}</code>
      </pre>
    </div>
  );
}

function ImagePart({
  block,
  editing,
  onUpdate,
  actions,
}: {
  block: SuperpageBlock;
  editing: boolean;
  onUpdate: (patch: Partial<SuperpageBlock>) => void;
  actions: PartActions;
}) {
  const fileRef = useRef<HTMLInputElement | null>(null);
  const [mode, setMode] = useState<'url' | 'upload' | 'generate'>(
    block.src === 'upload' ? 'upload' : 'url',
  );
  const [prompt, setPrompt] = useState('');
  const hasImg = block.src === 'upload' ? Boolean(block.item_id) : Boolean(block.url);

  if (hasImg) {
    const content =
      block.src === 'upload' && block.item_id ? (
        <FileImage key={block.item_id} fileId={block.item_id} alt={block.text ?? ''} />
      ) : (
        (() => {
          const src = safeUrl(block.url, true);
          return src ? (
            <div className="grid h-full place-items-center overflow-hidden">
              <img src={src} alt={block.text ?? ''} className="max-h-full max-w-full object-contain" />
            </div>
          ) : (
            <Empty>Invalid image URL.</Empty>
          );
        })()
      );
    return (
      <div className="flex h-full flex-col gap-1.5">
        <div className="min-h-0 flex-1">{content}</div>
        {editing ? (
          <Input
            value={block.text ?? ''}
            onChange={(e) => onUpdate({ text: e.target.value })}
            placeholder="caption (optional)"
            className="h-7 text-xs"
            data-no-drag
          />
        ) : (
          block.text && <p className="text-center text-xs text-muted-foreground">{block.text}</p>
        )}
        {editing && (
          <button
            type="button"
            onClick={() => onUpdate({ src: 'url', item_id: undefined, url: undefined })}
            className="self-start text-xs text-muted-foreground hover:text-foreground"
            data-no-drag
          >
            Replace image
          </button>
        )}
      </div>
    );
  }

  if (!editing) return <Empty>No image.</Empty>;

  return (
    <div className="flex h-full flex-col gap-2" data-no-drag>
      <div className="flex gap-1">
        {(['url', 'upload', 'generate'] as const).map((m) => (
          <button
            key={m}
            type="button"
            onClick={() => setMode(m)}
            className={cn(
              'rounded px-2 py-0.5 text-xs capitalize',
              mode === m ? 'bg-muted font-medium' : 'text-muted-foreground hover:bg-muted/60',
            )}
          >
            {m}
          </button>
        ))}
      </div>

      {mode === 'url' && (
        <Input
          autoFocus
          value={block.url ?? ''}
          onChange={(e) => onUpdate({ src: 'url', url: e.target.value })}
          placeholder="https://…/image.png"
          className="text-xs"
        />
      )}

      {mode === 'upload' && (
        <>
          <input
            ref={fileRef}
            type="file"
            accept="image/*"
            hidden
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) actions.upload(f, true);
              e.target.value = '';
            }}
          />
          <Button
            type="button"
            size="sm"
            variant="secondary"
            onClick={() => fileRef.current?.click()}
            disabled={actions.busy}
          >
            <Upload className="h-3.5 w-3.5" aria-hidden /> Choose image…
          </Button>
        </>
      )}

      {mode === 'generate' && (
        <div className="flex flex-col gap-1.5">
          <textarea
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            placeholder="Describe the image…"
            rows={2}
            className="resize-none rounded-md border border-border bg-background p-2 text-xs"
          />
          <Button
            type="button"
            size="sm"
            variant="primary"
            disabled={actions.busy || !prompt.trim()}
            onClick={() => actions.generateImage(prompt.trim())}
          >
            {actions.busy ? <Spinner label="Generating" /> : <Sparkles className="h-3.5 w-3.5" aria-hidden />}
            Generate
          </Button>
        </div>
      )}
    </div>
  );
}

function FilePart({
  block,
  payload,
  editing,
  actions,
}: {
  block: SuperpageBlock;
  payload?: ResolvedBlockPayload;
  editing: boolean;
  actions: PartActions;
}) {
  const fileRef = useRef<HTMLInputElement | null>(null);

  if (block.item_id) {
    if (payload?.missing) return <Empty>File not found — it may have been deleted.</Empty>;
    return (
      <FileCard
        name={payload?.name ?? payload?.title ?? 'File'}
        mime={payload?.mime}
        size={payload?.size}
        onDownload={actions.download}
      />
    );
  }
  if (!editing) return <Empty>No file.</Empty>;
  return (
    <div className="flex flex-col gap-2" data-no-drag>
      <input
        ref={fileRef}
        type="file"
        hidden
        onChange={(e) => {
          const f = e.target.files?.[0];
          if (f) actions.upload(f, false);
          e.target.value = '';
        }}
      />
      <Button
        type="button"
        size="sm"
        variant="secondary"
        onClick={() => fileRef.current?.click()}
        disabled={actions.busy}
      >
        <Upload className="h-3.5 w-3.5" aria-hidden /> Upload…
      </Button>
      <Button type="button" size="sm" variant="ghost" onClick={() => actions.pick('file')}>
        Pick existing file
      </Button>
    </div>
  );
}

function WebpagePart({
  block,
  payload,
  editing,
  onUpdate,
  actions,
}: {
  block: SuperpageBlock;
  payload?: ResolvedBlockPayload;
  editing: boolean;
  onUpdate: (patch: Partial<SuperpageBlock>) => void;
  actions: PartActions;
}) {
  const wk = block.web_kind ?? 'url';

  // Resolved content states.
  if (wk === 'url' && block.url && !editing) {
    return <ExternalFrame url={block.url} title="Web page" />;
  }
  if (wk === 'page' && block.item_id) {
    if (payload?.missing) return <Empty>Page not found.</Empty>;
    return (
      <div className="flex h-full flex-col gap-1.5">
        <TitleBar title={payload?.title} onOpen={actions.open} />
        <div className="min-h-0 flex-1">
          <HtmlFrame html={payload?.body ?? ''} />
        </div>
      </div>
    );
  }

  if (!editing) {
    return wk === 'url' && block.url ? <ExternalFrame url={block.url} /> : <Empty>No web page.</Empty>;
  }

  // Editing — setup / switch.
  return (
    <div className="flex h-full flex-col gap-2" data-no-drag>
      <div className="flex gap-1">
        {(['url', 'page'] as const).map((k) => (
          <button
            key={k}
            type="button"
            onClick={() => onUpdate({ web_kind: k })}
            className={cn(
              'rounded px-2 py-0.5 text-xs',
              wk === k ? 'bg-muted font-medium' : 'text-muted-foreground hover:bg-muted/60',
            )}
          >
            {k === 'url' ? 'External URL' : 'Baitler page'}
          </button>
        ))}
      </div>

      {wk === 'url' ? (
        <>
          <Input
            value={block.url ?? ''}
            onChange={(e) => onUpdate({ url: e.target.value })}
            placeholder="https://example.com"
            className="text-xs"
          />
          {block.url && (
            <div className="min-h-0 flex-1">
              <ExternalFrame url={block.url} />
            </div>
          )}
        </>
      ) : block.item_id ? (
        <div className="flex h-full flex-col gap-1.5">
          <div className="flex items-center justify-between gap-2">
            <TitleBar title={payload?.title} onOpen={actions.open} />
            <button
              type="button"
              onClick={() => actions.pick('page')}
              className="shrink-0 text-xs text-muted-foreground hover:text-foreground"
            >
              Change
            </button>
          </div>
          <div className="min-h-0 flex-1">
            <HtmlFrame html={payload?.body ?? ''} />
          </div>
        </div>
      ) : (
        <div className="flex gap-2">
          <Button type="button" size="sm" variant="secondary" onClick={() => actions.pick('page')}>
            Pick page
          </Button>
          <Button
            type="button"
            size="sm"
            variant="ghost"
            disabled={actions.busy}
            onClick={() => actions.createItem('page')}
          >
            New page
          </Button>
        </div>
      )}
    </div>
  );
}

/** Shared render for mindmap/diagram parts: reference-or-setup. */
function RefPart({
  hasRef,
  payload,
  editing,
  empty,
  itemType,
  actions,
  children,
}: {
  hasRef: boolean;
  payload?: ResolvedBlockPayload;
  editing: boolean;
  empty: string;
  itemType: 'mindmap' | 'diagram';
  actions: PartActions;
  children: React.ReactNode;
}) {
  if (hasRef) {
    if (payload?.missing) return <Empty>{`${empty} not found.`}</Empty>;
    if (!payload)
      return (
        <div className="grid h-full place-items-center">
          <Spinner label="Loading" />
        </div>
      );
    return (
      <div className="flex h-full flex-col gap-1.5">
        <TitleBar title={payload.title} onOpen={actions.open} />
        <div className="min-h-0 flex-1">{children}</div>
      </div>
    );
  }
  if (!editing) return <Empty>{`No ${empty}.`}</Empty>;
  return (
    <div className="flex gap-2" data-no-drag>
      <Button
        type="button"
        size="sm"
        variant="primary"
        disabled={actions.busy}
        onClick={() => actions.createItem(itemType)}
      >
        New {empty}
      </Button>
      <Button type="button" size="sm" variant="secondary" onClick={() => actions.pick(itemType)}>
        Pick existing
      </Button>
    </div>
  );
}
