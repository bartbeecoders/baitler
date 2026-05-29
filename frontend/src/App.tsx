import { lazy } from 'react';
import { Route, Routes } from 'react-router-dom';

import { AppLayout } from '@/components/layout/AppLayout';
import { featureItems } from '@/config/navigation';
import { Dashboard } from '@/features/portal/Dashboard';
import { FeaturePlaceholder } from '@/features/FeaturePlaceholder';
import { NotFound } from '@/features/NotFound';
import { useApplyTheme } from '@/stores/theme';

// Heavy feature pages are code-split so the initial bundle stays lean (the Ideas
// page pulls in the Markdown renderer, etc.).
const FilesPage = lazy(() =>
  import('@/features/files/FilesPage').then((m) => ({ default: m.FilesPage })),
);
const IdeasPage = lazy(() =>
  import('@/features/ideas/IdeasPage').then((m) => ({ default: m.IdeasPage })),
);

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
            element={
              item.path === '/files' ? (
                <FilesPage />
              ) : item.path === '/ideas' ? (
                <IdeasPage />
              ) : (
                <FeaturePlaceholder item={item} />
              )
            }
          />
        ))}
        <Route path="*" element={<NotFound />} />
      </Route>
    </Routes>
  );
}

export default App;
