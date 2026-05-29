import { lazy, type ReactNode } from 'react';
import { Route, Routes } from 'react-router-dom';

import { AppLayout } from '@/components/layout/AppLayout';
import { featureItems } from '@/config/navigation';
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

/** Feature routes that have a real implementation (others fall back to a placeholder). */
const FEATURE_PAGES: Record<string, ReactNode> = {
  '/files': <FilesPage />,
  '/ideas': <IdeasPage />,
  '/editor': <DocumentsPage />,
  '/ai': <AiPage />,
};

function App() {
  useApplyTheme();

  return (
    <Routes>
      <Route element={<AppLayout />}>
        <Route index element={<Dashboard />} />
        {featureItems.map((item) => (
          <Route
            key={item.path}
            path={item.path.slice(1)}
            element={FEATURE_PAGES[item.path] ?? <FeaturePlaceholder item={item} />}
          />
        ))}
        <Route path="*" element={<NotFound />} />
      </Route>
    </Routes>
  );
}

export default App;
