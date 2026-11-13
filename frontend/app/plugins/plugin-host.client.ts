/**
 * Boot the plugin host on the client (PS-D).
 *
 * Two sources feed the same host:
 * - the registry (`GET /api/plugins`): installed plugins the user approved,
 *   loaded from their served entry URL with only the granted capabilities;
 * - `NUXT_PUBLIC_DEV_PLUGINS`: local base URLs for plugin development.
 *
 * Loading waits for an authenticated session: a plugin's first data call
 * would otherwise 401, and the shared request helper treats a failed refresh
 * as a logout — starting plugins before login would sign the user out.
 */
import { createPluginHost, markdownHooks, editorHooks, pluginUiState, type PluginHost } from '~/services/plugins';
import type { PluginManifest } from '~/types/plugin';

/** One row of `GET /api/plugins`. */
interface RegistryEntry {
  id: string;
  name: string;
  version: string;
  apiVersion: string;
  entry: string;
  capabilities: string[];
  granted: string[];
  manifest: PluginManifest;
  entry_url: string;
}

export default defineNuxtPlugin(async nuxtApp => {
  if (!import.meta.client) return;

  const config = useRuntimeConfig();
  const devPlugins = String(config.public.devPlugins ?? '')
    .split(',')
    .map(url => url.trim())
    .filter(Boolean);

  const notifications = useNotifications();
  const userStore = useUserStore();

  const host: PluginHost = createPluginHost({
    host: {
      name: 'Pergame',
      version: String(config.public.appVersion ?? '0.0.0'),
      client: 'web',
    },
    async request(route, method, body) {
      return await makeRequest(route, method, (body ?? {}) as object);
    },
    notify: (message, opts) =>
      notifications.add({ title: message, type: opts?.type ?? 'info', timeout: opts?.timeout }),
  });

  // Dev-only inspection hook: lets a developer (or a browser check) see what a
  // plugin actually registered without opening the debugger.
  if (import.meta.dev) {
    (window as unknown as Record<string, unknown>).__pergamePluginDebug = {
      host,
      markdownHooks,
      editorHooks,
      ui: pluginUiState,
    };
  }

  // Exposed through `useNuxtApp().$pluginHost` so the settings page can list,
  // enable and disable plugins without a second host instance.
  nuxtApp.provide('pluginHost', host);

  function reportFailure(name: string, error: string | undefined) {
    notifications.add({
      title: `Plugin ${name} failed`,
      message: error,
      type: 'error',
      timeout: 6000,
    });
  }

  async function loadFromRegistry() {
    const response = await makeRequest<RegistryEntry[]>('plugins', 'GET');
    if (response.status !== 'success' || !response.result) return;
    for (const row of response.result) {
      // Nothing granted = the user has not approved this plugin yet.
      if (!row.granted.length) continue;
      const entry = await host.load(row.manifest, row.entry_url, row.granted);
      if (entry.status === 'failed') reportFailure(row.name, entry.error);
    }
  }

  async function loadDevPlugins() {
    for (const base of devPlugins) {
      const entry = await host.loadFromUrl(base);
      if (entry.status === 'failed') reportFailure(entry.manifest.name, entry.error);
    }
  }

  async function loadAll() {
    await loadFromRegistry();
    await loadDevPlugins();
  }

  if (userStore.user) {
    await loadAll();
    return;
  }
  const stop = watch(
    () => userStore.user,
    user => {
      if (!user) return;
      stop();
      void loadAll();
    },
  );
});
