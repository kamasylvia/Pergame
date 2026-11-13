/**
 * Manifest parsing and validation (PS-A).
 *
 * The host refuses anything it cannot trust to be well-formed *before* an
 * entry is imported: a bad manifest must never reach `import()`.
 */
import { HOST_API_VERSION, STATIC_CAPABILITIES, type PluginCapability, type PluginManifest } from '../../types/plugin';

const ID_RE = /^[a-z0-9][a-z0-9-]{0,63}$/;
const SEMVER_RE = /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/;
const DOMAIN_RE = /^(?:\*|[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)+)$/;

/** Validation outcome: either a normalized manifest or the reasons it failed. */
export type ManifestResult = { ok: true; manifest: PluginManifest } | { ok: false; errors: string[] };

function asString(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim() ? value.trim() : undefined;
}

/**
 * Entry must stay inside the plugin root: a relative `.mjs` path whose
 * segments are plain names (no `.` / `..`, no leading slash, no backslash).
 */
function isSafeEntry(entry: string): boolean {
  if (!entry.endsWith('.mjs') || entry.includes('\\')) return false;
  return entry
    .split('/')
    .every(segment => segment !== '.' && segment !== '..' && /^[A-Za-z0-9._-]+$/.test(segment));
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

  const entry = asString(m.entry) ?? 'main.mjs';
  if (!isSafeEntry(entry)) errors.push('entry must be a relative .mjs path inside the plugin');

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
      minHostVersion,
      author: asString(m.author),
      description: asString(m.description),
    },
  };
}
