import { useEffect, useRef, useState } from 'react';
import { Send, Sparkles, SquarePen, X } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { Markdown } from '@/components/ui/markdown';
import { cn } from '@/lib/cn';
import { streamChat, useProviders } from '@/features/ai/api';
import type { ChatMessage } from '@/features/ai/types';

interface Props {
  open: boolean;
  onClose: () => void;
  /** What the AI is grounded on, e.g. "Whole canvas" or "Text part". */
  contextLabel: string;
  /** Grounding text sent as the chat `context`. */
  context: string;
  /** Whether the assistant's reply can be written back to a part. */
  canApply: boolean;
  onApply: (text: string) => void;
}

const SYSTEM =
  'You are an assistant embedded in a visual canvas ("superpage"). The user is composing parts ' +
  '(text, code, images, files, web pages, mindmaps, diagrams). Help them draft, rewrite, summarize, ' +
  'or reason about the canvas content provided as context. When asked to produce content for a part, ' +
  'reply with just that content (Markdown for text parts, raw code for code parts).';

export function CanvasAiPane({ open, onClose, contextLabel, context, canApply, onApply }: Props) {
  const providers = useProviders();
  const [providerId, setProviderId] = useState('mock');
  const [modelId, setModelId] = useState('mock-1');
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState('');
  const [streaming, setStreaming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const abortRef = useRef<AbortController | null>(null);

  // Abort any in-flight stream when the pane closes or the editor unmounts, so
  // we don't leak the fetch/connection or keep decoding into a hidden tree.
  useEffect(() => {
    if (!open) abortRef.current?.abort();
  }, [open]);
  useEffect(() => () => abortRef.current?.abort(), []);

  if (!open) return null;

  const provider = (providers.data ?? []).find((p) => p.id === providerId);
  const canUse = !provider || !provider.requires_key || provider.configured;
  const lastAssistant = [...messages].reverse().find((m) => m.role === 'assistant');

  const send = async () => {
    const prompt = input.trim();
    if (!prompt || streaming) return;
    setError(null);
    const history: ChatMessage[] = [...messages, { role: 'user', content: prompt }];
    setMessages([...history, { role: 'assistant', content: '' }]);
    setInput('');
    setStreaming(true);
    const controller = new AbortController();
    abortRef.current = controller;
    await streamChat(
      {
        provider: providerId,
        model: modelId,
        messages: history,
        system: SYSTEM,
        context: context || undefined,
      },
      {
        onDelta: (d) =>
          setMessages((prev) => {
            const next = prev.slice();
            const last = next[next.length - 1];
            if (last && last.role === 'assistant') next[next.length - 1] = { ...last, content: last.content + d };
            return next;
          }),
        onError: (m) => setError(m),
        signal: controller.signal,
      },
    );
    setStreaming(false);
    abortRef.current = null;
  };

  return (
    <aside className="fixed right-0 top-0 z-40 flex h-full w-[min(92vw,22rem)] flex-col border-l border-border bg-card shadow-xl">
      <div className="flex items-center gap-2 border-b border-border px-3 py-2">
        <Sparkles className="h-4 w-4 text-primary" aria-hidden />
        <div className="min-w-0">
          <p className="text-sm font-semibold">Canvas assistant</p>
          <p className="truncate text-xs text-muted-foreground">Context: {contextLabel}</p>
        </div>
        <button
          type="button"
          onClick={onClose}
          aria-label="Close assistant"
          className="ml-auto rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
        >
          <X className="h-4 w-4" />
        </button>
      </div>

      <div className="flex items-center gap-1 border-b border-border px-3 py-1.5 text-xs">
        <select
          className="rounded border border-border bg-background px-1 py-0.5"
          value={providerId}
          onChange={(e) => {
            const p = (providers.data ?? []).find((x) => x.id === e.target.value);
            setProviderId(e.target.value);
            if (p?.models[0]) setModelId(p.models[0].id);
          }}
          aria-label="Provider"
        >
          {(providers.data ?? []).map((p) => (
            <option key={p.id} value={p.id}>
              {p.label}
            </option>
          ))}
        </select>
        <select
          className="min-w-0 flex-1 rounded border border-border bg-background px-1 py-0.5"
          value={modelId}
          onChange={(e) => setModelId(e.target.value)}
          aria-label="Model"
        >
          {(provider?.models ?? [{ id: 'mock-1', label: 'mock-1', modalities: [] }]).map((m) => (
            <option key={m.id} value={m.id}>
              {m.label}
            </option>
          ))}
        </select>
      </div>

      <div className="min-h-0 flex-1 space-y-3 overflow-auto p-3">
        {messages.length === 0 && (
          <p className="text-sm text-muted-foreground">
            Ask the assistant to draft, rewrite, or summarize. It can see {contextLabel.toLowerCase()}.
          </p>
        )}
        {messages.map((m, i) => (
          <div
            key={i}
            className={cn(
              'rounded-lg px-3 py-2 text-sm',
              m.role === 'user' ? 'bg-primary/10' : 'bg-muted/50',
            )}
          >
            {m.content ? (
              <Markdown>{m.content}</Markdown>
            ) : (
              <span className="text-muted-foreground">…</span>
            )}
          </div>
        ))}
        {error && (
          <p className="text-sm text-danger" role="alert">
            {error}
          </p>
        )}
      </div>

      {canApply && lastAssistant?.content && !streaming && (
        <div className="border-t border-border px-3 py-2">
          <Button type="button" size="sm" variant="secondary" onClick={() => onApply(lastAssistant.content)}>
            <SquarePen className="h-3.5 w-3.5" aria-hidden /> Apply reply to part
          </Button>
        </div>
      )}

      <div className="border-t border-border p-2">
        {!canUse && (
          <p className="mb-1 text-xs text-muted-foreground">
            Add an API key in the AI page to use {provider?.label}. The mock provider works offline.
          </p>
        )}
        <div className="flex items-end gap-2">
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                void send();
              }
            }}
            placeholder="Ask…"
            rows={2}
            disabled={!canUse}
            className="min-h-0 flex-1 resize-none rounded-md border border-border bg-background p-2 text-sm"
          />
          {streaming ? (
            <Button type="button" size="sm" variant="ghost" onClick={() => abortRef.current?.abort()}>
              Stop
            </Button>
          ) : (
            <Button type="button" size="icon" onClick={() => void send()} disabled={!canUse || !input.trim()}>
              <Send className="h-4 w-4" aria-hidden />
            </Button>
          )}
        </div>
      </div>
    </aside>
  );
}
