import { useState } from 'react';
import { Link } from 'react-router-dom';
import { Activity as ActivityIcon, CheckCircle2, FolderKanban, Plus, Trash2 } from 'lucide-react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/lib/cn';

import {
  errorMessage,
  useActivity,
  useApprove,
  useCreateProject,
  useDeleteProject,
  useProject,
  useProjects,
  useReviewQueue,
} from './api';
import { FEATURE_ROUTE, type ItemType, type MemberItem } from './types';

type Tab = 'projects' | 'review' | 'activity';

export function ProjectsPage() {
  const [tab, setTab] = useState<Tab>('projects');
  const review = useReviewQueue();
  const pending = (review.data?.ideas.length ?? 0) + (review.data?.documents.length ?? 0);

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Projects</h1>
        <p className="text-sm text-muted-foreground">
          Group your knowledge, review what agents drafted, and see who did what.
        </p>
      </div>

      <div className="flex gap-1 border-b border-border">
        <TabButton active={tab === 'projects'} onClick={() => setTab('projects')}>
          <FolderKanban className="h-4 w-4" aria-hidden="true" /> Projects
        </TabButton>
        <TabButton active={tab === 'review'} onClick={() => setTab('review')}>
          <CheckCircle2 className="h-4 w-4" aria-hidden="true" /> Review
          {pending > 0 && <Badge variant="warning">{pending}</Badge>}
        </TabButton>
        <TabButton active={tab === 'activity'} onClick={() => setTab('activity')}>
          <ActivityIcon className="h-4 w-4" aria-hidden="true" /> Activity
        </TabButton>
      </div>

      {tab === 'projects' && <ProjectsTab />}
      {tab === 'review' && <ReviewTab />}
      {tab === 'activity' && <ActivityTab />}
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
      className={cn(
        '-mb-px flex items-center gap-1.5 border-b-2 px-3 py-2 text-sm font-medium transition-colors',
        active
          ? 'border-primary text-foreground'
          : 'border-transparent text-muted-foreground hover:text-foreground',
      )}
    >
      {children}
    </button>
  );
}

// ── Projects tab ──────────────────────────────────────────────────────────────

function ProjectsTab() {
  const projects = useProjects();
  const create = useCreateProject();
  const del = useDeleteProject();
  const [selected, setSelected] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState('');
  const [summary, setSummary] = useState('');

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    create.mutate(
      { name: name.trim(), summary: summary.trim() },
      {
        onSuccess: () => {
          setName('');
          setSummary('');
          setCreating(false);
        },
      },
    );
  };

  if (projects.isLoading) return <Spinner />;
  if (projects.isError)
    return <p className="text-sm text-danger">{errorMessage(projects.error)}</p>;

  const list = projects.data ?? [];

  return (
    <div className="grid gap-6 lg:grid-cols-[20rem_1fr]">
      <div className="flex flex-col gap-3">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-semibold text-muted-foreground">
            {list.length} project{list.length === 1 ? '' : 's'}
          </h2>
          <Button size="sm" variant="outline" onClick={() => setCreating((v) => !v)}>
            <Plus className="h-4 w-4" aria-hidden="true" /> New
          </Button>
        </div>

        {creating && (
          <form onSubmit={submit} className="flex flex-col gap-2 rounded-md border border-border p-3">
            <Input
              placeholder="Project name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              autoFocus
            />
            <Input
              placeholder="Summary (optional)"
              value={summary}
              onChange={(e) => setSummary(e.target.value)}
            />
            <div className="flex justify-end gap-2">
              <Button type="button" size="sm" variant="ghost" onClick={() => setCreating(false)}>
                Cancel
              </Button>
              <Button type="submit" size="sm" disabled={!name.trim() || create.isPending}>
                Create
              </Button>
            </div>
            {create.isError && <p className="text-xs text-danger">{errorMessage(create.error)}</p>}
          </form>
        )}

        {list.length === 0 && (
          <p className="text-sm text-muted-foreground">
            No projects yet. An agent can create one over MCP, or add one here.
          </p>
        )}
        <ul className="flex flex-col gap-2">
          {list.map((p) => (
            <li key={p.id}>
              <button
                type="button"
                onClick={() => setSelected(p.id)}
                className={cn(
                  'w-full rounded-md border px-3 py-2 text-left transition-colors',
                  selected === p.id
                    ? 'border-primary bg-accent'
                    : 'border-border hover:bg-muted',
                )}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="truncate font-medium">{p.name}</span>
                  {p.status === 'archived' && <Badge variant="muted">archived</Badge>}
                </div>
                <span className="text-xs text-muted-foreground">/{p.slug}</span>
              </button>
            </li>
          ))}
        </ul>
      </div>

      <div>
        {selected ? (
          <ProjectDetailPanel
            id={selected}
            onDelete={() =>
              del.mutate(selected, { onSuccess: () => setSelected(null) })
            }
            deleting={del.isPending}
          />
        ) : (
          <Card className="p-6 text-sm text-muted-foreground">
            Select a project to see its members and pending drafts.
          </Card>
        )}
      </div>
    </div>
  );
}

