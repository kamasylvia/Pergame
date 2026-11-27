/**
 * Manifest-declared themes (no-JS theme packs).
 *
 * A plugin with the `themes` capability lists app themes (`{id, name, file,
 * darkFile?}` — CSS fetched once at load) and/or code palettes (`{id, name,
 * vars}` — manifest-inline) in its manifest; the host registers them as
 * `<plugin>:<theme>`. Unload unregisters (registry watchers fall back).
 */
import { registerAppTheme, unregisterAppTheme } from '~/composables/useAppTheme';
import { registerCodePalette, unregisterCodePalette } from '~/composables/useCodeTheme';
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

export function loadPluginCodeThemes(manifest: PluginManifest): void {
  for (const palette of manifest.codeThemes ?? []) {
    registerCodePalette({
      id: `${manifest.id}:${palette.id}`,
      name: palette.name,
      origin: manifest.id,
      originName: manifest.name,
      vars: { ...palette.vars },
    });
  }
}

export function unloadPluginCodeThemes(manifest: PluginManifest): void {
  for (const palette of manifest.codeThemes ?? []) unregisterCodePalette(`${manifest.id}:${palette.id}`);
}
