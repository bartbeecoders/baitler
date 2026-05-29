import { useRef, useState, type FormEvent } from 'react';
import { Send, Settings, Square } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { Markdown } from '@/components/ui/markdown';
import { Spinner } from '@/components/ui/spinner';
import { Textarea } from '@/components/ui/textarea';
import { cn } from '@/lib/cn';
import { streamChat, useProviders } from './api';
import { KeySettingsModal } from './KeySettingsModal';
import type { ChatMessage } from './types';

const SELECT_CLASS = 'h-10 rounded-md border border-input bg-background px-2 text-sm';

export function AiPage() {
  const { data: providers = [] } = useProviders();
  const [providerId, setProviderId] = useState('mock');
  const [modelId, setModelId] = useState('mock-1');
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState('');
  const [streaming, setStreaming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const abortRef = useRef<AbortController | null>(null);

  const provider = providers.find((p) => p.id === providerId);
  const canUse = provider ? !provider.requires_key || provider.configured : false;

  const selectProvider = (id: string) => {
    setProviderId(id);
    const next = providers.find((p) => p.id === id);
    setModelId(next?.models[0]?.id ?? '');
  };

  const send = async (e: FormEvent) => {
    e.preventDefault();
    const text = input.trim();
    if (!text || streaming || !canUse) return;

    const history: ChatMessage[] = [...messages, { role: 'user', content: text }];
    setMessages([...history, { role: 'assistant', content: '' }]);
    setInput('');
    setError(null);
    setStreaming(true);

    const controller = new AbortController();
    abortRef.current = controller;

    await streamChat(
      { provider: providerId, model: modelId, messages: history },
      {
        onDelta: (delta) =>
          setMessages((prev) =>
            prev.map((m, i) => (i === prev.length - 1 ? { ...m, content: m.content + delta } : m)),
          ),
        onError: (message) => setError(message),
        signal: controller.signal,
      },
    );

    setStreaming(false);
    abortRef.current = null;
  };

  return (
    <div className="flex h-[calc(100svh-9rem)] flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">AI</h1>
          <p className="text-sm text-muted-foreground">Chat with your data across providers.</p>
        </div>
        <div className="flex items-center gap-2">
          <select
            value={providerId}
            onChange={(e) => selectProvider(e.target.value)}
            aria-label="Provider"
            className={SELECT_CLASS}
          >
            {providers.map((p) => (
              <option key={p.id} value={p.id}>
                {p.label}
              </option>
            ))}
          </select>
          <select
            value={modelId}
            onChange={(e) => setModelId(e.target.value)}
            aria-label="Model"
            className={SELECT_CLASS}
            disabled={!provider}
          >
            {provider?.models.map((m) => (
              <option key={m.id} value={m.id}>
                {m.label}
              </option>
            ))}
          </select>
          <Button variant="outline" size="icon" onClick={() => setSettingsOpen(true)} aria-label="AI settings">
            <Settings className="h-4 w-4" />
          </Button>
        </div>
      </div>

      {provider && !canUse && (
        <p className="rounded-md border border-primary/30 bg-primary/10 px-3 py-2 text-sm" role="alert">
          {provider.label} needs an API key.{' '}
          <button type="button" className="font-medium underline" onClick={() => setSettingsOpen(true)}>
            Add one in settings
          </button>
          .
        </p>
      )}

      <div className="flex-1 overflow-auto rounded-lg border border-border bg-card p-4">
        {messages.length === 0 ? (
          <div className="grid h-full place-items-center text-center text-sm text-muted-foreground">
            Start a conversation. The Mock provider works offline with no API key.
          </div>
        ) : (
          <div className="flex flex-col gap-4">
            {messages.map((m, i) => (
              <div
                key={i}
                className={cn('flex', m.role === 'user' ? 'justify-end' : 'justify-start')}
              >
                <div
                  className={cn(
                    'max-w-[80%] rounded-lg px-3 py-2 text-sm',
                    m.role === 'user'
                      ? 'bg-primary text-primary-foreground'
                      : 'bg-muted text-foreground',
                  )}
                >
                  {m.role === 'assistant' ? (
                    m.content ? (
                      <Markdown>{m.content}</Markdown>
                    ) : (
                      <Spinner label="Thinking" />
                    )
                  ) : (
                    <span className="whitespace-pre-wrap">{m.content}</span>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {error && (
        <p className="rounded-md border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger" role="alert">
          {error}
        </p>
      )}

      <form onSubmit={send} className="flex items-end gap-2">
        <Textarea
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
              e.preventDefault();
              void send(e);
            }
          }}
          placeholder={canUse ? 'Message…  (Enter to send, Shift+Enter for newline)' : 'Configure a provider to chat'}
          aria-label="Message"
          rows={2}
          disabled={!canUse || streaming}
          className="resize-none"
        />
        {streaming ? (
          <Button type="button" variant="outline" onClick={() => abortRef.current?.abort()}>
            <Square className="h-4 w-4" aria-hidden="true" />
            Stop
          </Button>
        ) : (
          <Button type="submit" disabled={!input.trim() || !canUse}>
            <Send className="h-4 w-4" aria-hidden="true" />
            Send
          </Button>
        )}
      </form>

      <KeySettingsModal open={settingsOpen} onClose={() => setSettingsOpen(false)} />
    </div>
  );
}