function ProjectDetailPanel({
  id,
  onDelete,
  deleting,
}: {
  id: string;
  onDelete: () => void;
  deleting: boolean;
}) {
  const detail = useProject(id);
  if (detail.isLoading) return <Spinner />;
  if (detail.isError) return <p className="text-sm text-danger">{errorMessage(detail.error)}</p>;
  const p = detail.data;
  if (!p) return null;

  return (
    <Card className="flex flex-col gap-4 p-5">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold">{p.name}</h2>
          {p.summary && <p className="mt-1 text-sm text-muted-foreground">{p.summary}</p>}
        </div>
        <Button size="sm" variant="danger" onClick={onDelete} disabled={deleting}>
          <Trash2 className="h-4 w-4" aria-hidden="true" /> Delete
        </Button>
      </div>

      <div className="flex flex-wrap gap-2 text-xs">
        <Badge variant="muted">{p.counts.ideas} ideas</Badge>
        <Badge variant="muted">{p.counts.documents} documents</Badge>
        <Badge variant="muted">{p.counts.files} files</Badge>
        {p.counts.drafts > 0 && <Badge variant="warning">{p.counts.drafts} pending review</Badge>}
      </div>

      <MemberList title="Documents" type="document" items={p.members.documents} />
      <MemberList title="Ideas" type="idea" items={p.members.ideas} />
      <MemberList title="Files" type="file" items={p.members.files} />
    </Card>
  );
}

function MemberList({ title, type, items }: { title: string; type: ItemType; items: MemberItem[] }) {
  if (items.length === 0) return null;
  return (
    <div>
      <h3 className="mb-1 text-sm font-semibold text-muted-foreground">{title}</h3>
      <ul className="flex flex-col gap-1">
        {items.map((m) => (
          <li key={m.id} className="flex items-center justify-between gap-2 text-sm">
            <Link to={FEATURE_ROUTE[type]} className="truncate hover:underline">
              {m.title || '(untitled)'}
            </Link>
            {m.review === 'draft' && <Badge variant="warning">draft</Badge>}
          </li>
        ))}
      </ul>
    </div>
  );
}

// ── Review tab ────────────────────────────────────────────────────────────────

function ReviewTab() {
  const queue = useReviewQueue();
  const approve = useApprove();

  if (queue.isLoading) return <Spinner />;
  if (queue.isError) return <p className="text-sm text-danger">{errorMessage(queue.error)}</p>;

  const ideas = queue.data?.ideas ?? [];
  const documents = queue.data?.documents ?? [];
  if (ideas.length === 0 && documents.length === 0) {
    return <Card className="p-6 text-sm text-muted-foreground">Nothing pending review. 🎉</Card>;
  }

  const row = (type: 'idea' | 'document', m: MemberItem) => (
    <li
      key={`${type}:${m.id}`}
      className="flex items-center justify-between gap-3 rounded-md border border-border px-3 py-2"
    >
      <div className="flex min-w-0 items-center gap-2">
        <Badge variant="muted">{type}</Badge>
        <Link to={FEATURE_ROUTE[type]} className="truncate text-sm hover:underline">
          {m.title || '(untitled)'}
        </Link>
      </div>
      <Button
        size="sm"
        onClick={() => approve.mutate({ type, id: m.id })}
        disabled={approve.isPending}
      >
        <CheckCircle2 className="h-4 w-4" aria-hidden="true" /> Approve
      </Button>
    </li>
  );

  return (
    <div className="flex flex-col gap-2">
      <p className="text-sm text-muted-foreground">
        Agent-authored drafts awaiting your approval. Approving publishes the item.
      </p>
      <ul className="flex flex-col gap-2">
        {documents.map((m) => row('document', m))}
        {ideas.map((m) => row('idea', m))}
      </ul>
    </div>
  );
}

// ── Activity tab ──────────────────────────────────────────────────────────────

function ActivityTab() {
  const activity = useActivity();
  if (activity.isLoading) return <Spinner />;
  if (activity.isError) return <p className="text-sm text-danger">{errorMessage(activity.error)}</p>;
  const rows = activity.data ?? [];
  if (rows.length === 0) {
    return (
      <Card className="p-6 text-sm text-muted-foreground">
        No activity yet. Agent actions over MCP show up here.
      </Card>
    );
  }
  return (
    <ul className="flex flex-col">
      {rows.map((a) => (
        <li
          key={a.id}
          className="flex items-center justify-between gap-3 border-b border-border py-2 text-sm last:border-0"
        >
          <div className="flex min-w-0 items-center gap-2">
            <Badge variant={a.agent ? 'default' : 'muted'}>{a.agent ?? 'you'}</Badge>
            <span className="font-medium">{a.action}</span>
            {a.target_title && <span className="truncate text-muted-foreground">{a.target_title}</span>}
          </div>
          <time className="shrink-0 text-xs text-muted-foreground" dateTime={a.created_at}>
            {new Date(a.created_at).toLocaleString()}
          </time>
        </li>
      ))}
    </ul>
  );
}
