import { useMemo, useState } from 'react';
import { Link, useParams } from 'react-router-dom';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { ConfirmModal } from '@/components/ui/confirm-modal';
import { Spinner } from '@/components/ui/spinner';
import {
  errorMessage,
  useApprovePlugin,
  useDisablePlugin,
  useEnablePlugin,
  usePlugins,
  useRejectPlugin,
  useUninstallPlugin,
} from './api';
import { PluginFrame } from './PluginFrame';
import type { Plugin, PluginStatus } from './types';

const STATUS_VARIANT: Record<PluginStatus, 'default' | 'success' | 'warning' | 'danger' | 'muted'> =
  {
    draft: 'warning',
    approved: 'default',
    enabled: 'success',
    disabled: 'muted',
    rejected: 'danger',
  };

/**
 * `/plugins` — management list — and `/plugins/:slug` — the full-pane frame of
 * a plugin's `detail` UI component. Approve/enable/disable/reject/uninstall
 * are PORTAL verbs: this page is the human gate the MCP surface deliberately
 * doesn't have.
 */
export function PluginsPage() {
  const { id: slug } = useParams();
  if (slug) return <PluginDetail slug={slug} />;
  return <PluginList />;
}

function PluginList() {
  const { data: plugins, isLoading, error } = usePlugins();

  if (isLoading) return <Spinner className="mx-auto mt-12" />;
  if (error) {
    return (
      <div className="mx-auto mt-12 max-w-xl text-center text-sm text-muted-foreground">
        <p className="font-medium text-foreground">Plugins unavailable</p>
        <p className="mt-2">{errorMessage(error)}</p>
      </div>
    );
  }

  const drafts = plugins?.filter((p) => p.status === 'draft') ?? [];
  const rest = plugins?.filter((p) => p.status !== 'draft') ?? [];

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-6">
      <header>
        <h1 className="text-xl font-semibold">Plugins</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Agent-authored extensions. Drafts wait here for your review — nothing runs until you
          approve and enable it.
        </p>
      </header>

      {drafts.length > 0 && (
        <section aria-label="Awaiting review" className="flex flex-col gap-3">
          <h2 className="text-sm font-medium text-muted-foreground">
            Awaiting review ({drafts.length})
          </h2>
          {drafts.map((p) => (
            <PluginCard key={p.id} plugin={p} />
          ))}
        </section>
      )}

      <section aria-label="Installed" className="flex flex-col gap-3">
        {drafts.length > 0 && rest.length > 0 && (
          <h2 className="text-sm font-medium text-muted-foreground">Installed</h2>
        )}
        {rest.map((p) => (
          <PluginCard key={p.id} plugin={p} />
        ))}
        {plugins?.length === 0 && (
          <p className="text-sm text-muted-foreground">
            No plugins yet. Ask the butler to build one — it can scaffold, test, and submit a
            plugin for your review entirely on its own.
          </p>
        )}
      </section>
    </div>
  );
}

