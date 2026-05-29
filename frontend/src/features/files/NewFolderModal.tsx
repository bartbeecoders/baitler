import { useState, type FormEvent } from 'react';

import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Modal } from '@/components/ui/modal';
import { errorMessage, useCreateFolder } from './api';

interface Props {
  open: boolean;
  onClose: () => void;
  parentId: string | null;
}

export function NewFolderModal({ open, onClose, parentId }: Props) {
  const [name, setName] = useState('');
  const create = useCreateFolder();

  const close = () => {
    create.reset();
    setName('');
    onClose();
  };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    create.mutate({ name: name.trim(), parentId }, { onSuccess: close });
  };

  return (
    <Modal open={open} onClose={close} title="New folder">
      <form onSubmit={submit} className="flex flex-col gap-4">
        <Input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Folder name"
          aria-label="Folder name"
        />
        {create.isError && <p className="text-sm text-danger">{errorMessage(create.error)}</p>}
        <div className="flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={close}>
            Cancel
          </Button>
          <Button type="submit" disabled={!name.trim() || create.isPending}>
            {create.isPending ? 'Creating…' : 'Create'}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
