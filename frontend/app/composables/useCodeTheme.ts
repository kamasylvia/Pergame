/**
 * Code palette registry (built-ins + plugin-contributed).
 *
 * Built-ins render from the static code-hljs.scss; plugin palettes are
 * manifest-inline vars, emitted here as `html[data-code-theme="plugin:id"]`
 * rules in a dedicated <style>. Values passed manifest validation, but the
 * sheet generator re-checks the charset so a bad registry entry can never
 * inject rules.
 */
import { CODE_VAR_NAMES, isSafeCssValue } from '~/services/plugins/manifest';

export interface CodePaletteSource {
  id: string;
  name: string;
  /** 'builtin' or the contributing plugin id. */
  origin: string;
  /** Human plugin name for the settings list (empty for built-ins). */
  originName?: string;
  /** Manifest-inline vars; absent for built-ins (static scss). */
  vars?: Record<string, string>;
}

const SHEET_ID = 'code-theme-plugins';

/** Built-ins; plugin palettes register into the same list. */
export const codeThemeRegistry = ref<CodePaletteSource[]>([
  { id: 'palenight', name: 'Palenight', origin: 'builtin' },
  { id: 'dracula', name: 'Dracula', origin: 'builtin' },
  { id: 'github-light', name: 'GitHub Light', origin: 'builtin' },
  { id: 'solarized-light', name: 'Solarized Light', origin: 'builtin' },
]);

export function registerCodePalette(palette: CodePaletteSource): void {
  const i = codeThemeRegistry.value.findIndex(t => t.id === palette.id);
  if (i >= 0) codeThemeRegistry.value[i] = palette;
  else codeThemeRegistry.value.push(palette);
  renderPluginSheet();
}

export function unregisterCodePalette(id: string): void {
  codeThemeRegistry.value = codeThemeRegistry.value.filter(t => t.id !== id);
  renderPluginSheet();
}

function renderPluginSheet(): void {
  if (!import.meta.client) return;
  const blocks: string[] = [];
  for (const palette of codeThemeRegistry.value) {
    if (!palette.vars) continue;
    const decls: string[] = [];
    for (const name of CODE_VAR_NAMES) {
      const value = palette.vars[name];
      if (typeof value !== 'string' || !isSafeCssValue(value)) continue;
      decls.push(`--code-${name}: ${value};`);
    }
    // Partial palettes never apply: all-or-nothing keeps rendering predictable.
    if (decls.length !== CODE_VAR_NAMES.length) continue;
    blocks.push(`html[data-code-theme="${palette.id}"] {\n${decls.join('\n')}\n}`);
  }
  let el = document.getElementById(SHEET_ID) as HTMLStyleElement | null;
  if (!blocks.length) {
    el?.remove();
    return;
  }
  if (!el) {
    el = document.createElement('style');
    el.id = SHEET_ID;
    document.head.appendChild(el);
  }
  el.textContent = blocks.join('\n');
}

const FALLBACK_LIGHT = 'github-light';
const FALLBACK_DARK = 'palenight';

/**
 * Effective palette id for the current mode, with registry fallback.
 * Installs the client applier (data-code-theme attr) and the missing-id
 * reset (uninstalling a palette plugin restores the mode default).
 * Call once in App.vue; the returned computed also feeds SSR htmlAttrs.
 */
export function useCodeTheme(): ComputedRef<string> {
  const preferences = usePreferencesStore();
  const light = preferences.get('codeThemeLight');
  const dark = preferences.get('codeThemeDark');
  const colorMode = useColorMode();

  const effective = computed(() => {
    const id = colorMode.value === 'dark' ? dark.value : light.value;
    const fallback = colorMode.value === 'dark' ? FALLBACK_DARK : FALLBACK_LIGHT;
    return codeThemeRegistry.value.some(t => t.id === id) ? id : fallback;
  });

  watchEffect(() => {
    if (import.meta.client) document.documentElement.dataset.codeTheme = effective.value;
  });
  watch(codeThemeRegistry, () => {
    if (!codeThemeRegistry.value.some(t => t.id === light.value)) preferences.set('codeThemeLight', FALLBACK_LIGHT);
    if (!codeThemeRegistry.value.some(t => t.id === dark.value)) preferences.set('codeThemeDark', FALLBACK_DARK);
  });

  return effective;
}
