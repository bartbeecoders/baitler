import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { apiFetch } from '@/lib/api';
import { AgentPage } from './AgentPage';

const providers = [
  {
    id: 'anthropic',
    label: 'Anthropic',
    requires_key: true,
    configured: true,
    models: [{ id: 'claude-opus-4-8', label: 'Claude Opus 4.8', modalities: ['text'] }],
  },
];

vi.mock('@/lib/api', () => ({
  API_BASE_URL: 'http://localhost:8080',
  ApiError: class ApiError extends Error {},
  apiFetch: vi.fn(async (path: string) => {
    if (path === '/ai/providers') return { providers };
    if (path === '/projects') return { projects: [] };
    if (path === '/cli/runs') return { runs: [] };
    if (path === '/cli/status')
      return {
        enabled: true,
        kind: 'claude-cli',
        binary_ok: true,
        version: '1.0.0',
        has_stored_key: true,
        host_key_env: false,
        ready: true,
        message: 'Ready.',
      };
    if (path === '/ideas' || path === '/documents') return { id: 'new-1' };
    return null;
  }),
}));

const READY_STATUS = {
  enabled: true,
  kind: 'claude-cli',
  binary_ok: true,
  version: '1.0.0',
  has_stored_key: true,
  host_key_env: false,
  ready: true,
  message: 'Ready.',
  providers: [
    { id: 'claude_code', label: 'Claude Code', available: true, detail: 'Anthropic.' },
    { id: 'minimax', label: 'MiniMax-M3', available: true, detail: 'MiniMax.' },
  ],
  workspace_roots: [],
};

async function defaultApiFetch(path: string) {
  if (path === '/ai/providers') return { providers };
  if (path === '/projects') return { projects: [] };
  if (path === '/cli/runs') return { runs: [] };
  if (path === '/cli/status') return READY_STATUS;
  if (path === '/ideas' || path === '/documents') return { id: 'new-1' };
  return null;
}

// Reset to the default (ready) implementation before each test; individual tests
// override via `mockImplementation` for the not-ready case.
beforeEach(() => vi.mocked(apiFetch).mockImplementation(defaultApiFetch));

function encode(s: string) {
  return new TextEncoder().encode(s);
}

/** An SSE Response; if `keepOpen`, the stream never closes (run stays live). */
function sse(body: string, { keepOpen = false, status = 200 } = {}): Response {
  const stream = new ReadableStream({
    start(controller) {
      controller.enqueue(encode(body));
      if (!keepOpen) controller.close();
    },
  });
  return new Response(stream, { status });
}

function renderPage() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <AgentPage />
    </QueryClientProvider>,
  );
}

const FULL_RUN = [
  'data: {"type":"run","id":"r1"}',
  'data: {"type":"init","session_id":"s1","model":"claude"}',
  'data: {"type":"assistant","text":"Captured a draft idea."}',
  'data: {"type":"tool_use","name":"mcp__baitler__ideas_create","summary":"create"}',
  'data: {"type":"tool_result","ok":true,"summary":"created"}',
  'data: {"type":"result","text":"All done.","session_id":"s1","num_turns":2,"cost_usd":0,"is_error":false}',
  'data: {"type":"done","status":"succeeded"}',
  '',
].join('\n\n');

