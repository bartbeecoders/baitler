import { useEffect, useMemo, useRef, useState, type FormEvent } from 'react';
import { Play, Plus, Save, Square } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Modal } from '@/components/ui/modal';
import { Spinner } from '@/components/ui/spinner';
import { Textarea } from '@/components/ui/textarea';
import { useProviders } from '@/features/ai/api';
import { cn } from '@/lib/cn';
import {
  errorMessage,
  saveResultAs,
  useCliStatus,
  useProjectOptions,
  useRun,
  useRuns,
  type SaveTarget,
} from './api';
import { EventRow } from './EventRow';
import { resultOf, useAgentChat } from './useAgentChat';
import type { AgentProvider, ToolScope } from './types';

const SELECT_CLASS = 'h-10 rounded-md border border-input bg-background px-2 text-sm';

export interface AgentPanelProps {
  /** Rendered inside the right-hand dock (compact, fills its pane) vs. full page. */
  embedded?: boolean;
  /** Short human label for the page the agent is acting within (dock only). */
  contextLabel?: string;
  /** Orienting context appended to the agent's system prompt for each run. */
  context?: string;
}

/**
 * The Agent console as a **chat conversation**: each message continues the same
 * agent session (`--resume`), so you can keep talking; "New chat" starts fresh,
 * and a previous run can be re-opened to continue it. Used both as the `/agent`
 * page and inside the docked right-hand pane (`embedded`), where `context`
 * orients the agent to the page the user is viewing.
 */
