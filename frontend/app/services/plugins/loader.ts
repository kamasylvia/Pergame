/**
 * Plugin host kernel (PS-A): discovery, manifest gate, import, lifecycle.
 *
 * Loading order per design §4: read the manifest → validate contract and
 * capabilities → import the ESM entry → construct → `onload`. A plugin that
 * throws is recorded as failed and disabled, never taking another plugin or
 * the app down with it.
 */
import { createPluginContext, type PluginHostInfo, type PluginRequest } from './context';
import { createUiClient, clearPluginUi, type PluginUiClient } from './ui';
import { DisposableBag } from './registry';
import { validateManifest } from './manifest';
import {
  HOST_API_VERSION,
  type LoadedPlugin,
  type PluginConstructor,
  type PluginInstance,
  type PluginManifest,
  type UiClient,
} from '../../types/plugin';

/** One plugin in the host's table (loaded or failed). */
export interface PluginHostEntry {
  manifest: PluginManifest;
  status: 'loaded' | 'failed';
  /** Failure reason when `status === 'failed'`. */
  error?: string;
  loaded?: LoadedPlugin;
}

/** Dependencies of the host; the Nuxt boot plugin supplies the real ones. */
export interface PluginHostDeps {
  host: PluginHostInfo;
  request: PluginRequest;
  notify: (message: string, opts?: Parameters<UiClient['notice']>[1]) => void;
  /**
   * Build the per-plugin UI client. Defaults to the global reactive registry;
   * injected by tests that want an isolated one.
   */
  createUi?: (pluginId: string, track: (disposable: import('../../types/plugin').Disposable) => void) => PluginUiClient;
  /** ESM importer; injectable so the kernel is testable without a browser. */
  loadModule?: (url: string) => Promise<unknown>;
  /** Text fetcher for manifests (defaults to `fetch`). */
  fetchText?: (url: string) => Promise<string>;
}

/** Plugin host: one instance per app session. */
export interface PluginHost {
  readonly apiVersion: number;
  readonly entries: PluginHostEntry[];
  /** Load one plugin from a base URL holding `manifest.json` + entry. */
  loadFromUrl(baseUrl: string): Promise<PluginHostEntry>;
  /** Load a manifest that was already fetched (registry/install path). */
  load(manifest: PluginManifest, entryUrl: string): Promise<PluginHostEntry>;
  /** Unload by id (no-op when unknown). */
  unload(id: string): Promise<void>;
  /** Unload everything (app teardown / hot reload). */
  unloadAll(): Promise<void>;
}

const defaultLoadModule = (url: string): Promise<unknown> => import(/* @vite-ignore */ url);

/** Trailing-slash-free join for plugin-relative paths. */
function joinUrl(base: string, path: string): string {
  return `${base.replace(/\/+$/, '')}/${path.replace(/^\/+/, '')}`;
}

/** Placeholder manifest for a plugin whose own manifest could not be read. */
function stubManifest(source: string): PluginManifest {
  return {
    id: `invalid-${source}`,
    name: source,
    version: '0.0.0',
    apiVersion: String(HOST_API_VERSION),
    entry: 'main.mjs',
    capabilities: [],
  };
}

/** Create a host bound to the given transport and UI sinks. */
export function createPluginHost(deps: PluginHostDeps): PluginHost {
  const loadModule = deps.loadModule ?? defaultLoadModule;
  const fetchText =
    deps.fetchText ??
    (async (url: string) => {
      const response = await globalThis.fetch(url);
      if (!response.ok) throw new Error(`manifest ${response.status} for ${url}`);
      return response.text();
    });
  const entries: PluginHostEntry[] = [];

  async function load(manifest: PluginManifest, entryUrl: string): Promise<PluginHostEntry> {
    if (entries.some(e => e.manifest.id === manifest.id)) {
      return { manifest, status: 'failed', error: `plugin '${manifest.id}' is already registered` };
    }
    // Validated manifests already carry the host major; re-check because the
    // registry path may hand us an object that never went through validation.
    if (Number.parseInt(manifest.apiVersion, 10) !== HOST_API_VERSION) {
      const entry: PluginHostEntry = {
        manifest,
        status: 'failed',
        error: `apiVersion ${manifest.apiVersion} is not supported (host is ${HOST_API_VERSION})`,
      };
      entries.push(entry);
      return entry;
    }

    const bag = new DisposableBag();
    try {
      const module = await loadModule(entryUrl);
      const Ctor = (module as { default?: unknown }).default;
      if (typeof Ctor !== 'function') {
        throw new Error('entry has no default export (expected the Plugin class)');
      }
      const instance = new (Ctor as PluginConstructor)();
      if (typeof instance.onload !== 'function') {
        throw new Error('default export is not a Plugin (missing onload)');
      }
      const ui = (deps.createUi ?? createUiClient)(manifest.id, disposable => {
        bag.add(disposable);
      });
      const ctx = createPluginContext({
        manifest,
        version: HOST_API_VERSION,
        host: deps.host,
        request: deps.request,
        notify: deps.notify,
        ui,
      });
      await instance.onload(ctx);

      const loaded: LoadedPlugin = {
        manifest,
        instance,
        dispose: async () => {
          try {
            await instance.onunload?.();
          } catch (err) {
            console.warn(`[plugins] ${manifest.id} onunload threw`, err);
          }
          bag.dispose();
          clearPluginUi(manifest.id);
        },
      };
      const entry: PluginHostEntry = { manifest, status: 'loaded', loaded };
      entries.push(entry);
      return entry;
    } catch (err) {
      // Isolate: record the failure and reclaim anything the plugin managed to
      // register before it threw, so a bad plugin cannot poison the session.
      bag.dispose();
      const reason = err instanceof Error ? err.message : String(err);
      console.error(`[plugins] ${manifest.id} failed to load: ${reason}`, err);
      const entry: PluginHostEntry = { manifest, status: 'failed', error: reason };
      entries.push(entry);
      return entry;
    }
  }

  async function loadFromUrl(baseUrl: string): Promise<PluginHostEntry> {
    const manifestUrl = joinUrl(baseUrl, 'manifest.json');
    let raw: unknown;
    try {
      raw = JSON.parse(await fetchText(manifestUrl));
    } catch (err) {
      const reason = err instanceof Error ? err.message : String(err);
      console.error(`[plugins] cannot read ${manifestUrl}: ${reason}`);
      const entry: PluginHostEntry = { manifest: stubManifest(manifestUrl), status: 'failed', error: reason };
      entries.push(entry);
      return entry;
    }
    const validated = validateManifest(raw, deps.host.version);
    if (!validated.ok) {
      const entry: PluginHostEntry = {
        manifest: stubManifest(manifestUrl),
        status: 'failed',
        error: validated.errors.join('; '),
      };
      entries.push(entry);
      return entry;
    }
    return load(validated.manifest, joinUrl(baseUrl, validated.manifest.entry));
  }

  async function unload(id: string): Promise<void> {
    const index = entries.findIndex(e => e.manifest.id === id);
    if (index === -1) return;
    const removed = entries.splice(index, 1)[0];
    await removed?.loaded?.dispose();
  }

  return {
    apiVersion: HOST_API_VERSION,
    entries,
    loadFromUrl,
    load,
    unload,
    async unloadAll() {
      for (const id of entries.map(e => e.manifest.id)) await unload(id);
    },
  };
}

/** Re-exported for callers that only need the instance type. */
export type { PluginInstance };
