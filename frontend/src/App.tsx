import { Route, Routes } from 'react-router-dom';

import { AppLayout } from '@/components/layout/AppLayout';
import { featureItems } from '@/config/navigation';
import { Dashboard } from '@/features/portal/Dashboard';
import { FilesPage } from '@/features/files/FilesPage';
import { FeaturePlaceholder } from '@/features/FeaturePlaceholder';
import { NotFound } from '@/features/NotFound';
import { useApplyTheme } from '@/stores/theme';

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
            element={item.path === '/files' ? <FilesPage /> : <FeaturePlaceholder item={item} />}
          />
        ))}
        <Route path="*" element={<NotFound />} />
      </Route>
    </Routes>
  );
}

export default App;
