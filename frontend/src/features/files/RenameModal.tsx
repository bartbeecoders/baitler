import { useState, type FormEvent } from 'react';

import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Modal } from '@/components/ui/modal';
import { errorMessage, useRename, type ItemKind } from './api';

interface Props {
  open: boolean;
  onClose: () => void;
  kind: ItemKind;
  id: string;
  currentName: string;
}

// Callers should pass `key={id}` so a new target remounts this with a fresh
// initial name (no reset-in-effect needed).
export function RenameModal({ open, onClose, kind, id, currentName }: Props) {
  const [name, setName] = useState(currentName);
  const rename = useRename();

  const close = () => {
    rename.reset();
    onClose();
  };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    rename.mutate({ kind, id, name: name.trim() }, { onSuccess: close });
  };

  return (
    <Modal open={open} onClose={close} title={`Rename ${kind}`}>
      <form onSubmit={submit} className="flex flex-col gap-4">
        <Input
          value={name}
          onChange={(e) => setName(e.target.value)}
          aria-label="New name"
        />
        {rename.isError && <p className="text-sm text-danger">{errorMessage(rename.error)}</p>}
        <div className="flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={close}>
            Cancel
          </Button>
          <Button
            type="submit"
            disabled={!name.trim() || name.trim() === currentName || rename.isPending}
          >
            {rename.isPending ? 'Saving…' : 'Save'}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
