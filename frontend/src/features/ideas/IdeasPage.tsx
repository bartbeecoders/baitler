import { useLocation, useNavigate, useParams } from 'react-router-dom';

import { EmptyDetail } from '@/features/objects/EmptyDetail';
import { IdeaEditorModal } from './IdeaEditorModal';

/**
 * Editor-only ideas route. The idea list now lives in the sidebar "Objects"
 * group; this route opens the idea editor modal for `/ideas/:id` (edit) or
 * `/ideas/new` (create), over a hint in the main area. Closing returns to
 * `/ideas`.
 */
export function IdeasPage() {
  const { id } = useParams();
  const { pathname } = useLocation();
  const navigate = useNavigate();
  const isNew = pathname === '/ideas/new';
  const open = isNew || !!id;

  return (
    <>
      <EmptyDetail noun="idea" />
      <IdeaEditorModal
        ideaId={id ?? null}
        open={open}
        onClose={() => navigate('/ideas')}
      />
    </>
  );
}