export function AgentPanel({ embedded = false, context }: AgentPanelProps) {
  const { data: providers = [] } = useProviders();
  const { data: projects = [] } = useProjectOptions();
  const { data: runs = [] } = useRuns();
  const { data: status } = useCliStatus();

  const [prompt, setPrompt] = useState('');
  const [provider, setProvider] = useState<AgentProvider>('claude_code');
  const [model, setModel] = useState('');
  const [projectId, setProjectId] = useState('');
  const [toolScope, setToolScope] = useState<ToolScope>('kb_only');
  const [maxTurns, setMaxTurns] = useState('');
  const [workspaceDir, setWorkspaceDir] = useState('');

  const chat = useAgentChat();
  const { turns, sessionId, streaming, error, unavailable } = chat;

  // Resume a previous run as the start of a chat.
  const [openId, setOpenId] = useState<string | null>(null);
  const { data: openRun } = useRun(openId);
  const seededRef = useRef<string | null>(null);

  const threadRef = useRef<HTMLDivElement | null>(null);

  const anthropicModels = useMemo(
    () => providers.find((p) => p.id === 'anthropic')?.models ?? [],
    [providers],
  );

  const providerList = status?.providers ?? [];
  const selectedProvider = providerList.find((p) => p.id === provider);
  const workspaceRoots = status?.workspace_roots ?? [];
  const blocked = !!status && !!selectedProvider && !selectedProvider.available;
  const authHint =
    provider === 'claude_code' &&
    status?.ready &&
    status.kind === 'claude-cli' &&
    !status.has_stored_key &&
    !status.host_key_env;

  const lastTurn = turns[turns.length - 1];
  const lastResult = lastTurn && !lastTurn.streaming ? resultOf(lastTurn.events) : undefined;

  // Seed the thread from a re-opened past run (once per run), so the next message
  // continues its session.
  useEffect(() => {
    if (!openRun || seededRef.current === openRun.id) return;
    seededRef.current = openRun.id;
    chat.seedFromRun(openRun);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [openRun]);

  // Keep the latest message in view as it streams.
  useEffect(() => {
    const el = threadRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [turns]);

  const send = async (e: FormEvent) => {
    e.preventDefault();
    const text = prompt.trim();
    if (!text || streaming || blocked) return;
    setPrompt('');
    const turnNum = Number.parseInt(maxTurns, 10);
    await chat.send(text, {
      provider,
      context: context || undefined,
      model: model || undefined,
      project_id: projectId || undefined,
      workspace_dir: workspaceDir.trim() || undefined,
      tool_scope: toolScope,
      max_turns: Number.isFinite(turnNum) && turnNum > 0 ? turnNum : undefined,
    });
  };

  const newChat = () => {
    if (streaming) return;
    chat.reset();
    setOpenId(null);
    seededRef.current = null;
  };

  return (
    <div className={cn('flex flex-col gap-3', embedded && 'h-full')}>
      {!embedded && (
        <div className="flex items-start justify-between gap-2">
          <div>
            <h1 className="text-2xl font-bold tracking-tight">Agent</h1>
            <p className="text-sm text-muted-foreground">
              Chat with Claude Code or MiniMax over your Baitler data — each message continues the
              same session. Writes land as drafts for your review.
            </p>
          </div>
        </div>
      )}

      {blocked && selectedProvider && (
        <p
          className="rounded-md border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger"
          role="alert"
        >
          {selectedProvider.label}: {selectedProvider.detail}
        </p>
      )}
      {authHint && status && (
        <p className="rounded-md border border-primary/30 bg-primary/10 px-3 py-2 text-sm">
          {status.message}
        </p>
      )}
      {unavailable && !blocked && (
        <p
          className="rounded-md border border-primary/30 bg-primary/10 px-3 py-2 text-sm"
          role="alert"
        >
          {unavailable}
        </p>
      )}

      {/* Conversation toolbar */}
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs text-muted-foreground">
          {sessionId ? 'Continuing this conversation' : 'New conversation'}
        </span>
        <div className="flex items-center gap-2">
          {runs.length > 0 && (
            <select
              aria-label="Previous chats"
              className={cn(SELECT_CLASS, 'max-w-[12rem]')}
              value=""
              disabled={streaming}
              onChange={(e) => e.target.value && setOpenId(e.target.value)}
            >
              <option value="">Previous chats…</option>
              {runs.map((r) => (
                <option key={r.id} value={r.id}>
                  {r.prompt.slice(0, 48)}
                </option>
              ))}
            </select>
          )}
          <Button
            variant="ghost"
            size="sm"
            onClick={newChat}
            disabled={streaming || turns.length === 0}
          >
            <Plus className="h-4 w-4" aria-hidden="true" />
            New chat
          </Button>
        </div>
      </div>

      {/* Thread */}
      <div
        ref={threadRef}
        className={cn(
          'flex flex-col gap-4 rounded-lg border border-border bg-card p-3',
          embedded ? 'min-h-0 flex-1 overflow-auto' : 'min-h-64',
        )}
      >
        {turns.length === 0 ? (
          <div className="grid flex-1 place-items-center py-10 text-center text-sm text-muted-foreground">
            Start a conversation. Ask the agent to organise, draft, or answer from your data.
          </div>
        ) : (
          turns.map((turn, i) => (
            <div key={i} className="flex flex-col gap-2">
              <div className="self-end max-w-[85%] whitespace-pre-wrap rounded-lg bg-primary px-3 py-2 text-sm text-primary-foreground">
                {turn.prompt}
              </div>
              <div className="flex flex-col gap-2">
                {turn.events.length === 0 && turn.streaming ? (
                  <Spinner label="Starting the agent" />
                ) : (
                  turn.events.map((ev, j) => <EventRow key={j} event={ev} />)
                )}
                {turn.streaming && turn.events.length > 0 && <Spinner label="Working" />}
              </div>
            </div>
          ))
        )}
        {lastResult?.text && <SaveResultBar title="Save this result" body={lastResult.text} />}
      </div>

      {error && (
        <p
          className="rounded-md border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger"
          role="alert"
        >
          {error}
        </p>
      )}

      {/* Composer (chat input) */}
      <form
        onSubmit={send}
        className="flex flex-col gap-3 rounded-lg border border-border bg-card p-3"
      >
        <Textarea
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
              e.preventDefault();
              void send(e);
            }
          }}
          placeholder={
            sessionId
              ? 'Reply to continue…  (Enter to send, Shift+Enter for newline)'
              : 'Describe a task — e.g. “Summarise my open ideas.”  (Enter to send)'
          }
          aria-label="Task"
          rows={2}
          disabled={streaming}
          className="resize-none"
        />
        <div className="flex flex-wrap items-center gap-2">
          <select
            value={provider}
            onChange={(e) => {
              setProvider(e.target.value as AgentProvider);
              setModel('');
            }}
            aria-label="Agent"
            className={SELECT_CLASS}
            disabled={streaming}
          >
            {(providerList.length > 0
              ? providerList
              : [{ id: 'claude_code' as AgentProvider, label: 'Claude Code', available: true }]
            ).map((p) => (
              <option key={p.id} value={p.id}>
                {p.label}
                {p.available ? '' : ' (unavailable)'}
              </option>
            ))}
          </select>
          <select
            value={model}
            onChange={(e) => setModel(e.target.value)}
            aria-label="Model"
            className={SELECT_CLASS}
            disabled={streaming || provider !== 'claude_code'}
          >
            <option value="">Default model</option>
            {provider === 'claude_code' &&
              anthropicModels.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.label}
                </option>
              ))}
          </select>
          <select
            value={projectId}
            onChange={(e) => setProjectId(e.target.value)}
            aria-label="Project"
            className={SELECT_CLASS}
            disabled={streaming}
          >
            <option value="">No project</option>
            {projects.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
          <select
            value={toolScope}
            onChange={(e) => setToolScope(e.target.value as ToolScope)}
            aria-label="Tool scope"
            className={SELECT_CLASS}
            disabled={streaming}
          >
            <option value="kb_only">KB tools only</option>
            <option value="kb_plus_read">KB + read files</option>
          </select>
          <Input
            type="number"
            min={1}
            value={maxTurns}
            onChange={(e) => setMaxTurns(e.target.value)}
            placeholder="Max turns"
            aria-label="Max turns"
            className="h-10 w-28"
            disabled={streaming}
          />
          <div className="ml-auto">
            {streaming ? (
              <Button type="button" variant="outline" onClick={chat.stop}>
                <Square className="h-4 w-4" aria-hidden="true" />
                Stop
              </Button>
            ) : (
              <Button type="submit" disabled={!prompt.trim() || blocked}>
                <Play className="h-4 w-4" aria-hidden="true" />
                {sessionId ? 'Send' : 'Run'}
              </Button>
            )}
          </div>
        </div>

        {workspaceRoots.length > 0 && (
          <div className="flex flex-col gap-1">
            <Input
              value={workspaceDir}
              onChange={(e) => setWorkspaceDir(e.target.value)}
              placeholder="Local folder to read (e.g. import files from disk)"
              aria-label="Local folder"
              disabled={streaming}
              className="h-10"
            />
            <p className="text-xs text-muted-foreground">
              Grants the agent read-only access to a folder within: {workspaceRoots.join(', ')}
            </p>
          </div>
        )}
      </form>
    </div>
  );
}

