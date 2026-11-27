/**
 * App chrome themes (real theme management, not the document-only `theme` pref).
 *
 * A theme = light + dark declaration blocks applied as:
 *   html[data-app-theme="<id>"] { <light> }
 *   html.dark-mode[data-app-theme="<id>"] { <dark> }
 * in a dedicated unlayered <style> (layered sheets lose to the unlayered app
 * defaults, so @layer is deliberately NOT used here, unlike user CSS).
 *
 * `--primary*` needs special handling: useAppColors writes it as an inline
 * style, which beats any stylesheet. Declarations the theme provides for it
 * are extracted and applied inline too; switching back to a theme without
 * them restores the user's accent via setAppColor.
 */
import forestLight from '~/themes/forest-light.css?raw';
import forestDark from '~/themes/forest-dark.css?raw';
import oceanLight from '~/themes/ocean-light.css?raw';
import oceanDark from '~/themes/ocean-dark.css?raw';
import roseLight from '~/themes/rose-light.css?raw';
import roseDark from '~/themes/rose-dark.css?raw';
import slateLight from '~/themes/slate-light.css?raw';
import slateDark from '~/themes/slate-dark.css?raw';

export interface AppThemeSource {
  id: string;
  name: string;
  /** 'builtin' or the contributing plugin id. */
  origin: string;
  /** Human plugin name for the settings list (empty for built-ins). */
  originName?: string;
  light: string;
  dark: string;
}

const SHEET_ID = 'app-theme';
const PRIMARY_RE = /--primary(-dark|-bg|-border)?\s*:\s*[^;]+;/g;

/** Built-ins; plugin themes register into the same list (batch 4). */
export const appThemeRegistry = ref<AppThemeSource[]>([
  { id: 'default', name: 'Default', origin: 'builtin', light: '', dark: '' },
  { id: 'forest', name: 'Forest', origin: 'builtin', light: forestLight, dark: forestDark },
  { id: 'ocean', name: 'Ocean', origin: 'builtin', light: oceanLight, dark: oceanDark },
  { id: 'rose', name: 'Rose', origin: 'builtin', light: roseLight, dark: roseDark },
  { id: 'slate', name: 'Slate', origin: 'builtin', light: slateLight, dark: slateDark },
]);

export function registerAppTheme(theme: AppThemeSource): void {
  const i = appThemeRegistry.value.findIndex(t => t.id === theme.id);
  if (i >= 0) appThemeRegistry.value[i] = theme;
  else appThemeRegistry.value.push(theme);
}

export function unregisterAppTheme(id: string): void {
  appThemeRegistry.value = appThemeRegistry.value.filter(t => t.id !== id);
}

/**
 * Strip what a theme must never do: pull remote stylesheets or beacon
 * through remote urls. data: urls survive (fonts, patterns).
 */
export function sanitizeThemeCss(css: string): string {
  return css
    .replace(/@import[^;]+;/gi, '')
    .replace(/url\(\s*['"]?https?:[^)]+\)/gi, 'none');
}

function splitPrimary(css: string): { primary: Array<[string, string]>; rest: string } {
  const primary: Array<[string, string]> = [];
  const rest = css.replace(PRIMARY_RE, match => {
    const colon = match.indexOf(':');
    primary.push([match.slice(0, colon).trim(), match.slice(colon + 1).replace(';', '').trim()]);
    return '';
  });
  return { primary, rest };
}

export function applyAppTheme(id: string): void {
  if (!import.meta.client) return;
  const root = document.documentElement;
  const fallback: AppThemeSource = { id: 'default', name: 'Default', origin: 'builtin', light: '', dark: '' };
  const theme = appThemeRegistry.value.find(t => t.id === id) ?? fallback;
  root.dataset.appTheme = theme.id;

  const lightSplit = splitPrimary(sanitizeThemeCss(theme.light));
  const darkSplit = splitPrimary(sanitizeThemeCss(theme.dark));

  let el = document.getElementById(SHEET_ID) as HTMLStyleElement | null;
  if (!lightSplit.rest.trim() && !darkSplit.rest.trim()) {
    el?.remove();
  } else {
    if (!el) {
      el = document.createElement('style');
      el.id = SHEET_ID;
      document.head.appendChild(el);
    }
    el.textContent =
      `html[data-app-theme="${theme.id}"] {\n${lightSplit.rest}\n}\n` +
      `html.dark-mode[data-app-theme="${theme.id}"] {\n${darkSplit.rest}\n}\n`;
  }

  // Inline --primary* beats stylesheets; mirror setAppColor's mechanism.
  // Only the active mode's declarations apply (inline cannot be conditional).
  const { setAppColor } = useAppColors();
  const preferences = usePreferencesStore();
  const isDark = useColorMode().value === 'dark';
  const defined = isDark ? darkSplit.primary : lightSplit.primary;
  if (defined.length) {
    for (const [name, value] of defined) root.style.setProperty(name, value);
  } else {
    setAppColor(Number(preferences.get('primaryColor').value));
  }
}

export function useAppTheme(): void {
  const preferences = usePreferencesStore();
  const appTheme = preferences.get('appTheme');
  const primaryColor = preferences.get('primaryColor');
  const colorMode = useColorMode();

  watch([appTheme, primaryColor, () => colorMode.value], () => applyAppTheme(appTheme.value), { immediate: true });
  // Registry changes (plugin install/uninstall): re-resolve, fall back
  // to Default when the selected theme disappears.
  watch(appThemeRegistry, () => {
    if (!appThemeRegistry.value.some(t => t.id === appTheme.value)) {
      preferences.set('appTheme', 'default');
    }
    applyAppTheme(appTheme.value);
  });
}
