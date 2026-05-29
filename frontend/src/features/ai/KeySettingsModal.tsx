import { useState } from 'react';
import { Check } from 'lucide-react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Modal } from '@/components/ui/modal';
import { errorMessage, useDeleteKey, useProviders, useSetKey } from './api';

export function KeySettingsModal({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { data: providers = [] } = useProviders();
  const setKey = useSetKey();
  const deleteKey = useDeleteKey();
  const [inputs, setInputs] = useState<Record<string, string>>({});

  const realProviders = providers.filter((p) => p.requires_key);
  const error = setKey.error ?? deleteKey.error;

  return (
    <Modal open={open} onClose={onClose} title="AI provider keys" className="max-w-lg">
      <div className="flex flex-col gap-4">
        <p className="text-sm text-muted-foreground">
          Keys are stored encrypted and never shown again. The Mock provider needs no key.
        </p>
        {realProviders.map((p) => (
          <div key={p.id} className="flex flex-col gap-2 rounded-md border border-border p-3">
            <div className="flex items-center justify-between">
              <span className="font-medium">{p.label}</span>
              {p.configured ? (
                <Badge variant="success">
                  <Check className="h-3 w-3" aria-hidden="true" />
                  Configured
                </Badge>
              ) : (
                <Badge variant="muted">Not set</Badge>
              )}
            </div>
            <div className="flex gap-2">
              <Input
                type="password"
                value={inputs[p.id] ?? ''}
                onChange={(e) => setInputs((prev) => ({ ...prev, [p.id]: e.target.value }))}
                placeholder={`${p.label} API key`}
                aria-label={`${p.label} API key`}
              />
              <Button
                onClick={() =>
                  setKey.mutate(
                    { provider: p.id, apiKey: (inputs[p.id] ?? '').trim() },
                    { onSuccess: () => setInputs((prev) => ({ ...prev, [p.id]: '' })) },
                  )
                }
                disabled={!(inputs[p.id] ?? '').trim() || setKey.isPending}
              >
                Save
              </Button>
              {p.configured && (
                <Button
                  variant="outline"
                  onClick={() => deleteKey.mutate(p.id)}
                  disabled={deleteKey.isPending}
                >
                  Remove
                </Button>
              )}
            </div>
          </div>
        ))}
        {error && <p className="text-sm text-danger">{errorMessage(error)}</p>}
        <div className="flex justify-end">
          <Button variant="outline" onClick={onClose}>
            Done
          </Button>
        </div>
      </div>
    </Modal>
  );
}
