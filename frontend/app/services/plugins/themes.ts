/**
 * Manifest-declared app themes (no-JS theme packs).
 *
 * A plugin with the `themes` capability lists `{id, name, file, darkFile?}`
 * in its manifest; the host fetches the declaration files once at load and
 * registers them into the app-theme registry as `<plugin>:<theme>`.
 * Unload unregisters (the registry watcher falls back to Default).
 */
import { registerAppTheme, unregisterAppTheme } from '~/composables/useAppTheme';
import type { PluginManifest } from '../../types/plugin';

/** Refuse absurd theme files (install already caps at 2MB per file). */
const MAX_THEME_BYTES = 256 * 1024;

async function fetchThemeCss(url: string, fetchText: (url: string) => Promise<string>): Promise<string> {
  // Same-origin asset route needs the session cookie; cross-origin dev
  // servers must not get credentials attached.
  const sameOrigin = new URL(url, globalThis.location.href).origin === globalThis.location.origin;
  const text = sameOrigin
    ? await globalThis.fetch(url, { credentials: 'include' }).then(async response => {
        if (!response.ok) throw new Error(`theme ${response.status} for ${url}`);
        return response.text();
      })
    : await fetchText(url);
  if (text.length > MAX_THEME_BYTES) throw new Error(`theme ${url} exceeds ${MAX_THEME_BYTES} bytes`);
  return text;
}

export async function loadPluginThemes(
  manifest: PluginManifest,
  assetBase: string,
  fetchText: (url: string) => Promise<string>,
): Promise<void> {
  if (!manifest.themes?.length) return;
  const base = assetBase.replace(/\/+$/, '');
  for (const theme of manifest.themes) {
    const qualified = `${manifest.id}:${theme.id}`;
    try {
      const light = await fetchThemeCss(`${base}/${theme.file}`, fetchText);
      const dark = theme.darkFile ? await fetchThemeCss(`${base}/${theme.darkFile}`, fetchText) : '';
      registerAppTheme({
        id: qualified,
        name: theme.name,
        origin: manifest.id,
        originName: manifest.name,
        light,
        dark,
      });
    } catch (err) {
      // A broken theme file must not fail the plugin (or a sibling theme).
      console.warn(`[plugins] ${qualified} theme skipped:`, err instanceof Error ? err.message : err);
    }
  }
}

export function unloadPluginThemes(manifest: PluginManifest): void {
  for (const theme of manifest.themes ?? []) unregisterAppTheme(`${manifest.id}:${theme.id}`);
}
