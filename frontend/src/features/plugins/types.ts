/** Data shapes for the plugin system (Phase 16). */

export type PluginStatus = 'draft' | 'approved' | 'enabled' | 'disabled' | 'rejected';

export interface PluginToolDecl {
  name: string;
  export: string;
  description?: string;
}

export interface PluginEndpointDecl {
  method: string;
  path: string;
  export: string;
}

export type PluginUiSlot = 'object' | 'rail' | 'butler_widget' | 'detail';

export interface PluginUiDecl {
  slot: PluginUiSlot;
  key: string;
  label?: string;
  /** Bundle-relative entry asset, e.g. `ui/index.html`. */
  entry: string;
}

export interface PluginCapabilities {
  host_fns?: string[];
  egress?: string[];
  secrets?: string[];
  memory_max_pages?: number;
  timeout_ms?: number;
}

export interface PluginManifest {
  name: string;
  version?: string;
  api_version?: number;
  description?: string;
  tools?: PluginToolDecl[];
  endpoints?: PluginEndpointDecl[];
  ui?: PluginUiDecl[];
  capabilities?: PluginCapabilities;
}

export interface Plugin {
  id: string;
  name: string;
  slug: string;
  description: string;
  version: string;
  version_int: number;
  api_version: number;
  manifest: PluginManifest | null;
  kinds: string[];
  status: PluginStatus;
  review: 'draft' | 'published';
  bundle_sha256: string;
  project_id: string | null;
  author_agent: string | null;
  run_id: string | null;
  created_at: string;
  updated_at: string;
}

/** One mountable UI component of an enabled plugin, resolved for a slot. */
export interface PluginUiMount {
  pluginId: string;
  slug: string;
  pluginName: string;
  slot: PluginUiSlot;
  key: string;
  label: string;
  entry: string;
}
