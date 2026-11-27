/**
 * Manifest parsing and validation (PS-A).
 *
 * The host refuses anything it cannot trust to be well-formed *before* an
 * entry is imported: a bad manifest must never reach `import()`.
 */
import { HOST_API_VERSION, STATIC_CAPABILITIES, type PluginCapability, type PluginCodeTheme, type PluginManifest, type PluginTheme } from '../../types/plugin';

const ID_RE = /^[a-z0-9][a-z0-9-]{0,63}$/;
const SEMVER_RE = /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/;
const DOMAIN_RE = /^(?:\*|[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)+)$/;

/** Validation outcome: either a normalized manifest or the reasons it failed. */
export type ManifestResult = { ok: true; manifest: PluginManifest } | { ok: false; errors: string[] };

function asString(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim() ? value.trim() : undefined;
}

/**
 * Paths must stay inside the plugin root: a relative path with the given
 * extension whose segments are plain names (no `.` / `..`, no leading
 * slash, no backslash).
 */
function isSafeAsset(path: string, ext: '.mjs' | '.css'): boolean {
  if (!path.endsWith(ext) || path.includes('\\')) return false;
  return path
    .split('/')
    .every(segment => segment !== '.' && segment !== '..' && /^[A-Za-z0-9._-]+$/.test(segment));
}

/** The 18 `--code-*` slots a palette must fill (mirrors code-hljs.scss). */
export const CODE_VAR_NAMES = [
  'bg', 'fg', 'comment', 'keyword', 'type', 'literal', 'attr', 'number', 'string', 'function',
  'class', 'variable', 'operator', 'section', 'addition', 'deletion', 'meta', 'tag',
] as const;

/**
 * A CSS var value that cannot break out of its declaration (mirrors the
 * server's is_safe_css_value): short, colors/functions only.
 */
export function isSafeCssValue(value: string): boolean {
  if (!value || value.length > 64) return false;
  return /^[a-zA-Z0-9#(),.%/*+ _/-]+$/.test(value);
}

/**
 * A capability is either one of the closed set or a `net:` scope
 * (`net:*` = any domain, warned at install time; a bare domain otherwise).
 */
export function isKnownCapability(cap: string): boolean {
  if ((STATIC_CAPABILITIES as readonly string[]).includes(cap)) return true;
  if (!cap.startsWith('net:')) return false;
  const scope = cap.slice(4);
  return DOMAIN_RE.test(scope);
}

/** True when `url` is allowed by any `net:` capability in `capabilities`. */
export function netAllows(capabilities: readonly PluginCapability[], url: URL): boolean {
  if (url.protocol !== 'https:' && url.protocol !== 'http:') return false;
  const host = url.hostname.toLowerCase();
  for (const cap of capabilities) {
    if (!cap.startsWith('net:')) continue;
    const scope = cap.slice(4);
    if (scope === '*') return true;
    if (host === scope || host.endsWith(`.${scope}`)) return true;
  }
  return false;
}

/** Compare dotted versions; `null` when either side is unparseable. */
export function compareVersions(a: string, b: string): number | null {
  const pa = a.split('.').map(n => Number.parseInt(n, 10));
  const pb = b.split('.').map(n => Number.parseInt(n, 10));
  if (pa.some(Number.isNaN) || pb.some(Number.isNaN)) return null;
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const diff = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (diff !== 0) return diff;
  }
  return 0;
}

/**
 * Validate a raw `manifest.json` object.
 *
 * `hostVersion` enables the optional `minHostVersion` floor. Validation is
 * total: every problem is reported, so the install UI can show all of them.
 */
