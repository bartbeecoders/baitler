import { useEffect } from 'react';
import { useLocation } from 'react-router-dom';
import { ChevronDown, ChevronRight, Boxes } from 'lucide-react';

import { OBJECT_TYPES } from '@/config/objects';
import { cn } from '@/lib/cn';
import { useObjectsNav } from '@/stores/objectsNav';
import {
  DiagramsList,
  DocumentsList,
  IdeasList,
  MindmapsList,
  PagesList,
  SuperpagesList,
} from './adapters';

/** Adapter component per object-type key (mounted lazily when a section expands). */
const OBJECT_LIST_COMPONENTS: Record<string, () => React.JSX.Element> = {
  documents: DocumentsList,
  ideas: IdeasList,
  pages: PagesList,
  mindmaps: MindmapsList,
  diagrams: DiagramsList,
  superpages: SuperpagesList,
};

/**
 * The "Objects" sidebar group: a collapsible header, and under it one collapsible
 * row per content type (Documents/Ideas/Pages/Mindmaps/Diagrams). Expanding a type
 * mounts its list adapter (search/filter/+/refresh/trash + the item list). The
 * adapter only mounts while open, so at most one list query runs at a time.
 */
export function ObjectsNav() {
  const { pathname } = useLocation();
  const groupOpen = useObjectsNav((s) => s.groupOpen);
  const openType = useObjectsNav((s) => s.openType);
  const toggleGroup = useObjectsNav((s) => s.toggleGroup);
  const setGroupOpen = useObjectsNav((s) => s.setGroupOpen);
  const toggleType = useObjectsNav((s) => s.toggleType);
  const setOpenType = useObjectsNav((s) => s.setOpenType);

  // When navigating to an object route, reveal that type (and the group).
  const matched = OBJECT_TYPES.find(
    (t) => pathname === t.basePath || pathname.startsWith(`${t.basePath}/`),
  );
  useEffect(() => {
    if (matched) {
      setGroupOpen(true);
      setOpenType(matched.key);
    }
  }, [matched, setGroupOpen, setOpenType]);

  return (
    <li>
      <button
        type="button"
        onClick={toggleGroup}
        aria-expanded={groupOpen}
        className="flex w-full items-center gap-3 rounded-md border-l-2 border-transparent px-3 py-2 text-sm font-medium text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
      >
        <Boxes className="h-5 w-5 shrink-0" aria-hidden="true" />
        <span className="flex-1 text-left">Objects</span>
        {groupOpen ? (
          <ChevronDown className="h-4 w-4 shrink-0" aria-hidden="true" />
        ) : (
          <ChevronRight className="h-4 w-4 shrink-0" aria-hidden="true" />
        )}
      </button>

      {groupOpen && (
        <ul className="mt-1 flex flex-col gap-1 pl-3">
          {OBJECT_TYPES.map((type) => {
            const expanded = openType === type.key;
            const isActive = matched?.key === type.key;
            const Adapter = OBJECT_LIST_COMPONENTS[type.key];
            return (
              <li key={type.key}>
                <button
                  type="button"
                  onClick={() => toggleType(type.key)}
                  aria-expanded={expanded}
                  className={cn(
                    'flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm transition-colors',
                    isActive
                      ? 'text-foreground'
                      : 'text-muted-foreground hover:bg-muted hover:text-foreground',
                  )}
                >
                  <type.icon className="h-4 w-4 shrink-0" aria-hidden="true" />
                  <span className="flex-1 text-left">{type.label}</span>
                  {expanded ? (
                    <ChevronDown className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
                  ) : (
                    <ChevronRight className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
                  )}
                </button>
                {expanded && Adapter && (
                  <div className="pl-1 pr-0 pt-1">
                    <Adapter />
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </li>
  );
}