describe('AgentPage', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('runs a task and renders the streamed transcript', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => sse(FULL_RUN)));
    renderPage();

    await userEvent.type(screen.getByRole('textbox', { name: 'Task' }), 'Document my project');
    await userEvent.click(screen.getByRole('button', { name: /run/i }));

    expect(await screen.findByText('Captured a draft idea.')).toBeInTheDocument();
    expect(screen.getByText('mcp__baitler__ideas_create')).toBeInTheDocument();
    // The result footer + the save affordance appear.
    expect(await screen.findByRole('button', { name: /save this result/i })).toBeInTheDocument();
  });

  it('blocks Run with a preflight banner when the runner is not ready', async () => {
    vi.mocked(apiFetch).mockImplementation(async (path: string) => {
      if (path === '/ai/providers') return { providers };
      if (path === '/projects') return { projects: [] };
      if (path === '/cli/runs') return { runs: [] };
      if (path === '/cli/status')
        return {
          enabled: false,
          kind: 'claude-cli',
          binary_ok: false,
          version: null,
          has_stored_key: false,
          host_key_env: false,
          ready: false,
          message: 'The agent runner is disabled. Set CLAUDE_CLI_ENABLED=true on the server.',
          providers: [
            {
              id: 'claude_code',
              label: 'Claude Code',
              available: false,
              detail: 'The agent runner is disabled. Set CLAUDE_CLI_ENABLED=true on the server.',
            },
            { id: 'minimax', label: 'MiniMax-M3', available: false, detail: 'The agent runner is disabled.' },
          ],
        };
      return null;
    });
    renderPage();

    expect(await screen.findByText(/set CLAUDE_CLI_ENABLED=true/i)).toBeInTheDocument();
    await userEvent.type(screen.getByRole('textbox', { name: 'Task' }), 'hi');
    expect(screen.getByRole('button', { name: /run/i })).toBeDisabled();
  });

  it('offers the agent providers and blocks Run when the selected one is unavailable', async () => {
    vi.mocked(apiFetch).mockImplementation(async (path: string) => {
      if (path === '/ai/providers') return { providers };
      if (path === '/projects') return { projects: [] };
      if (path === '/cli/runs') return { runs: [] };
      if (path === '/cli/status')
        return {
          ...READY_STATUS,
          providers: [
            { id: 'claude_code', label: 'Claude Code', available: true, detail: 'Anthropic.' },
            {
              id: 'minimax',
              label: 'MiniMax-M3',
              available: false,
              detail: 'Not configured — set MINIMAX_API_KEY on the server.',
            },
          ],
        };
      return null;
    });
    renderPage();

    // Both providers are offered in the Agent select.
    const agentSelect = await screen.findByRole('combobox', { name: 'Agent' });
    // The MiniMax option appears once the /cli/status query resolves.
    expect(await within(agentSelect).findByRole('option', { name: /minimax/i })).toBeInTheDocument();
    expect(within(agentSelect).getByRole('option', { name: /claude code/i })).toBeInTheDocument();

    // Claude Code is available → Run enabled.
    await userEvent.type(screen.getByRole('textbox', { name: 'Task' }), 'hi');
    expect(screen.getByRole('button', { name: /run/i })).toBeEnabled();

    // Switch to the unconfigured MiniMax → banner + Run disabled.
    await userEvent.selectOptions(agentSelect, 'minimax');
    expect(await screen.findByText(/set MINIMAX_API_KEY/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /run/i })).toBeDisabled();
  });

  it('shows a local-folder input only when the server allow-lists roots', async () => {
    // Default status has no workspace roots → no input.
    const { unmount } = renderPage();
    expect(await screen.findByRole('textbox', { name: 'Task' })).toBeInTheDocument();
    expect(screen.queryByRole('textbox', { name: 'Local folder' })).not.toBeInTheDocument();
    unmount();

    // With a configured root, the input + hint appear.
    vi.mocked(apiFetch).mockImplementation(async (path: string) => {
      if (path === '/cli/status') return { ...READY_STATUS, workspace_roots: ['/home/bart'] };
      if (path === '/ai/providers') return { providers };
      if (path === '/projects') return { projects: [] };
      if (path === '/cli/runs') return { runs: [] };
      return null;
    });
    renderPage();
    expect(await screen.findByRole('textbox', { name: 'Local folder' })).toBeInTheDocument();
    expect(screen.getByText(/\/home\/bart/)).toBeInTheDocument();
  });

  it('shows the disabled banner when the runner returns 503', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(
        async () =>
          new Response(
            '{"error":{"code":"unavailable","message":"the Claude Code CLI runner is disabled"}}',
            { status: 503 },
          ),
      ),
    );
    renderPage();

    await userEvent.type(screen.getByRole('textbox', { name: 'Task' }), 'hi');
    await userEvent.click(screen.getByRole('button', { name: /run/i }));

    const alerts = await screen.findAllByText(/runner is disabled/i);
    expect(alerts.length).toBeGreaterThan(0);
  });

  it('Stop cancels the in-flight run via the cancel endpoint', async () => {
    // A stream that opens (run + init) but never closes, so the run stays live.
    const open = ['data: {"type":"run","id":"r1"}', 'data: {"type":"init","session_id":"s1","model":"m"}', ''].join(
      '\n\n',
    );
    vi.stubGlobal('fetch', vi.fn(async () => sse(open, { keepOpen: true })));
    renderPage();

    await userEvent.type(screen.getByRole('textbox', { name: 'Task' }), 'long task');
    await userEvent.click(screen.getByRole('button', { name: /run/i }));

    // Wait until the stream's leading events have been processed (run id known).
    expect(await screen.findByText(/session started/i)).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: /stop/i }));

    await waitFor(() =>
      expect(vi.mocked(apiFetch)).toHaveBeenCalledWith('/cli/runs/r1/cancel', { method: 'POST' }),
    );
  });

  it('continues the conversation by resuming the prior session', async () => {
    const bodies: string[] = [];
    vi.stubGlobal(
      'fetch',
      vi.fn(async (_url: string, init?: RequestInit) => {
        bodies.push(String(init?.body ?? ''));
        return sse(FULL_RUN);
      }),
    );
    renderPage();

    // First message → establishes the session (FULL_RUN carries session_id "s1").
    await userEvent.type(screen.getByRole('textbox', { name: 'Task' }), 'first');
    await userEvent.click(screen.getByRole('button', { name: /run/i }));
    expect(await screen.findByText('Captured a draft idea.')).toBeInTheDocument();

    // The button now reads "Send" (we're continuing a conversation).
    const send = await screen.findByRole('button', { name: /send/i });
    await userEvent.type(screen.getByRole('textbox', { name: 'Task' }), 'second');
    await userEvent.click(send);

    await waitFor(() => expect(bodies.length).toBe(2));
    const first = JSON.parse(bodies[0] ?? '{}');
    const second = JSON.parse(bodies[1] ?? '{}');
    // First call has no resume; the second resumes session "s1".
    expect(first.resume_session_id).toBeUndefined();
    expect(second.resume_session_id).toBe('s1');
    // Both turns share one conversation id (→ same server working dir).
    expect(first.conversation_id).toBeTruthy();
    expect(second.conversation_id).toBe(first.conversation_id);
    // Both prompts remain visible in the thread.
    expect(screen.getByText('first')).toBeInTheDocument();
    expect(screen.getByText('second')).toBeInTheDocument();
  });

  it('saves a result as a draft idea', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => sse(FULL_RUN)));
    renderPage();

    await userEvent.type(screen.getByRole('textbox', { name: 'Task' }), 'do it');
    await userEvent.click(screen.getByRole('button', { name: /run/i }));
    await userEvent.click(await screen.findByRole('button', { name: /save this result/i }));

    // Modal open → save as idea.
    await userEvent.click(screen.getByRole('button', { name: /as idea/i }));

    await waitFor(() =>
      expect(vi.mocked(apiFetch)).toHaveBeenCalledWith(
        '/ideas',
        expect.objectContaining({ method: 'POST' }),
      ),
    );
    // …then flipped to a draft for the Review queue.
    await waitFor(() =>
      expect(vi.mocked(apiFetch)).toHaveBeenCalledWith(
        '/ideas/new-1',
        expect.objectContaining({ method: 'PATCH' }),
      ),
    );
  });
});