export function validateManifest(raw: unknown, hostVersion?: string): ManifestResult {
  const errors: string[] = [];
  if (typeof raw !== 'object' || raw === null) {
    return { ok: false, errors: ['manifest must be a JSON object'] };
  }
  const m = raw as Record<string, unknown>;

  const id = asString(m.id);
  if (!id) errors.push('id is required');
  else if (!ID_RE.test(id)) errors.push('id must be a lowercase slug (a-z, 0-9, dashes, max 64)');

  const name = asString(m.name);
  if (!name) errors.push('name is required');

  const version = asString(m.version);
  if (!version) errors.push('version is required');
  else if (!SEMVER_RE.test(version)) errors.push('version must be semver (x.y.z)');

  const apiVersion = asString(m.apiVersion);
  if (!apiVersion) errors.push('apiVersion is required');
  else if (Number.parseInt(apiVersion, 10) !== HOST_API_VERSION) {
    errors.push(`apiVersion ${apiVersion} is not supported (host contract is ${HOST_API_VERSION})`);
  }

  const themesRaw = m.themes;
  const themes: PluginTheme[] = [];
  if (themesRaw !== undefined && !Array.isArray(themesRaw)) {
    errors.push('themes must be an array');
  } else if (Array.isArray(themesRaw)) {
    themesRaw.forEach((item, i) => {
      const tag = `themes[${i}]`;
      if (typeof item !== 'object' || item === null) {
        errors.push(`${tag} must be an object`);
        return;
      }
      const t = item as Record<string, unknown>;
      const tid = asString(t.id);
      if (!tid) {
        errors.push(`${tag}.id is required`);
        return;
      } else if (!ID_RE.test(tid)) {
        errors.push(`${tag}.id must be a lowercase slug`);
        return;
      } else if (themes.some(x => x.id === tid)) {
        errors.push(`${tag}.id '${tid}' is duplicated`);
        return;
      }
      const tname = asString(t.name);
      if (!tname) {
        errors.push(`${tag}.name is required`);
        return;
      }
      const file = asString(t.file);
      if (!file) {
        errors.push(`${tag}.file is required`);
        return;
      } else if (!isSafeAsset(file, '.css')) {
        errors.push(`${tag}.file must be a relative .css path inside the plugin`);
        return;
      }
      const darkFile = asString(t.darkFile);
      if (darkFile && !isSafeAsset(darkFile, '.css')) {
        errors.push(`${tag}.darkFile must be a relative .css path inside the plugin`);
        return;
      }
      themes.push({ id: tid, name: tname, file, ...(darkFile ? { darkFile } : {}) });
    });
  }

  const codeThemesRaw = m.codeThemes;
  const codeThemes: PluginCodeTheme[] = [];
  if (codeThemesRaw !== undefined && !Array.isArray(codeThemesRaw)) {
    errors.push('codeThemes must be an array');
  } else if (Array.isArray(codeThemesRaw)) {
    codeThemesRaw.forEach((item, i) => {
      const tag = `codeThemes[${i}]`;
      if (typeof item !== 'object' || item === null) {
        errors.push(`${tag} must be an object`);
        return;
      }
      const t = item as Record<string, unknown>;
      const tid = asString(t.id);
      if (!tid) {
        errors.push(`${tag}.id is required`);
        return;
      } else if (!ID_RE.test(tid)) {
        errors.push(`${tag}.id must be a lowercase slug`);
        return;
      } else if (codeThemes.some(x => x.id === tid)) {
        errors.push(`${tag}.id '${tid}' is duplicated`);
        return;
      }
      const tname = asString(t.name);
      if (!tname) {
        errors.push(`${tag}.name is required`);
        return;
      }
      const varsRaw = t.vars;
      if (typeof varsRaw !== 'object' || varsRaw === null || Array.isArray(varsRaw)) {
        errors.push(`${tag}.vars must be an object`);
        return;
      }
      const varsMap = varsRaw as Record<string, unknown>;
      const vars: Record<string, string> = {};
      let varsOk = true;
      for (const name of CODE_VAR_NAMES) {
        const v = typeof varsMap[name] === 'string' ? (varsMap[name] as string).trim() : '';
        if (!v) {
          errors.push(`${tag}.vars.${name} is required`);
          varsOk = false;
        } else if (!isSafeCssValue(v)) {
          errors.push(`${tag}.vars.${name} is not a safe CSS value`);
          varsOk = false;
        } else {
          vars[name] = v;
        }
      }
      for (const key of Object.keys(varsMap)) {
        if (!(CODE_VAR_NAMES as readonly string[]).includes(key)) {
          errors.push(`${tag}.vars.${key} is not a known code var`);
          varsOk = false;
        }
      }
      if (!varsOk) return;
      codeThemes.push({ id: tid, name: tname, vars });
    });
  }

  // Themes-only plugins ship no JS: entry may be omitted only then.
  const entryRaw = asString(m.entry);
  const entry = entryRaw ?? (themes.length || codeThemes.length ? undefined : 'main.mjs');
  if (entry !== undefined && !isSafeAsset(entry, '.mjs')) errors.push('entry must be a relative .mjs path inside the plugin');

  const capabilitiesRaw = m.capabilities;
  if (capabilitiesRaw !== undefined && !Array.isArray(capabilitiesRaw)) {
    errors.push('capabilities must be an array');
  }
  const capabilities = Array.isArray(capabilitiesRaw)
    ? capabilitiesRaw.filter((c): c is string => typeof c === 'string').map(c => c.trim()).filter(Boolean)
    : [];
  for (const cap of capabilities) {
    if (!isKnownCapability(cap)) errors.push(`unknown capability '${cap}'`);
  }

  const minHostVersion = asString(m.minHostVersion);
  if (minHostVersion) {
    if (!SEMVER_RE.test(minHostVersion)) errors.push('minHostVersion must be semver (x.y.z)');
    else if (hostVersion && (compareVersions(hostVersion, minHostVersion) ?? 0) < 0) {
      // Deliberately not an error: reported separately so the settings list
      // can say "needs a newer host" without losing the manifest.
    }
  }

  if (errors.length) return { ok: false, errors };

  return {
    ok: true,
    manifest: {
      id: id!,
      name: name!,
      version: version!,
      apiVersion: apiVersion!,
      entry,
      capabilities: [...new Set(capabilities)],
      themes: themes.length ? themes : undefined,
      codeThemes: codeThemes.length ? codeThemes : undefined,
      minHostVersion,
      author: asString(m.author),
      description: asString(m.description),
      icon: asString(m.icon),
      settings: Array.isArray(m.settings) ? (m.settings as PluginSettingField[]) : undefined,
    },
  };
}
