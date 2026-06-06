import { Component, type ReactNode } from 'react';
import { Link, NavLink } from 'react-router-dom';
import { Puzzle } from 'lucide-react';

import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { cn } from '@/lib/cn';
import { uiMounts, useEnabledPlugins } from './api';
import { PluginFrame } from './PluginFrame';

/**
 * Chrome mounts for enabled plugins' UI components (Phase 16.C). Every surface
 * here is failure-silent — a disabled plugin system, an offline backend, or a
 * crashing plugin must never take app chrome down with it — so each mount is
 * wrapped in an error boundary and the query never surfaces errors.
 */

class SlotBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  render() {
    return this.state.failed ? null : this.props.children;
  }
}

/** `rail` slot: one icon per mount, linking to the plugin's detail surface. */
export function PluginRailItems() {
  const { data } = useEnabledPlugins();
  const mounts = uiMounts(data, 'rail');
  if (mounts.length === 0) return null;
  return (
    <SlotBoundary>
      {mounts.map((m) => (
        <NavLink
          key={`${m.slug}:${m.key}`}
          to={`/plugins/${m.slug}`}
          title={m.label}
          aria-label={m.label}
          className={({ isActive }) =>
            cn(
              'grid h-11 w-11 place-items-center rounded-lg transition-colors',
              isActive
                ? 'bg-accent text-accent-foreground'
                : 'text-muted-foreground hover:bg-muted hover:text-foreground',
            )
          }
        >
          <Puzzle className="h-5 w-5" aria-hidden="true" />
        </NavLink>
      ))}
    </SlotBoundary>
  );
}

/** `butler_widget` slot: compact framed cards on the butler home. */
export function PluginWidgets() {
  const { data } = useEnabledPlugins();
  const mounts = uiMounts(data, 'butler_widget');
  if (mounts.length === 0) return null;
  return (
    <SlotBoundary>
      <section aria-label="Plugin widgets" className="flex w-full flex-col gap-3">
        {mounts.map((m) => (
          <Card key={`${m.slug}:${m.key}`}>
            <CardHeader className="py-3">
              <CardTitle className="flex items-center gap-2 text-sm">
                <Puzzle className="h-4 w-4 text-muted-foreground" aria-hidden="true" />
                {m.label}
              </CardTitle>
            </CardHeader>
            <CardContent className="pt-0">
              <PluginFrame mount={m} className="h-64 w-full rounded-md border border-border" />
            </CardContent>
          </Card>
        ))}
      </section>
    </SlotBoundary>
  );
}

/** `object` slot: rows in the Objects flyout, deep-linking to the detail UI. */
export function PluginObjectItems() {
  const { data } = useEnabledPlugins();
  const mounts = uiMounts(data, 'object');
  if (mounts.length === 0) return null;
  return (
    <SlotBoundary>
      {mounts.map((m) => (
        <li key={`${m.slug}:${m.key}`}>
          <Link
            to={`/plugins/${m.slug}`}
            className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
          >
            <Puzzle className="h-4 w-4 shrink-0" aria-hidden="true" />
            <span className="flex-1 text-left">{m.label}</span>
          </Link>
        </li>
      ))}
    </SlotBoundary>
  );
}
