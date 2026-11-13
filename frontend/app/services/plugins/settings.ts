/**
 * Server-backed plugin settings (PS-D).
 *
 * `ctx.settings` used to be an in-memory map; it now reads and writes the
 * caller's own namespace (`/api/plugins/<id>/kv`). Every call is async, so a
 * plugin awaiting a read gets the stored value rather than a stale cache.
 */
import type { KVStore } from '../../types/plugin';
import type { PluginRequest } from './context';

/** Per-plugin KV backed by the API. */
export function createRemoteKV(pluginId: string, request: PluginRequest): KVStore {
  const base = `plugins/${pluginId}/kv`;
  return {
    async get<T>(key: string): Promise<T | undefined> {
      const response = await request<Record<string, unknown>>(base, 'GET');
      if (response.status !== 'success' || !response.result) return undefined;
      return response.result[key] as T | undefined;
    },
    async set(key: string, value: unknown): Promise<void> {
      const response = await request(`${base}/${encodeURIComponent(key)}`, 'PUT', value as object);
      if (response.status !== 'success') {
        throw new Error(response.message || 'cannot save plugin setting');
      }
    },
    async delete(key: string): Promise<void> {
      const response = await request(`${base}/${encodeURIComponent(key)}`, 'DELETE');
      if (response.status !== 'success') {
        throw new Error(response.message || 'cannot delete plugin setting');
      }
    },
  };
}
