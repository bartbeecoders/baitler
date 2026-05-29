import { Badge } from '@/components/ui/badge';
import { STATUS_LABELS, type IdeaStatus } from './types';

const VARIANT: Record<IdeaStatus, 'muted' | 'warning' | 'success'> = {
  inbox: 'muted',
  active: 'warning',
  done: 'success',
  archived: 'muted',
};

export function StatusBadge({ status }: { status: IdeaStatus }) {
  return <Badge variant={VARIANT[status]}>{STATUS_LABELS[status]}</Badge>;
}
