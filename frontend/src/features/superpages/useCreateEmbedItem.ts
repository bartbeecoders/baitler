import { useCreateDiagram } from '@/features/diagrams/api';
import { useCreateDocument } from '@/features/documents/api';
import { useCreateIdea } from '@/features/ideas/api';
import { useCreateMindmap } from '@/features/mindmaps/api';
import { useCreatePage } from '@/features/pages/api';
import type { EmbedItemType } from './types';

/** Embeddable types that can be authored fresh from the canvas (files need an upload). */
export const CREATABLE_TYPES: EmbedItemType[] = [
  'idea',
  'document',
  'page',
  'mindmap',
  'diagram',
];

/**
 * Create a blank item of the given type and return its id, so a superpage can
 * compose brand-new content — not just reference what already exists.
 */
export function useCreateEmbedItem() {
  const idea = useCreateIdea();
  const doc = useCreateDocument();
  const page = useCreatePage();
  const mindmap = useCreateMindmap();
  const diagram = useCreateDiagram();

  const pending =
    idea.isPending ||
    doc.isPending ||
    page.isPending ||
    mindmap.isPending ||
    diagram.isPending;

  async function create(type: EmbedItemType): Promise<string | null> {
    switch (type) {
      case 'idea':
        return (
          await idea.mutateAsync({ title: 'Untitled idea', body: '', tags: [], status: 'inbox' })
        ).id;
      case 'document':
        return (await doc.mutateAsync({ title: 'Untitled document' })).id;
      case 'page':
        return (await page.mutateAsync({ title: 'Untitled page' })).id;
      case 'mindmap':
        return (
          await mindmap.mutateAsync({ title: 'Untitled mindmap', graph: { nodes: [], edges: [] } })
        ).id;
      case 'diagram':
        return (await diagram.mutateAsync({ title: 'Untitled diagram' })).id;
      default:
        return null; // files are uploaded, not created blank
    }
  }

  return { create, pending };
}