/** Offer to save a result as a draft Idea / Document / Page. */
function SaveResultBar({ title, body }: { title: string; body: string }) {
  const [open, setOpen] = useState(false);
  const [saveTitle, setSaveTitle] = useState('');
  const [saving, setSaving] = useState<SaveTarget | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const openModal = () => {
    setSaveTitle(body.split('\n').find((l) => l.trim())?.slice(0, 80) ?? 'Agent result');
    setSaved(null);
    setErr(null);
    setOpen(true);
  };

  const save = async (target: SaveTarget) => {
    setSaving(target);
    setErr(null);
    try {
      await saveResultAs(target, { title: saveTitle.trim() || 'Agent result', body });
      setSaved(`Saved as a draft ${target}.`);
    } catch (e) {
      setErr(errorMessage(e));
    } finally {
      setSaving(null);
    }
  };

  return (
    <div className="border-t border-border pt-3">
      <Button variant="outline" size="sm" onClick={openModal}>
        <Save className="h-4 w-4" aria-hidden="true" />
        {title}
      </Button>

      <Modal open={open} onClose={() => setOpen(false)} title="Save result as a draft">
        <div className="flex flex-col gap-3">
          <label className="text-sm">
            Title
            <Input
              value={saveTitle}
              onChange={(e) => setSaveTitle(e.target.value)}
              className="mt-1"
              aria-label="Save title"
            />
          </label>
          <p className="text-xs text-muted-foreground">
            Creates a draft you can review and publish later.
          </p>
          {err && (
            <p className="text-sm text-danger" role="alert">
              {err}
            </p>
          )}
          {saved && (
            <p className="text-sm text-success" role="status">
              {saved}
            </p>
          )}
          <div className="flex flex-wrap gap-2">
            {(['idea', 'document', 'page'] as const).map((t) => (
              <Button
                key={t}
                variant="secondary"
                size="sm"
                disabled={saving !== null}
                onClick={() => save(t)}
              >
                {saving === t ? 'Saving…' : `As ${t}`}
              </Button>
            ))}
          </div>
        </div>
      </Modal>
    </div>
  );
}
