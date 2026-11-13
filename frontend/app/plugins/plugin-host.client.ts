/**
 * Boot the plugin host on the client (PS-A).
 *
 * Dev-mode only: `NUXT_PUBLIC_DEV_PLUGINS` is a comma-separated list of URLs
 * serving `manifest.json` + the entry. The install/registry path (PS-D) will
 * feed the same host from the API instead of this list.
 */
import { createPluginHost, type PluginHost } from '~/services/plugins';

export default defineNuxtPlugin(async nuxtApp => {
  if (!import.meta.client) return;

  const config = useRuntimeConfig();
  const devPlugins = String(config.public.devPlugins ?? '')
    .split(',')
    .map(url => url.trim())
    .filter(Boolean);
  if (!devPlugins.length) return;

  const hostVersion = String(config.public.appVersion ?? '0.0.0');
  const notifications = useNotifications();

  const host: PluginHost = createPluginHost({
    host: { name: 'Pergame', version: hostVersion, client: 'web' },
    async request(route, method, body) {
      return await makeRequest(route, method, (body ?? {}) as object);
    },
    notify: (message, opts) =>
      notifications.add({ title: message, type: opts?.type ?? 'info', timeout: opts?.timeout }),
  });

  // Exposed through `useNuxtApp().$pluginHost` so later phases (settings UI)
  // can list, enable and disable plugins without a second host instance.
  nuxtApp.provide('pluginHost', host);

  for (const base of devPlugins) {
    const entry = await host.loadFromUrl(base);
    if (entry.status === 'failed') {
      notifications.add({
        title: `Plugin ${entry.manifest.name} failed`,
        message: entry.error,
        type: 'error',
        timeout: 6000,
      });
    }
  }
});
