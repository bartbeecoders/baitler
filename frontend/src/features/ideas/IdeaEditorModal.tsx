import { useState, type FormEvent } from 'react';
import { Link2, Trash2, X } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { MarkdownEditor } from '@/components/ui/markdown-editor';
import { Modal } from '@/components/ui/modal';
import { Spinner } from '@/components/ui/spinner';
import {
  errorMessage,
  useCreateIdea,
  useDeleteIdea,
  useIdea,
  useLinkIdea,
  useUnlinkIdea,
  useUpdateIdea,
} from './api';
import { LinkPicker } from './LinkPicker';
import { StatusBadge } from './StatusBadge';
import { TagInput } from './TagInput';
import { STATUSES, STATUS_LABELS, type IdeaDetail, type IdeaStatus } from './types';

interface Props {
  ideaId: string | null; // null = create
  open: boolean;
  onClose: () => void;
}

/** Outer shell: fetches the idea (when editing) and mounts a fresh form per id. */
export function IdeaEditorModal({ ideaId, open, onClose }: Props) {
  const detail = useIdea(open ? ideaId : null);

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={ideaId ? 'Edit idea' : 'New idea'}
      className="max-w-2xl"
    >
      {ideaId && detail.isLoading ? (
        <div className="grid place-items-center py-10">
          <Spinner label="Loading idea" />
        </div>
      ) : ideaId && detail.isError ? (
        <p className="py-6 text-sm text-danger">{errorMessage(detail.error)}</p>
      ) : (
        <IdeaForm
          key={ideaId ?? 'new'}
          ideaId={ideaId}
          initial={detail.data ?? null}
          related={detail.data?.related ?? []}
          onClose={onClose}
        />
      )}
    </Modal>
  );
}

function IdeaForm({
  ideaId,
  initial,
  related,
  onClose,
}: {
  ideaId: string | null;
  initial: IdeaDetail | null;
  related: IdeaDetail['related'];
  onClose: () => void;
}) {
  const isEdit = ideaId !== null;
  const [title, setTitle] = useState(initial?.title ?? '');
  const [body, setBody] = useState(initial?.body ?? '');
  const [tags, setTags] = useState<string[]>(initial?.tags ?? []);
  const [status, setStatus] = useState<IdeaStatus>(initial?.status ?? 'inbox');
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  const create = useCreateIdea();
  const update = useUpdateIdea();
  const remove = useDeleteIdea();
  const link = useLinkIdea();
  const unlink = useUnlinkIdea();

  const saving = create.isPending || update.isPending;
  const error =
    create.error ?? update.error ?? remove.error ?? link.error ?? unlink.error ?? null;

  const submit = (e: FormEvent) => {
    e.preventDefault();
    const payload = { title: title.trim(), body, tags, status };
    if (isEdit) {
      update.mutate({ id: ideaId, patch: payload }, { onSuccess: onClose });
    } else {
      create.mutate(payload, { onSuccess: onClose });
    }
  };

  const excludeIds = [...(ideaId ? [ideaId] : []), ...related.map((r) => r.id)];

  return (
    <form onSubmit={submit} className="flex flex-col gap-4">
      <Input
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        placeholder="Idea title"
        aria-label="Idea title"
      />

      <MarkdownEditor value={body} onChange={setBody} placeholder="Write your idea in Markdown…" />

      <div className="flex flex-wrap items-center gap-3">
        <label className="flex items-center gap-2 text-sm">
          Status
          <select
            value={status}
            onChange={(e) => setStatus(e.target.value as IdeaStatus)}
            className="h-9 rounded-md border border-input bg-background px-2 text-sm"
          >
            {STATUSES.map((s) => (
              <option key={s} value={s}>
                {STATUS_LABELS[s]}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div>
        <p className="mb-1 text-sm font-medium">Tags</p>
        <TagInput tags={tags} onChange={setTags} />
      </div>

      {isEdit && (
        <div className="rounded-md border border-border p-3">
          <p className="mb-2 flex items-center gap-1.5 text-sm font-medium">
            <Link2 className="h-4 w-4" aria-hidden="true" />
            Linked ideas
          </p>
          {related.length > 0 ? (
            <ul className="mb-2 flex flex-col gap-1">
              {related.map((r) => (
                <li key={r.id} className="flex items-center justify-between gap-2 text-sm">
                  <span className="flex items-center gap-2">
                    {r.title}
                    <StatusBadge status={r.status} />
                  </span>
                  <button
                    type="button"
                    aria-label={`Unlink ${r.title}`}
                    onClick={() => unlink.mutate({ id: ideaId, targetId: r.id })}
                    className="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
                  >
                    <X className="h-4 w-4" />
                  </button>
                </li>
              ))}
            </ul>
          ) : (
            <p className="mb-2 text-sm text-muted-foreground">No linked ideas yet.</p>
          )}
          <LinkPicker
            excludeIds={excludeIds}
            onPick={(targetId) => link.mutate({ id: ideaId, targetId })}
          />
        </div>
      )}

      {error && <p className="text-sm text-danger">{errorMessage(error)}</p>}

      <div className="flex items-center justify-between gap-2">
        <div>
          {isEdit &&
            (confirmingDelete ? (
              <div className="flex items-center gap-2">
                <span className="text-sm text-danger">Delete?</span>
                <Button
                  type="button"
                  size="sm"
                  variant="danger"
                  disabled={remove.isPending}
                  onClick={() => remove.mutate(ideaId, { onSuccess: onClose })}
                >
                  Yes
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  onClick={() => setConfirmingDelete(false)}
                >
                  No
                </Button>
              </div>
            ) : (
              <Button
                type="button"
                size="sm"
                variant="ghost"
                onClick={() => setConfirmingDelete(true)}
              >
                <Trash2 className="h-4 w-4" aria-hidden="true" />
                Delete
              </Button>
            ))}
        </div>
        <div className="flex gap-2">
          <Button type="button" variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" disabled={!title.trim() || saving}>
            {saving ? 'Saving…' : 'Save'}
          </Button>
        </div>
      </div>
    </form>
  );
}
