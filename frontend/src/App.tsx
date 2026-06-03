import { Fragment, lazy, type ReactNode } from 'react';
import { Route, Routes } from 'react-router-dom';

import { AppLayout } from '@/components/layout/AppLayout';
import { featureItems } from '@/config/navigation';
import { OBJECT_BASE_PATHS } from '@/config/objects';
import { Dashboard } from '@/features/portal/Dashboard';
import { FeaturePlaceholder } from '@/features/FeaturePlaceholder';
import { NotFound } from '@/features/NotFound';
import { useApplyTheme } from '@/stores/theme';

// Heavy feature pages are code-split so the initial bundle stays lean (the Ideas
// and Documents pages pull in the Markdown/editor libraries, etc.).
const FilesPage = lazy(() =>
  import('@/features/files/FilesPage').then((m) => ({ default: m.FilesPage })),
);
const IdeasPage = lazy(() =>
  import('@/features/ideas/IdeasPage').then((m) => ({ default: m.IdeasPage })),
);
const AiPage = lazy(() => import('@/features/ai/AiPage').then((m) => ({ default: m.AiPage })));
const DocumentsPage = lazy(() =>
  import('@/features/documents/DocumentsPage').then((m) => ({ default: m.DocumentsPage })),
);
const ProjectsPage = lazy(() =>
  import('@/features/projects/ProjectsPage').then((m) => ({ default: m.ProjectsPage })),
);
const PagesPage = lazy(() =>
  import('@/features/pages/PagesPage').then((m) => ({ default: m.PagesPage })),
);
const AgentPage = lazy(() =>
  import('@/features/cli/AgentPage').then((m) => ({ default: m.AgentPage })),
);
const MindmapsPage = lazy(() =>
  import('@/features/mindmaps/MindmapsPage').then((m) => ({ default: m.MindmapsPage })),
);
const DiagramsPage = lazy(() =>
  import('@/features/diagrams/DiagramsPage').then((m) => ({ default: m.DiagramsPage })),
);

/** Non-object feature routes that have a real implementation (others → placeholder). */
const FEATURE_PAGES: Record<string, ReactNode> = {
  '/files': <FilesPage />,
  '/projects': <ProjectsPage />,
  '/agent': <AgentPage />,
  '/ai': <AiPage />,
};

/**
 * Object types now live in the sidebar "Objects" list; their routes are explicit
 * `base` + `base/:id` pairs so a selected item deep-links to its editor while the
 * list stays in the sidebar.
 */
const OBJECT_ELEMENTS: Record<string, ReactNode> = {
  '/editor': <DocumentsPage />,
  '/ideas': <IdeasPage />,
  '/pages': <PagesPage />,
  '/mindmaps': <MindmapsPage />,
  '/diagrams': <DiagramsPage />,
};

function App() {
  useApplyTheme();

  // Non-object features still render from the flat nav (dashboard cards + routes).
  const nonObjectFeatures = featureItems.filter((item) => !OBJECT_BASE_PATHS.includes(item.path));

  return (
    <Routes>
      <Route element={<AppLayout />}>
        <Route index element={<Dashboard />} />
        {nonObjectFeatures.map((item) => (
          <Route
            key={item.path}
            path={item.path.slice(1)}
            element={FEATURE_PAGES[item.path] ?? <FeaturePlaceholder item={item} />}
          />
        ))}
        {OBJECT_BASE_PATHS.map((base) => (
          <Fragment key={base}>
            <Route path={base.slice(1)} element={OBJECT_ELEMENTS[base]} />
            {base === '/ideas' && <Route path="ideas/new" element={OBJECT_ELEMENTS[base]} />}
            <Route path={`${base.slice(1)}/:id`} element={OBJECT_ELEMENTS[base]} />
          </Fragment>
        ))}
        <Route path="*" element={<NotFound />} />
      </Route>
    </Routes>
  );
}

export default App;
