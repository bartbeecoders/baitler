import { useEffect, useState } from 'react';
import { Download } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { Modal } from '@/components/ui/modal';
import { Spinner } from '@/components/ui/spinner';
import { downloadContent, fetchContent } from './api';
import type { FileItem } from './types';
import { formatBytes, isPreviewableImage } from './util';

interface Props {
  file: FileItem | null;
  onClose: () => void;
}

export function PreviewModal({ file, onClose }: Props) {
  const [objectUrl, setObjectUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  // Fetch image bytes through the credentialed path and expose as an object URL.
  // (Parent remounts via `key`, so initial state is fresh per file — no reset.)
  useEffect(() => {
    if (!file || !isPreviewableImage(file.mime)) return;

    let active = true;
    let url: string | null = null;
    fetchContent(file.id)
      .then((blob) => {
        if (!active) return;
        url = URL.createObjectURL(blob);
        setObjectUrl(url);
      })
      .catch(() => {
        if (active) setFailed(true);
      });

    return () => {
      active = false;
      if (url) URL.revokeObjectURL(url);
    };
  }, [file]);

  if (!file) return null;
  const canPreview = isPreviewableImage(file.mime) && !failed;

  return (
    <Modal open={!!file} onClose={onClose} title={file.name} className="max-w-2xl">
      <div className="flex flex-col gap-4">
        {canPreview ? (
          objectUrl ? (
            <img
              src={objectUrl}
              alt=""
              className="max-h-[60vh] w-full rounded-md object-contain"
              onError={() => setFailed(true)}
            />
          ) : (
            <div className="grid h-40 place-items-center">
              <Spinner label="Loading preview" />
            </div>
          )
        ) : (
          <p className="text-sm text-muted-foreground">
            No inline preview for this file type ({file.mime}).
          </p>
        )}
        <div className="flex items-center justify-between">
          <span className="text-sm text-muted-foreground">{formatBytes(file.size)}</span>
          <Button onClick={() => void downloadContent(file.id, file.name)}>
            <Download className="h-4 w-4" aria-hidden="true" />
            Download
          </Button>
        </div>
      </div>
    </Modal>
  );
}
