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
import { createPluginHost, markdownHooks, editorHooks, pluginUiState, pluginModalState, type PluginHost } from '~/services/plugins';
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
  // i18n composable (useI18nT) asserts a setup context and throws inside an
  // async boot plugin — take the app-level instance instead.
  const i18n = nuxtApp.$i18n as
    | { t: (key: string, params?: Record<string, unknown>) => string; locale: string }
    | undefined;

  /** Render-stage wording in the UI language (backend sends stage keys only). */
  const formatClipProgress = (percent: number, stage: string) => {
    const stageKeys: Record<string, string> = {
      queued: 'stageQueued',
      connecting: 'stageConnecting',
      rendering: 'stageRendering',
      extracted: 'stageExtracted',
    };
    const stageKey = stageKeys[stage];
    const stageLabel = stageKey ? i18n?.t(`settings.plugins.clip.${stageKey}`) ?? stage : stage;
    return i18n ? i18n.t('settings.plugins.clip.progress', { percent, stage: stageLabel }) : `${stageLabel} ${percent}%`;
  };

  const progressToasts = new Map<string, number>();
  const host: PluginHost = createPluginHost({
    host: {
      name: 'Pergame',
      version: String(config.public.appVersion ?? '0.0.0'),
      client: 'web',
      locale: String(i18n?.locale ?? 'en'),
    },
    async request(route, method, body) {
      return await makeRequest(route, method, (body ?? {}) as object);
    },
    formatClipProgress,
    notify: (message, opts) => {
      const payload = { title: message, type: opts?.type ?? 'info', timeout: opts?.timeout };
      if (opts?.id) {
        // Progress toasts: the plugin reuses one id, the host updates that
        // toast in place instead of stacking one per stage.
        const numeric = progressToasts.get(opts.id);
        if (numeric !== undefined) {
          notifications.update(numeric, payload, opts.timeout);
          return;
        }
        progressToasts.set(opts.id, notifications.add(payload));
        return;
      }
      notifications.add(payload);
    },
    async onNodesChanged() {
      // Plugin node writes bypass the pinia stores (plain REST); reload the
      // held directory pages so the tree shows them without a full reload.
      await useNodesStore().refreshLoadedDirs();
    },
  });

  // Dev-only inspection hook: lets a developer (or a browser check) see what a
  // plugin actually registered without opening the debugger.
  if (import.meta.dev) {
    (window as unknown as Record<string, unknown>).__pergamePluginDebug = {
      host,
      markdownHooks,
      editorHooks,
      ui: pluginUiState,
      modal: pluginModalState,
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
    console.log('[plugin-host] registry rows:', response.result.map(r => `${r.id}@${r.version} granted=${r.granted.length}`));
    for (const row of response.result) {
      // Nothing granted = the user has not approved this plugin yet.
      if (!row.granted.length) continue;
      const entry = await host.load(row.manifest, row.entry_url, row.granted, `/api/plugins/${row.id}/asset`);
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
    console.log('[plugin-host] loadAll started, user =', userStore.user?.username ?? null);
    await loadFromRegistry();
    await loadDevPlugins();
    console.log('[plugin-host] loadAll finished');
  }

  if (userStore.user) {
    console.log('[plugin-host] user present at init, loading immediately');
    await loadAll();
    return;
  }
  console.log('[plugin-host] user not ready yet, watching');
  const stop = watch(
    () => userStore.user,
    user => {
      if (!user) return;
      stop();
      void loadAll();
    },
  );
});