function PluginCard({ plugin }: { plugin: Plugin }) {
  const approve = useApprovePlugin();
  const enable = useEnablePlugin();
  const disable = useDisablePlugin();
  const reject = useRejectPlugin();
  const uninstall = useUninstallPlugin();
  const [confirmUninstall, setConfirmUninstall] = useState(false);

  const caps = plugin.manifest?.capabilities;
  const detailMount = (plugin.manifest?.ui ?? []).some((u) => u.slot === 'detail');
  const actionError =
    approve.error ?? enable.error ?? disable.error ?? reject.error ?? uninstall.error;

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between gap-2">
        <CardTitle className="flex items-center gap-2 text-base">
          {plugin.name}
          <span className="text-xs font-normal text-muted-foreground">v{plugin.version}</span>
          <Badge variant={STATUS_VARIANT[plugin.status]}>{plugin.status}</Badge>
        </CardTitle>
        <div className="flex items-center gap-2">
          {plugin.status === 'draft' && (
            <>
              <Button
                size="sm"
                onClick={() => approve.mutate(plugin.id)}
                disabled={approve.isPending}
              >
                Approve
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={() => reject.mutate(plugin.id)}
                disabled={reject.isPending}
              >
                Reject
              </Button>
            </>
          )}
          {(plugin.status === 'approved' || plugin.status === 'disabled') && (
            <Button size="sm" onClick={() => enable.mutate(plugin.id)} disabled={enable.isPending}>
              Enable
            </Button>
          )}
          {plugin.status === 'enabled' && (
            <Button
              size="sm"
              variant="outline"
              onClick={() => disable.mutate(plugin.id)}
              disabled={disable.isPending}
            >
              Disable
            </Button>
          )}
          {plugin.status !== 'draft' && (
            <Button size="sm" variant="outline" onClick={() => setConfirmUninstall(true)}>
              Uninstall
            </Button>
          )}
        </div>
      </CardHeader>
      <CardContent className="flex flex-col gap-2 text-sm">
        {plugin.description && <p className="text-muted-foreground">{plugin.description}</p>}

        {/* The permission diff: exactly what this plugin may touch. */}
        <div className="flex flex-wrap items-center gap-1.5" aria-label="Capabilities">
          {(caps?.host_fns ?? []).map((fn) => (
            <Badge key={fn} variant="warning">
              {fn}
            </Badge>
          ))}
          {(plugin.manifest?.tools ?? []).length > 0 && (
            <Badge variant="muted">{plugin.manifest?.tools?.length} tool(s)</Badge>
          )}
          {(plugin.manifest?.endpoints ?? []).length > 0 && (
            <Badge variant="muted">{plugin.manifest?.endpoints?.length} endpoint(s)</Badge>
          )}
          {(plugin.manifest?.ui ?? []).length > 0 && (
            <Badge variant="muted">{plugin.manifest?.ui?.length} UI</Badge>
          )}
          <Badge variant="muted">
            {caps?.memory_max_pages ?? 256}p · {caps?.timeout_ms ?? 2000}ms
          </Badge>
        </div>

        <p className="text-xs text-muted-foreground">
          {plugin.author_agent ? `authored by ${plugin.author_agent}` : 'authored in the portal'}
          {plugin.run_id ? ` · run ${plugin.run_id.slice(0, 8)}` : ''}
          {` · sha256 ${plugin.bundle_sha256.slice(0, 12)}…`}
        </p>

        {plugin.status === 'enabled' && detailMount && (
          <Link to={`/plugins/${plugin.slug}`} className="text-xs text-primary underline">
            Open plugin UI
          </Link>
        )}
        {actionError && <p className="text-xs text-danger">{errorMessage(actionError)}</p>}
      </CardContent>

      <ConfirmModal
        open={confirmUninstall}
        title={`Uninstall ${plugin.name}?`}
        message="This removes the plugin, its knowledge links, and its stored data. Content it created (ideas, pages) stays."
        confirmLabel="Uninstall"
        danger
        pending={uninstall.isPending}
        error={uninstall.error ? errorMessage(uninstall.error) : null}
        onConfirm={() =>
          uninstall.mutate(plugin.id, { onSuccess: () => setConfirmUninstall(false) })
        }
        onClose={() => setConfirmUninstall(false)}
      />
    </Card>
  );
}

/** `/plugins/:slug` — the plugin's `detail` UI component, full pane. */
function PluginDetail({ slug }: { slug: string }) {
  const { data: plugins, isLoading } = usePlugins();
  const plugin = plugins?.find((p) => p.slug === slug || p.id === slug);
  const mount = useMemo(() => {
    if (!plugin || plugin.status !== 'enabled') return null;
    const decl = (plugin.manifest?.ui ?? []).find((u) => u.slot === 'detail');
    if (!decl) return null;
    return {
      pluginId: plugin.id,
      slug: plugin.slug,
      pluginName: plugin.name,
      slot: decl.slot,
      key: decl.key,
      label: decl.label || plugin.name,
      entry: decl.entry,
    };
  }, [plugin]);

  if (isLoading) return <Spinner className="mx-auto mt-12" />;
  if (!plugin) {
    return (
      <p className="mx-auto mt-12 text-sm text-muted-foreground">No plugin “{slug}”.</p>
    );
  }
  if (!mount) {
    return (
      <p className="mx-auto mt-12 text-sm text-muted-foreground">
        {plugin.status === 'enabled'
          ? `${plugin.name} has no detail UI component.`
          : `${plugin.name} is ${plugin.status} — enable it under `}
        {plugin.status !== 'enabled' && (
          <Link to="/plugins" className="text-primary underline">
            Plugins
          </Link>
        )}
      </p>
    );
  }
  return (
    <div className="flex h-[calc(100vh-8rem)] flex-col gap-2">
      <header className="flex items-center justify-between">
        <h1 className="text-lg font-semibold">{mount.label}</h1>
        <Link to="/plugins" className="text-xs text-muted-foreground underline">
          All plugins
        </Link>
      </header>
      <PluginFrame mount={mount} className="min-h-0 w-full flex-1 rounded-md border border-border bg-background" />
    </div>
  );
}
