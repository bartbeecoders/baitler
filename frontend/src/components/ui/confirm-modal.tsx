import { Button } from '@/components/ui/button';
import { Modal } from '@/components/ui/modal';

interface Props {
  open: boolean;
  title: string;
  message: string;
  confirmLabel?: string;
  danger?: boolean;
  pending?: boolean;
  error?: string | null;
  onConfirm: () => void;
  onClose: () => void;
}

/** A confirm dialog built on the accessible Modal (Cancel takes initial focus). */
export function ConfirmModal({
  open,
  title,
  message,
  confirmLabel = 'Confirm',
  danger = false,
  pending = false,
  error = null,
  onConfirm,
  onClose,
}: Props) {
  return (
    <Modal open={open} onClose={onClose} title={title}>
      <div className="flex flex-col gap-4">
        <p className="text-sm text-muted-foreground">{message}</p>
        {error && <p className="text-sm text-danger">{error}</p>}
        <div className="flex justify-end gap-2">
          {/* Cancel first so the trap focuses the non-destructive action. */}
          <Button type="button" variant="outline" onClick={onClose} disabled={pending}>
            Cancel
          </Button>
          <Button
            type="button"
            variant={danger ? 'danger' : 'primary'}
            onClick={onConfirm}
            disabled={pending}
          >
            {pending ? 'Working…' : confirmLabel}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
