import { useEffect, useMemo, useRef, useState, type FormEvent } from 'react';
import { Paperclip, Play, Plus, SlidersHorizontal, Square, X } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { Spinner } from '@/components/ui/spinner';
import { Textarea } from '@/components/ui/textarea';
import { useProviders } from '@/features/ai/api';
import { useCliStatus, useProjectOptions } from '@/features/cli/api';
import { EventRow } from '@/features/cli/EventRow';
import { useAgentChat } from '@/features/cli/useAgentChat';
import type { AgentProvider, ToolScope } from '@/features/cli/types';
import { SystemStatus } from '@/features/portal/SystemStatus';
import { cn } from '@/lib/cn';
import { QUICK_TASKS, type QuickTask } from './quickTasks';
import { RunReportFeed } from './RunReportFeed';
import { WorkspacePicker } from './WorkspacePicker';

const SELECT_CLASS = 'h-9 rounded-md border border-input bg-background px-2 text-sm';

function greeting(now: Date = new Date()): string {
  const h = now.getHours();
  if (h < 6) return 'Up late';
  if (h < 12) return 'Good morning';
  if (h < 18) return 'Good afternoon';
  return 'Good evening';
}

/** The last path segment, for the attached-folder chip. */
function basename(path: string): string {
  const parts = path.split('/').filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

/**
 * The butler-first home: a conversational command surface. Point the butler at a
 * local code project or documents folder (read-only, allow-listed), tell it what
 * to do, watch it work, and review what it organized in the report feed below.
 */
export function ButlerHome() {
  const { data: status } = useCliStatus();
  const { data: providers = [] } = useProviders();
  const { data: projects = [] } = useProjectOptions();

  const [prompt, setPrompt] = useState('');
  const [workspaceDir, setWorkspaceDir] = useState('');
  const [toolScope, setToolScope] = useState<ToolScope>('kb_only');
  const [provider, setProvider] = useState<AgentProvider>('claude_code');
  const [model, setModel] = useState('');
  const [projectId, setProjectId] = useState('');
  const [showSettings, setShowSettings] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);

  const chat = useAgentChat();
  const { turns, sessionId, streaming, error, unavailable } = chat;

  const threadRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    const el = threadRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [turns]);

  const anthropicModels = useMemo(
    () => providers.find((p) => p.id === 'anthropic')?.models ?? [],
    [providers],
  );
  const providerList = status?.providers ?? [];
  const selectedProvider = providerList.find((p) => p.id === provider);
  const blocked = !!status && !!selectedProvider && !selectedProvider.available;
  const rootsConfigured = (status?.workspace_roots.length ?? 0) > 0;

  const attach = (path: string) => {
    setWorkspaceDir(path);
    // A granted folder implies the butler should be able to read it.
    setToolScope('kb_plus_read');
  };

  const detach = () => {
    setWorkspaceDir('');
    setToolScope('kb_only');
  };

  const runQuickTask = (task: QuickTask) => {
    setPrompt(task.prompt);
    setToolScope(task.toolScope);
    if (task.needsFolder && !workspaceDir) setPickerOpen(true);
  };

  const send = async (e: FormEvent) => {
    e.preventDefault();
    const text = prompt.trim();
    if (!text || streaming || blocked) return;
    setPrompt('');
    await chat.send(text, {
      provider,
      model: model || undefined,
      project_id: projectId || undefined,
      workspace_dir: workspaceDir || undefined,
      tool_scope: toolScope,
    });
  };

  const conversationActive = turns.length > 0;

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-8">
      {/* Hero greeting (compact once a conversation starts) */}
      <section className={cn('text-center', conversationActive ? 'pt-0' : 'pt-6 sm:pt-12')}>
        {!conversationActive && (
          <>
            <p className="text-sm font-medium text-primary-strong">Baitler</p>
            <h1 className="mt-1 text-3xl font-bold tracking-tight sm:text-4xl">
              {greeting()} — what shall I organize for you?
            </h1>
            <p className="mx-auto mt-3 max-w-xl text-sm text-muted-foreground">
              Point me at a code project or a folder of documents on disk and I'll read it,
              summarize it, and file it into your knowledge base — tagged, categorized, and
              linked. Everything I write lands as a draft for your review.
            </p>
          </>
        )}
      </section>

      {!status?.ready && status && (
        <p className="rounded-md border border-primary/30 bg-primary/10 px-3 py-2 text-sm">
          {status.message}
        </p>
      )}
      {blocked && selectedProvider && (
        <p
          className="rounded-md border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger"
          role="alert"
        >
          {selectedProvider.label}: {selectedProvider.detail}
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

      {/* Conversation thread */}
      {conversationActive && (
        <div
          ref={threadRef}
          className="flex max-h-[28rem] flex-col gap-4 overflow-y-auto rounded-lg border border-border bg-card p-3"
        >
          {turns.map((turn, i) => (
            <div key={i} className="flex flex-col gap-2">
              <div className="max-w-[85%] self-end whitespace-pre-wrap rounded-lg bg-primary px-3 py-2 text-sm text-primary-foreground">
                {turn.prompt}
              </div>
              <div className="flex flex-col gap-2">
                {turn.events.length === 0 && turn.streaming ? (
                  <Spinner label="Summoning the butler" />
                ) : (
                  turn.events.map((ev, j) => <EventRow key={j} event={ev} />)
                )}
                {turn.streaming && turn.events.length > 0 && <Spinner label="Working" />}
              </div>
            </div>
          ))}
        </div>
      )}

      {error && (
        <p
          className="rounded-md border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger"
          role="alert"
        >
          {error}
        </p>
      )}

      {/* Command composer */}
      <form
        onSubmit={send}
        className="flex flex-col gap-3 rounded-xl border border-border bg-card p-3 shadow-sm"
      >
        <div className="flex flex-wrap items-center gap-2">
          {workspaceDir ? (
            <span className="inline-flex max-w-full items-center gap-1.5 rounded-full bg-accent px-3 py-1 text-xs font-medium text-accent-foreground">
              <Paperclip className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
              <span className="min-w-0 truncate" title={workspaceDir}>
                {basename(workspaceDir)}
              </span>
              <button
                type="button"
                onClick={detach}
                aria-label="Detach folder"
                className="ml-0.5 rounded-full p-0.5 hover:bg-background/60"
                disabled={streaming}
              >
                <X className="h-3 w-3" aria-hidden="true" />
              </button>
            </span>
          ) : (
            rootsConfigured && (
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => setPickerOpen(true)}
                disabled={streaming}
              >
                <Paperclip className="h-4 w-4" aria-hidden="true" />
                Attach a folder
              </Button>
            )
          )}
          {workspaceDir && (
            <span className="text-xs text-muted-foreground">read-only · this conversation</span>
          )}
          <div className="ml-auto flex items-center gap-1">
            {conversationActive && (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={chat.reset}
                disabled={streaming}
              >
                <Plus className="h-4 w-4" aria-hidden="true" />
                New task
              </Button>
            )}
            <Button
              type="button"
              variant="ghost"
              size="icon"
              onClick={() => setShowSettings((v) => !v)}
              aria-label="Run settings"
              aria-expanded={showSettings}
            >
              <SlidersHorizontal className="h-4 w-4" aria-hidden="true" />
            </Button>
          </div>
        </div>

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
              : 'e.g. “Look at this code project, understand it, and create a tagged summary.”'
          }
          aria-label="Tell the butler what to do"
          rows={3}
          disabled={streaming}
          className="resize-none border-0 bg-transparent p-1 shadow-none focus-visible:ring-0"
        />

        <div className="flex flex-wrap items-center gap-2">
          {showSettings && (
            <>
              <select
                value={provider}
                onChange={(e) => {
                  setProvider(e.target.value as AgentProvider);
                  setModel('');
                }}
                aria-label="Agent provider"
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
                <option value="kb_plus_read">KB + read host files</option>
              </select>
            </>
          )}
          <div className="ml-auto">
            {streaming ? (
              <Button type="button" variant="outline" onClick={chat.stop}>
                <Square className="h-4 w-4" aria-hidden="true" />
                Stop
              </Button>
            ) : (
              <Button type="submit" disabled={!prompt.trim() || blocked}>
                <Play className="h-4 w-4" aria-hidden="true" />
                {sessionId ? 'Send' : 'Go'}
              </Button>
            )}
          </div>
        </div>
      </form>

      {/* Quick tasks (fresh start only) */}
      {!conversationActive && (
        <div className="flex flex-wrap justify-center gap-2">
          {QUICK_TASKS.map((task) => (
            <Button
              key={task.key}
              type="button"
              variant="outline"
              size="sm"
              onClick={() => runQuickTask(task)}
              disabled={streaming || (task.needsFolder && !rootsConfigured)}
              title={
                task.needsFolder && !rootsConfigured
                  ? 'Set CLAUDE_CLI_WORKSPACE_ROOTS on the server to grant local folders.'
                  : undefined
              }
            >
              <task.icon className="h-4 w-4" aria-hidden="true" />
              {task.label}
            </Button>
          ))}
        </div>
      )}

      <RunReportFeed />

      <SystemStatus />

      <WorkspacePicker open={pickerOpen} onClose={() => setPickerOpen(false)} onPick={attach} />
    </div>
  );
}
