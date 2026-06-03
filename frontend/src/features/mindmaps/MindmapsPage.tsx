import { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import {
  addEdge,
  Background,
  Controls,
  ReactFlow,
  useEdgesState,
  useNodesState,
  useReactFlow,
  type Connection,
  type Edge,
  type Node,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { Plus, Trash2 } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { ConfirmModal } from '@/components/ui/confirm-modal';
import { Input } from '@/components/ui/input';
import { Spinner } from '@/components/ui/spinner';
import { ExportMenu } from '@/features/documents/ExportMenu';
import { TagInput } from '@/features/ideas/TagInput';
import { EmptyDetail } from '@/features/objects/EmptyDetail';
import { OBJECT_CANVAS, OBJECT_EDITOR_ROOT } from '@/features/objects/editorLayout';
import { cn } from '@/lib/cn';
import { errorMessage, useDeleteMindmap, useMindmap, useUpdateMindmap } from './api';
import type { Mindmap, MindmapGraph } from './types';

/** Convert our stored graph to React Flow nodes/edges (auto-laying out any node without a position). */
function toFlow(graph: MindmapGraph): { nodes: Node[]; edges: Edge[] } {
  const nodes: Node[] = (graph.nodes ?? []).map((n, i) => ({
    id: n.id,
    position: { x: n.x ?? (i % 5) * 200, y: n.y ?? Math.floor(i / 5) * 130 },
    data: { label: n.label },
  }));
  const edges: Edge[] = (graph.edges ?? []).map((e, i) => ({
    id: `e${i}-${e.from}-${e.to}`,
    source: e.from,
    target: e.to,
    label: e.label ?? undefined,
  }));
  return { nodes, edges };
}

/** Convert React Flow state back to our stored graph (positions preserved). */
function fromFlow(nodes: Node[], edges: Edge[]): MindmapGraph {
  return {
    nodes: nodes.map((n) => ({
      id: n.id,
      label: String((n.data as { label?: unknown })?.label ?? ''),
      x: Math.round(n.position.x),
      y: Math.round(n.position.y),
    })),
    edges: edges.map((e) => ({ from: e.source, to: e.target })),
  };
}



/**
 * Editor-only mindmap route. The mindmap list lives in the sidebar "Objects"
 * group; this renders the canvas editor for `/mindmaps/:id` (or a hint).
 */
export function MindmapsPage() {
  const { id } = useParams();
  const navigate = useNavigate();
  const detail = useMindmap(id ?? null);

  if (!id) return <EmptyDetail noun="mindmap" />;
  if (detail.isLoading) {
    return (
      <div className="grid h-[calc(100svh-12rem)] place-items-center">
        <Spinner label="Loading mindmap" />
      </div>
    );
  }
  if (detail.isError) {
    return (
      <p className="text-sm text-danger" role="alert">
        {errorMessage(detail.error)}
      </p>
    );
  }
  if (!detail.data) return <EmptyDetail noun="mindmap" />;

  return (
    <MindmapEditor key={detail.data.id} mindmap={detail.data} onDeleted={() => navigate('/mindmaps')} />
  );
}


/** Refit the graph when the canvas pane is resized (fullscreen, dock toggle, etc.). */
function FitViewOnResize() {
  const { fitView } = useReactFlow();
  useEffect(() => {
    if (typeof ResizeObserver === 'undefined') return;
    const el = document.querySelector('[data-testid="mindmap-canvas"]');
    if (!el) return;
    let raf = 0;
    const ro = new ResizeObserver(() => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => void fitView({ padding: 0.08, duration: 0 }));
    });
    ro.observe(el);
    return () => {
      ro.disconnect();
      cancelAnimationFrame(raf);
    };
  }, [fitView]);
  return null;
}

function MindmapEditor({ mindmap, onDeleted }: { mindmap: Mindmap; onDeleted: () => void }) {
  const initial = useMemo(() => toFlow(mindmap.graph), [mindmap.graph]);
  const [nodes, setNodes, onNodesChange] = useNodesState(initial.nodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState(initial.edges);
  const [title, setTitle] = useState(mindmap.title);
  const [tags, setTags] = useState<string[]>(mindmap.tags ?? []);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [nextId, setNextId] = useState(initial.nodes.length + 1);

  const update = useUpdateMindmap();
  const remove = useDeleteMindmap();

  const onConnect = useCallback(
    (c: Connection) => setEdges((eds) => addEdge(c, eds)),
    [setEdges],
  );

  const addNode = () => {
    const id = `n${nextId}`;
    setNextId((v) => v + 1);
    setNodes((nds) => [
      ...nds,
      { id, position: { x: 80 + nds.length * 24, y: 80 + nds.length * 16 }, data: { label: 'New node' } },
    ]);
  };

  // Debounced autosave of the graph + title + tags.
  const graphKey = JSON.stringify(fromFlow(nodes, edges));
  const tagsKey = JSON.stringify(tags);
  const save = update.mutate;
  useEffect(() => {
    const dirty =
      title !== mindmap.title ||
      tagsKey !== JSON.stringify(mindmap.tags ?? []) ||
      graphKey !== JSON.stringify(mindmap.graph);
    if (!dirty) return;
    const timer = setTimeout(
      () =>
        save({
          id: mindmap.id,
          patch: { title: title.trim() || 'Untitled map', graph: fromFlow(nodes, edges), tags },
        }),
      800,
    );
    return () => clearTimeout(timer);
  }, [graphKey, tagsKey, title, nodes, edges, tags, mindmap, save]);

  // Outline export: a Markdown bullet list of node labels (a simple flattening).
  const outline = useMemo(
    () => nodes.map((n) => `- ${String((n.data as { label?: unknown }).label ?? '')}`).join('\n'),
    [nodes],
  );

  const status = update.isPending ? 'Saving…' : `Saved · v${mindmap.version}`;

  return (
    <div className={OBJECT_EDITOR_ROOT}>
      <div className="flex shrink-0 flex-wrap items-center gap-2">
        <Input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          aria-label="Mindmap title"
          className="flex-1 text-lg font-semibold"
        />
        <span className="text-xs text-muted-foreground" aria-live="polite">
          {status}
        </span>
        <Button variant="outline" onClick={addNode}>
          <Plus className="h-4 w-4" aria-hidden="true" />
          Add node
        </Button>
        <ExportMenu content={`# ${title}\n\n${outline}`} source="markdown" filename={title} />
        <Button
          variant="outline"
          size="icon"
          aria-label="Delete mindmap"
          onClick={() => setConfirmDelete(true)}
        >
          <Trash2 className="h-4 w-4" />
        </Button>
      </div>

      <div className="shrink-0">
        <TagInput tags={tags} onChange={setTags} />
      </div>

      <div className={cn(OBJECT_CANVAS, 'bg-background')} data-testid="mindmap-canvas">
        <div className="absolute inset-0">
          <ReactFlow
            nodes={nodes}
            edges={edges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            fitView
            proOptions={{ hideAttribution: true }}
          >
          <Background />
          <Controls />
          <FitViewOnResize />
        </ReactFlow>
        </div>
      </div>

      <ConfirmModal
        open={confirmDelete}
        title="Delete mindmap"
        message={`Delete "${mindmap.title}"? This cannot be undone.`}
        confirmLabel="Delete"
        danger
        pending={remove.isPending}
        error={remove.isError ? errorMessage(remove.error) : null}
        onConfirm={() =>
          remove.mutate(mindmap.id, {
            onSuccess: () => {
              setConfirmDelete(false);
              onDeleted();
            },
          })
        }
        onClose={() => setConfirmDelete(false)}
      />
    </div>
  );
}
