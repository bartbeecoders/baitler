import { FolderInput, ScanSearch, Sparkles, type LucideIcon } from 'lucide-react';

import type { ToolScope } from '@/features/cli/types';

/** A one-click butler errand: a prefilled prompt + the scope it needs. */
export interface QuickTask {
  key: string;
  label: string;
  icon: LucideIcon;
  /** Whether the task needs a granted local folder (opens the picker first). */
  needsFolder: boolean;
  toolScope: ToolScope;
  prompt: string;
}

export const QUICK_TASKS: QuickTask[] = [
  {
    key: 'analyze-code',
    label: 'Summarize a code project',
    icon: ScanSearch,
    needsFolder: true,
    toolScope: 'kb_plus_read',
    prompt: `Look at the code project in the granted folder. Use Glob/Grep/Read to understand its architecture, stack, and key modules. Then, in Baitler:
1. Create (or reuse) a project named after the repo, and a tagged summary document under it covering purpose, architecture, and the most important files.
2. Create one linked idea per notable module or insight, with precise tags.
3. Use files_import to bring a few representative files (README, key configs) into Baitler's Files under a matching folder.
Tag and categorize everything you create.`,
  },
  {
    key: 'ingest-docs',
    label: 'Ingest & organize a documents folder',
    icon: FolderInput,
    needsFolder: true,
    toolScope: 'kb_plus_read',
    prompt: `Look at the documents in the granted folder. Read them, understand what each one is, and then, in Baitler:
1. Create a tagged summary document describing the folder's contents overall.
2. Use files_import to bring the documents into Baitler's file structure under a sensibly named folder (e.g. an Archive/... path), preserving the originals on disk.
3. Create an idea per document (or per coherent group) with a one-paragraph summary and precise tags, linked to the imported files where useful.
4. Finish by suggesting which originals are now safe to archive on disk.
Annotate, tag, and categorize everything you process.`,
  },
  {
    key: 'tidy-kb',
    label: 'Tidy my knowledge base',
    icon: Sparkles,
    needsFolder: false,
    toolScope: 'kb_only',
    prompt: `Review my Baitler knowledge base. Use knowledge_search, the tag list, and the project list to find untagged or uncategorized ideas, documents, and pages. Propose and apply better tags, link related items with knowledge_link, and group orphans under suitable projects. Summarize what you changed at the end.`,
  },
];
