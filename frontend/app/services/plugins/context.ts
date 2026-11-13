/**
 * The context handed to `plugin.onload(ctx)` (PS-A: L1 data + notice + net).
 *
 * Everything here is built on injected dependencies (`request`, `notify`) so
 * the kernel stays free of Nuxt imports and can run under a test harness.
 * Permissions are the logged-in user's: the clients call the same REST
 * endpoints the UI does, so a plugin can never see more than its owner.
 */
import { netAllows } from './manifest';
import type {
  KVStore,
  NetClient,
  NodeClient,
  PluginContext,
  PluginManifest,
  PluginNode,
  ResourceClient,
  SearchClient,
  UiClient,
} from '../../types/plugin';

/** Wire result of one API call (the app's `makeRequest` shape). */
export interface PluginRequestResult<T> {
  status: 'success' | 'error';
  result?: T;
  message?: string;
}

/** Injected API transport (bound to `makeRequest` by the Nuxt boot plugin). */
export type PluginRequest = <T>(
  route: string,
  method: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE',
  body?: object,
) => Promise<PluginRequestResult<T>>;

/** Host identity, shown to plugins and in the settings list. */
export interface PluginHostInfo {
  name: string;
  version: string;
  client: string;
}

/** Dependencies the kernel injects into every context. */
export interface PluginContextDeps {
  manifest: PluginManifest;
  version: number;
  host: PluginHostInfo;
  request: PluginRequest;
  notify: (message: string, opts?: Parameters<UiClient['notice']>[1]) => void;
}

const NET_ALLOWED_METHODS: Record<string, true> = { GET: true, POST: true };
const NET_MAX_RESPONSE_BYTES = 8 * 1024 * 1024;

/** Throw when a call failed, keeping the server's message. */
function unwrap<T>(result: PluginRequestResult<T>, fallback: string): T {
  if (result.status !== 'success') throw new Error(result.message || fallback);
  return result.result as T;
}

/** `/nodes/:id` returns `{node, permissions}`; plugins get the node only. */
function nodeFrom(result: PluginRequestResult<{ node: PluginNode }>, fallback: string): PluginNode {
  return unwrap(result, fallback).node;
}

/**
 * In-memory settings for PS-A. PS-D replaces this with a per-plugin
 * namespace in `user_settings`; the interface stays identical.
 */
export function createMemoryKV(initial: Record<string, unknown> = {}): KVStore {
  const data = new Map(Object.entries(initial));
  return {
    get: async <T>(key: string) => data.get(key) as T | undefined,
    set: async (key, value) => {
      data.set(key, value);
    },
    delete: async key => {
      data.delete(key);
    },
  };
}

/** Network facade: capability allowlist, method and size limits (spec §9.4). */
export function createNetClient(capabilities: readonly string[]): NetClient {
  return {
    async fetch(url, init) {
      const target = new URL(String(url));
      if (!netAllows(capabilities, target)) {
        throw new Error(`net: ${target.hostname} is not allowed by this plugin's capabilities`);
      }
      const method = (init?.method ?? 'GET').toUpperCase();
      if (!NET_ALLOWED_METHODS[method]) {
        throw new Error(`net: ${method} is not allowed (GET and POST only)`);
      }
      const response = await globalThis.fetch(target, init);
      // Read through a capped buffer so a plugin cannot pull an unbounded
      // body into the tab; the caller still gets a normal Response.
      const declared = Number(response.headers.get('content-length') ?? '0');
      if (declared > NET_MAX_RESPONSE_BYTES) {
        throw new Error(`net: response too large (${declared} bytes)`);
      }
      const bytes = await response.arrayBuffer();
      if (bytes.byteLength > NET_MAX_RESPONSE_BYTES) {
        throw new Error(`net: response too large (${bytes.byteLength} bytes)`);
      }
      return new Response(bytes, {
        status: response.status,
        statusText: response.statusText,
        headers: response.headers,
      });
    },
  };
}

/** Build the context object for one loaded plugin. */
export function createPluginContext(deps: PluginContextDeps): PluginContext {
  const { request } = deps;

  const nodes: NodeClient = {
    get: async id => nodeFrom(await request(`nodes/${id}`, 'GET'), 'node not found'),
    async listChildren(parentId) {
      const rows = unwrap(await request<PluginNode[]>('nodes/user/@me', 'GET'), 'cannot list nodes');
      return parentId ? rows.filter(n => n.parent_id === parentId) : rows;
    },
    async create(input) {
      const created = unwrap(
        await request<PluginNode | { node: PluginNode }>('nodes', 'POST', input),
        'cannot create node',
      );
      // Folder creation answers `{node, index}` (issue #4).
      return 'node' in created ? created.node : created;
    },
    update: async (id, patch) =>
      unwrap(await request<PluginNode>(`nodes/${id}`, 'PUT', { ...patch }), 'cannot update node'),
    remove: async id => {
      const result = await request(`nodes/${id}`, 'DELETE');
      if (result.status !== 'success') throw new Error(result.message || 'cannot delete node');
    },
  };

  const resources: ResourceClient = {
    async upload(file, parentId) {
      const body = new FormData();
      body.append('file', file, file instanceof File ? file.name : 'upload.bin');
      if (parentId) body.append('parent_id', parentId);
      return unwrap(await request<PluginNode>('resources', 'POST', body), 'cannot upload file');
    },
  };

  const search: SearchClient = {
    async fulltext(query, opts) {
      const params = new URLSearchParams({
        q: query,
        content: String(opts?.content ?? true),
        limit: String(opts?.limit ?? 20),
      });
      return unwrap(await request<PluginNode[]>(`nodes/search?${params}`, 'GET'), 'search failed');
    },
    async query(dql) {
      const result = unwrap(
        await request<{ rows: PluginNode[] }>('nodes/query', 'POST', { query: dql }),
        'query failed',
      );
      return result.rows;
    },
  };

  return {
    api: { version: deps.version, host: deps.host },
    manifest: deps.manifest,
    nodes,
    resources,
    search,
    settings: createMemoryKV(),
    ui: { notice: (message, opts) => deps.notify(message, opts) },
    net: createNetClient(deps.manifest.capabilities),
  };
}
