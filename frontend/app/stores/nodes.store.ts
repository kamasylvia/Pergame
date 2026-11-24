import { IndexedCollection } from '~/helpers/IndexedCollection';
import { parseTags, mergeNode, resolveIcon, resolveNodeLink } from '~/helpers/node';
import type { TreeItem } from '~/helpers/TreeBuilder';
import { LocalDbService } from '~/services/localDB';
import { skipHydrate } from 'pinia';
import type { DB_Node, DB_NodeLink, Node, NodeSearchResult, Permission, TrashEntry } from './db_structures';
import type { KanbanMetadata } from '~/components/Kanban/Board.vue';

export type TeamRole = 0;
export type CategoryRole = 1 | 2;
export type DocumentRole = 3;
export type ResourceRole = 4;
export type NodeRole = TeamRole | CategoryRole | DocumentRole | ResourceRole;

export interface SearchOptions {
  query?: string;
  fromDate?: Date;
  toDate?: Date;
  dateType?: 'created' | 'modified';
  tags?: string[];
  category?: string;
  matchMode?: 'includes' | 'starts' | 'exact';
  role?: NodeRole;
  kanbanAssignStatus?: 'assigned' | 'unassigned' | 'all';
  boardId?: string;
  assignedUserId?: string;
}

export interface FlatParams {
  role?: number;
  root?: string;
  pinned?: boolean;
  offset?: number;
  limit?: number;
  sortKey?: string;
  sortDir?: string;
}

export const useNodesStore = defineStore('nodes', () => {
  const userStore = useUserStore();
  const { sortNodes } = useNodesSort();

  const nodes = shallowRef(new IndexedCollection());
  const allTags = ref<string[]>([]);
  const isFetching = ref(false);
  /** Soft-link positions (note-links spec): same note, extra tree locations. */
  const links = ref<DB_NodeLink[]>([]);

  // ---- Lazy directory loading (SiYuan listDocTreeRange parity) ----
  // One load unit per directory: which 100-row pages are merged. Roots use
  // a null key. `hasChildren` feeds chevrons without fetching (kept outside
  // the rows so single-node refetches never drop it).
  interface DirState {
    total: number;
    pages: Set<number>;
    loading: boolean;
  }
  const PAGE_SIZE = 100;
  const dirStates = ref(new Map<string | null, DirState>());
  const hasChildren = ref(new Map<string, boolean>());

  const getAll = computed(() => nodes.value);
  const getAllTags = computed(() => allTags.value);
  const getById = computed(() => (id: string) => nodes.value.get(id));
  const getTotalUsedStorage = computed(() => nodes.value.toSortedArray().reduce((total, node) => total + (node.size || 0), 0));

  /* Getters for nodes by role */
  const teams = computed(() => nodes.value.getIdsByRole(0).map(id => nodes.value.get(id)!));
  const categories = computed(() => [...nodes.value.getIdsByRole(1), ...nodes.value.getIdsByRole(2)].map(id => nodes.value.get(id)!));
  const workspaces = computed(() => nodes.value.getIdsByRole(1).map(id => nodes.value.get(id)!));
  const documents = computed(() => nodes.value.getIdsByRole(3).map(id => nodes.value.get(id)!));
  const resources = computed(() => nodes.value.getIdsByRole(4).map(id => nodes.value.get(id)!));
  /** Clone positions grouped by the folder they appear under (tree synthesis). */
  const linksByParent = computed(() => {
    const map = new Map<string, DB_NodeLink[]>();
    for (const l of links.value) {
      const arr = map.get(l.parent_id) ?? [];
      arr.push(l);
      map.set(l.parent_id, arr);
    }
    return map;
  });

  async function init() {
    console.debug('[store/nodes] Initializing store');
    clear();

    isFetching.value = true;
    nodes.value.startBulk();
    await Promise.all([syncLocalDeltas(), loadDirPage(null, 0), fetchShared(), fetchLinks(), fetchTags()]);
    nodes.value.endBulk();
    isFetching.value = false;
    if (import.meta.client) {
      window.addEventListener('online', () => {
        console.debug('[store/nodes] Online, syncing local deltas');
        syncLocalDeltas();
      });
    }
  }

  async function syncLocalDeltas() {
    const deltas = await LocalDbService.getDeltas();
    await LocalDbService.clearDeltas(); // Clear deltas before syncing to avoid duplicates in case of failure & reset counter
    const corresponding: Record<string, string> = {};
    for (const delta of deltas) {
      if (nodes.value.get(delta.id)) nodes.value.delete(delta.id); // Remove the node from the store to ensure a clean state before syncing
      try {
        if (delta.action === 'POST') {
          if (delta.node?.parent_id && corresponding[delta.node.parent_id]) delta.node.parent_id = corresponding[delta.node.parent_id];
          const res = await post(delta.node!);
          corresponding[delta.id] = res.id;
        } else if (delta.action === 'PUT' && delta.node) {
          await update(delta.node! as Node);
        } else if (delta.action === 'DELETE') {
          await remove(delta.id);
        }
      } catch (error) {
        console.error(`[store/nodes] Failed to sync delta ${delta.action} for node ${delta.id}:`, error);
      }
    }
  }

  // (recomputeTags retired with the full load: tags come from fetchTags plus per-page merges.)

  // Check if a node (descendantId) is a descendant of another node.
  // Partial-data blind spot (lazy loading): an unloaded ancestor reads as
  // "not a descendant". UI guards only — the server rejects real cycles.
  const isDescendant = (parent: Node, descendantId: string): boolean => {
    let current = nodes.value.get(descendantId);
    while (current?.parent_id) {
      if (current.parent_id === parent.id) return true;
      current = nodes.value.get(current.parent_id);
    }
    return false;
  };
  // Get all descendant IDs of a node
  const getDescendantIds = (id: string): string[] => {
    const result: string[] = [];
    const queue: string[] = [id];

    while (queue.length > 0) {
      const currentId = queue.shift()!;
      const childrenIds = nodes.value.getChildrenIds(currentId);
      for (const childId of childrenIds) {
        result.push(childId);
        queue.push(childId);
      }
    }
    return result;
  };

  const search = (options: SearchOptions, nodesToSearch: Node[] | readonly Node[] = nodes.value.toSortedArray()) => {
    const { query, fromDate, toDate, dateType = 'modified', tags, category, matchMode = 'includes' } = options;

    const queryLower = query?.toLowerCase().trim();
    const hasQuery = Boolean(queryLower);
    const hasTags = tags && tags.length > 0;

    const kanbanBoard: KanbanMetadata | undefined = (nodes.value.get(options.boardId || '')?.metadata as KanbanMetadata) ?? undefined;

    const filtered = nodesToSearch.filter(node => {
      // --- Filter by role ---
      if (options.role && node.role !== options.role) return false;
      // --- Filter by text search ---
      if (hasQuery) {
        const content = `${node.name ?? ''} ${node.description ?? ''} ${node.tags ?? ''}`.toLowerCase();
        switch (matchMode) {
          case 'starts':
            if (!content.startsWith(queryLower!)) return false;
            break;
          case 'exact':
            if (node.name.toLowerCase() !== queryLower) return false;
            break;
          default: // includes
            if (!content.includes(queryLower!)) return false;
        }
      }

      // --- Filter by kanban assignment status ---
      if (options.boardId) {
        if (options.kanbanAssignStatus && kanbanBoard) {
          const users = kanbanBoard?.kanban?.users ?? {};
          if (options.kanbanAssignStatus === 'assigned' && !users[node.id]) return false;
          if (options.kanbanAssignStatus === 'unassigned' && users[node.id]) return false;
        }
        if (options.assignedUserId && kanbanBoard) {
          const users = kanbanBoard?.kanban?.users ?? {};
          if (options.assignedUserId && (!users[node.id] || !users[node.id]!.includes(options.assignedUserId))) return false;
        }
      }

      // --- Filter by dates ---
      if (fromDate || toDate) {
        const timestamp = dateType === 'created' ? node.created_timestamp : node.updated_timestamp;
        const nodeDate = new Date(timestamp);
        if (fromDate && nodeDate < fromDate) return false;
        if (toDate && nodeDate > toDate) return false;
      }

      // --- Filter by tags ---
      if (hasTags) {
        if (!node.tags) return false;
        const nodeTags = parseTags(node.tags);
        // All tags must match
        if (!tags!.some(tag => nodeTags.includes(tag))) return false;
      }

      // --- Filter by category ---
      if (category && node.parent_id !== category) return false;

      return true;
    });

    // --- Sort results ---
    return filtered.sort(sortNodes);
  };

  /** Fetch one full row by id (deep links, link names, refresh-after-write). Merges, returns the live row. */
  async function fetch(opts: FetchOptions): Promise<Node> {
    const cached = nodes.value.get(opts.id);
    if (cached && !cached.partial) return cached;
    console.debug(`[store/nodes] Fetching node: ${opts.id}`);
    const request = await makeRequest(`nodes/${opts.id}`, 'GET', {});
    if (request.status == 'success') {
      const result = request.result as { node: DB_Node; permissions: Permission[] };
      const shared = nodes.value.get(opts.id)?.shared ?? false;
      // Keep the server parent_id verbatim (lazy loading: an unloaded
      // parent is routine, not corruption — never detach).
      const updatedNode: Node = {
        ...(result.node as DB_Node),
        parent_id: result.node.parent_id,
        partial: false,
        synced: true,
        shared: shared,
        permissions: result.permissions,
      };
      nodes.value.set(opts.id, updatedNode, true);
      return updatedNode;
    } else throw request.message;
  }

  /** Fetch every soft-link position anchored on the caller's own nodes. */
  async function fetchLinks(): Promise<void> {
    if (import.meta.server) return;
    try {
      const request = await makeRequest('nodes/links', 'GET', {});
      if (request.status === 'success') {
        links.value = (request.result as DB_NodeLink[]) ?? [];
      } else {
        links.value = [];
      }
    } catch (e) {
      console.warn('[store/nodes] fetchLinks failed; clone positions hidden', e);
      links.value = [];
    }
  }

  /** Clone a note into another folder (soft link). Rejects with server error. */
  async function cloneTo(nodeId: string, parentId: string): Promise<void> {
    const request = await makeRequest(`nodes/${nodeId}/links`, 'POST', { parent_id: parentId });
    if (request.status !== 'success') {
      throw new Error(request.message || 'clone failed');
    }
    const row = request.result as DB_NodeLink;
    if (row && !links.value.some(l => l.id === row.id)) {
      links.value = [...links.value, row];
      invalidateDir(parentId);
    }
  }

  /** Remove one clone position (the note itself and its other positions stay). */
  async function removeLink(nodeId: string, linkId: string): Promise<void> {
    const request = await makeRequest(`nodes/${nodeId}/links/${linkId}`, 'DELETE', {});
    if (request.status !== 'success') {
      throw new Error(request.message || 'remove link failed');
    }
    const gone = links.value.find(l => l.id === linkId);
    links.value = links.value.filter(l => l.id !== linkId);
    if (gone) invalidateDir(gone.parent_id);
  }

  async function fetchShared(): Promise<IndexedCollection> {
    console.debug(`[store/nodes] Fetching shared nodes`);
    const request = await makeRequest(`nodes/shared/@me`, 'GET', {});
    if (request.status === 'success') {
      if (!request.result) return nodes.value as IndexedCollection;
      nodes.value.startBulk();
      for (const node of request.result as DB_Node[]) {
        // Keep the server parent_id verbatim (see fetch() — never detach).
        const finalParentId = node.parent_id;
        if (!nodes.value.has(node.id))
          nodes.value.set(node.id, { ...node, parent_id: finalParentId, partial: true, synced: true, shared: true, permissions: node.permissions || [] });
        else {
          const state = nodes.value.get(node.id);
          nodes.value.set(node.id, {
            ...node,
            parent_id: finalParentId,
            partial: true,
            synced: true,
            shared: state?.shared ?? true,
            permissions: node.permissions || [],
          });
        }
      }
      nodes.value.endBulk();
      return nodes.value as IndexedCollection;
    } else throw request;
  }
  /**
   * Performs a fulltext search on the server using MySQL FULLTEXT index
   * This is optimized for searching in document content (body)
   * @param query - Search query (min 2 characters)
   * @param includeContent - Whether to search in document body content
   * @param limit - Max results to return
   */
  async function searchFulltext(query: string, includeContent = true, limit = 20): Promise<NodeSearchResult[]> {
    if (query.length < 2) return [];
    const params = new URLSearchParams({
      q: query,
      content: includeContent.toString(),
      limit: limit.toString(),
    });
    const request = await makeRequest(`nodes/search?${params.toString()}`, 'GET', {});
    if (request.status === 'success') {
      return request.result as NodeSearchResult[];
    }
    console.error('[store/nodes] Fulltext search failed:', request.message);
    return [];
  }

  async function post(node: Partial<Node>): Promise<DB_Node> {
    const oldId = node.id;
    delete node.id;
    if (!node.parent_id) delete node.parent_id; // Ensure parent_id is not sent as empty string
    const response = await makeRequest<DB_Node | { node: DB_Node }>('nodes', 'POST', { ...node, id: undefined });
    if (response.status == 'success') {
      // Folder creation returns `{node}`; everything else the row itself.
      const payload = response.result!;
      const created = 'node' in payload ? payload.node : payload;
      nodes.value.set(created.id, { ...created, partial: false, synced: true, shared: false, permissions: [] }, true);
      await refreshDir(created.parent_id ?? null);
      return created;
    } else if (isNetworkError(response)) {
      node.user_id = userStore.user?.id;
      const localNode = await LocalDbService.saveCreateDelta({ ...node, id: oldId });
      nodes.value.set(localNode.id, localNode, true);
      return localNode;
    }
    throw response.message || 'Failed to create node';
  }

  async function update(node: Node): Promise<void> {
    if (node.partial) {
      console.debug('[store/nodes] Node looks partial, cannot update it directly.');
      const full_node = await fetch({ id: node.id });
      if (!full_node) throw 'Node not found';
      node = mergeNode(node, full_node);
    }
    const oldParent = nodes.value.get(node.id)?.parent_id ?? null;
    if (!node.parent_id) delete node.parent_id; // Ensure parent_id is not sent as empty string
    const response = await makeRequest(`nodes/${node.id}`, 'PUT', node);
    if (response.status == 'success') {
      node.updated_timestamp = Date.now(); // approximate update time for better UX & backups import
      node.synced = true;
      // Never detach an unloaded parent (see fetch()); order/name edits
      // re-sort the directory, moves touch two.
      nodes.value.set(node.id, node, true);
      await refreshDir(oldParent);
      await refreshDir(node.parent_id ?? null);
    } else if (isNetworkError(response)) {
      await LocalDbService.saveUpdateDelta(node);
      node.synced = false;
      nodes.value.set(node.id, node, true);
    }
    nodes.value.rebuildIndexes();
  }

  async function duplicate(node: Node): Promise<DB_Node> {
    if (!node) throw 'Node not found in store, cannot duplicate';
    if (node.partial) {
      console.debug('[store/nodes] Node looks partial, cannot duplcate it directly.');
      const full_node = await fetch({ id: node.id });
      if (!full_node) throw 'Node not found';
      node = mergeNode(node, full_node);
    }
    const newNodeData: Partial<Node> = {
      name: node.name,
      description: node.description,
      role: node.role,
      parent_id: node.parent_id,
      tags: node.tags,
      accessibility: node.accessibility,
      access: node.access,
      content: node.content,
      content_compiled: node.content_compiled,
    };
    const newNode = await post(newNodeData);
    return newNode;
  }

  async function remove(id: string) {
    const response = await makeRequest(`nodes/${id}`, 'DELETE', {});
    if (response.status == 'success') {
      //
    } else if (isNetworkError(response)) {
      await LocalDbService.saveDeleteDelta(id);
    }
    const parent = nodes.value.get(id)?.parent_id ?? null;
    const allChildrens = getDescendantIds(id);
    nodes.value.delete(id);
    allChildrens.forEach(childId => nodes.value.delete(childId));
    await refreshDir(parent);
  }

  async function bulkDelete(toDelete: Node[]) {
    const deletedIds: string[] = [];
    const failedIds: { id: string; message: string }[] = [];
    for (const node of toDelete) {
      const response = await makeRequest(`nodes/${node.id}`, 'DELETE', {});
      if (response.status === 'success') {
        deletedIds.push(node.id);
      } else if (isNetworkError(response)) {
        await LocalDbService.saveDeleteDelta(node.id);
        deletedIds.push(node.id);
      } else {
        failedIds.push({ id: node.id, message: response.message || 'Unknown error' });
      }
    }
    // Update store
    nodes.value.startBulk();
    deletedIds.forEach(id => {
      const allChildrens = getDescendantIds(id);
      nodes.value.delete(id);
      allChildrens.forEach(childId => nodes.value.delete(childId));
    });
    for (const node of toDelete) {
      if (deletedIds.includes(node.id)) await refreshDir(node.parent_id ?? null);
    }
    nodes.value.endBulk();
    return { deletedIds, failedIds };
  }

  function clear() {
    nodes.value.clear();
    allTags.value = [];
    isFetching.value = false;
    dirStates.value.clear();
    hasChildren.value.clear();
  }

  // ------------------------------------------------------------- trash
  // Soft delete (spec .work/features/trash.md): DELETE /nodes/:id only moves
  // the subtree to the trash; these actions list, restore and purge it.

  async function fetchTrash(): Promise<TrashEntry[]> {
    const request = await makeRequest<TrashEntry[]>('trash', 'GET', {});
    return request.status === 'success' ? (request.result ?? []) : [];
  }

  async function restoreFromTrash(id: string): Promise<{ restored: number; reparented: boolean }> {
    const request = await makeRequest<{ restored: number; reparented: boolean }>(
      `trash/${id}/restore`,
      'POST',
      {},
    );
    if (request.status === 'success' && request.result) {
      // Reload held directory pages so the restored subtree reappears with fresh state.
      await refreshLoadedDirs();
      return request.result;
    }
    return { restored: 0, reparented: false };
  }

  async function purgeFromTrash(id: string): Promise<number> {
    const request = await makeRequest<{ purged: number }>(`trash/${id}`, 'DELETE', {});
    return request.status === 'success' ? (request.result?.purged ?? 0) : 0;
  }

  async function emptyTrash(): Promise<number> {
    const request = await makeRequest<{ purged: number }>('trash', 'DELETE', {});
    return request.status === 'success' ? (request.result?.purged ?? 0) : 0;
  }

  // ---- Lazy directory loading ------------------------------------------------

  function dirStateOf(parentId: string | null): DirState {
    let st = dirStates.value.get(parentId);
    if (!st) {
      st = { total: -1, pages: new Set<number>(), loading: false };
      dirStates.value.set(parentId, st);
    }
    return st;
  }

  function isDirComplete(parentId: string | null): boolean {
    const st = dirStates.value.get(parentId);
    if (!st || st.total < 0) return false;
    const pages = Math.ceil(st.total / PAGE_SIZE);
    for (let p = 0; p < pages; p++) if (!st.pages.has(p)) return false;
    return true;
  }

  function isDirLoading(parentId: string | null): boolean {
    return dirStates.value.get(parentId)?.loading ?? false;
  }

  /** Server-known child total (-1 while never loaded). */
  function dirTotal(parentId: string | null): number {
    return dirStates.value.get(parentId)?.total ?? -1;
  }

  interface DirPage {
    rows: (DB_Node & { has_children?: boolean })[];
    total: number;
  }

  /**
   * Merge one metadata-only row. Never clobbers a full (content-bearing)
   * row — list responses carry no content. Returns the live row.
   */
  function mergePartialRow(row: DB_Node): Node {
    const ex = nodes.value.get(row.id);
    if (ex && !ex.partial) return ex;
    const merged = {
      ...row,
      partial: true,
      synced: true,
      shared: row.user_id !== userStore.user?.id,
      permissions: ex?.permissions ?? [],
    };
    nodes.value.set(row.id, merged, true);
    return merged as Node;
  }

  async function loadDirPage(parentId: string | null, page: number, sortKey = 'name', sortDir = 'asc'): Promise<void> {
    const st = dirStateOf(parentId);
    if (st.loading || st.pages.has(page)) return;
    st.loading = true;
    try {
      const route = parentId === null ? 'nodes/roots' : `nodes/${parentId}/children`;
      const params = new URLSearchParams({
        offset: String(page * PAGE_SIZE),
        limit: String(PAGE_SIZE),
        sort: sortKey,
        order: sortDir,
      });
      const res = await makeRequest<DirPage>(`${route}?${params.toString()}`, 'GET', {});
      if (res.status !== 'success' || !res.result) throw new Error(res.message || 'directory load failed');
      const tags = new Set(allTags.value);
      nodes.value.startBulk();
      for (const row of res.result.rows) {
        mergePartialRow(row);
        if (typeof row.has_children === 'boolean') hasChildren.value.set(row.id, row.has_children);
        if (row.tags) parseTags(row.tags).forEach(tag => tags.add(tag));
      }
      nodes.value.endBulk();
      allTags.value = Array.from(tags).sort();
      st.total = res.result.total;
      st.pages.add(page);
    } finally {
      st.loading = false;
    }
  }

  /** Load the first page unless this directory is already complete. */
  async function ensureChildren(parentId: string | null, sortKey = 'name', sortDir = 'asc'): Promise<void> {
    if (isDirComplete(parentId)) return;
    await loadDirPage(parentId, 0, sortKey, sortDir);
  }

  /** Load the next missing page (scroll-driven); no-op when complete. */
  async function loadMoreChildren(parentId: string | null, sortKey = 'name', sortDir = 'asc'): Promise<void> {
    const st = dirStates.value.get(parentId);
    if (!st || st.loading) {
      if (!st) await loadDirPage(parentId, 0, sortKey, sortDir);
      return;
    }
    if (isDirComplete(parentId)) return;
    const pages = Math.ceil(Math.max(0, st.total) / PAGE_SIZE);
    for (let p = 0; p < pages; p++) {
      if (!st.pages.has(p)) {
        await loadDirPage(parentId, p, sortKey, sortDir);
        return;
      }
    }
  }

  function invalidateDir(parentId: string | null): void {
    dirStates.value.delete(parentId);
  }

  function invalidateAllDirs(): void {
    dirStates.value.clear();
  }

  /**
   * Invalidate one directory and re-pull its held pages (create/rename/move/
   * delete stay visible in expanded trees); unheld directories just drop
   * state and reload on next expansion.
   */
  async function refreshDir(parentId: string | null): Promise<void> {
    const held = dirStates.value.get(parentId);
    dirStates.value.delete(parentId);
    if (!held) return;
    await Promise.allSettled([...held.pages].map(p => loadDirPage(parentId, p)));
  }

  /** True when the upward chain from `id` resolves fully from memory. */
  function isChainComplete(id: string): boolean {
    let cur = nodes.value.get(id);
    if (!cur) return false;
    while (cur.parent_id) {
      cur = nodes.value.get(cur.parent_id);
      if (!cur) return false;
    }
    return true;
  }

  const ancestorInflight = new Set<string>();

  /**
   * Ensure the ancestor chain of `id` is merged (breadcrumbs, reveal,
   * permission walks). Skips when the chain already resolves; merges only
   * missing rows so full rows are never clobbered by partials.
   */
  async function ensureAncestors(id: string): Promise<void> {
    if (!id || isChainComplete(id) || ancestorInflight.has(id)) return;
    ancestorInflight.add(id);
    try {
      const res = await makeRequest<DB_Node[]>(`nodes/${id}/ancestors`, 'GET', {});
      if (res.status !== 'success' || !res.result) return;
      nodes.value.startBulk();
      for (const n of res.result) mergePartialRow(n);
      nodes.value.endBulk();
    } finally {
      ancestorInflight.delete(id);
    }
  }

  /**
   * Adjacent visible siblings under an ordering (prev/next navigation).
   * Merges full rows and returns live store nodes (or null past the ends).
   */
  async function fetchNeighbors(
    id: string,
    sortKey = 'name',
    sortDir = 'asc',
    role?: number,
  ): Promise<{ prev: Node | null; next: Node | null }> {
    const none = { prev: null, next: null };
    if (!id) return none;
    const params = new URLSearchParams({ sort: sortKey, order: sortDir });
    if (role !== undefined) params.set('role', String(role));
    const res = await makeRequest<{ prev: DB_Node | null; next: DB_Node | null }>(
      `nodes/${id}/neighbors?${params.toString()}`,
      'GET',
      {},
    );
    if (res.status !== 'success' || !res.result) return none;
    nodes.value.startBulk();
    const out = {
      prev: res.result.prev ? mergePartialRow(res.result.prev) : null,
      next: res.result.next ? mergePartialRow(res.result.next) : null,
    };
    nodes.value.endBulk();
    return out;
  }

  /** Substring search over name/description/tags (server, any length). Merges hits. */
  async function searchMeta(query: string, role?: number, limit = 20): Promise<Node[]> {
    const params = new URLSearchParams({ q: query, limit: String(limit) });
    if (role !== undefined) params.set('role', String(role));
    const res = await makeRequest<DB_Node[]>(`nodes/search/meta?${params.toString()}`, 'GET', {});
    if (res.status !== 'success' || !res.result) return [];
    nodes.value.startBulk();
    const out = res.result.map(mergePartialRow);
    nodes.value.endBulk();
    return out;
  }

  /** Paged flat list (index pages, cards); merges rows, returns live nodes + total. */
  async function fetchFlat(p: FlatParams): Promise<{ rows: Node[]; total: number }> {
    const params = new URLSearchParams({
      offset: String(p.offset ?? 0),
      limit: String(p.limit ?? 50),
      sort: p.sortKey ?? 'name',
      order: p.sortDir ?? 'asc',
    });
    if (p.role !== undefined) params.set('role', String(p.role));
    if (p.root) params.set('root', p.root);
    if (p.pinned) params.set('pinned', 'true');
    const res = await makeRequest<{ rows: DB_Node[]; total: number }>(`nodes/flat?${params.toString()}`, 'GET', {});
    if (res.status !== 'success' || !res.result) return { rows: [], total: 0 };
    nodes.value.startBulk();
    const rows = res.result.rows.map(mergePartialRow);
    nodes.value.endBulk();
    return { rows, total: res.result.total };
  }

  /** Counts for stats cards, globally or under one subtree root. */
  async function fetchStats(root?: string): Promise<{ total: number; teams: number; workspaces: number; folders: number; documents: number; resources: number }> {
    const empty = { total: 0, teams: 0, workspaces: 0, folders: 0, documents: 0, resources: 0 };
    const params = root ? `?root=${root}` : '';
    const res = await makeRequest<typeof empty>(`nodes/stats${params}`, 'GET', {});
    return res.status === 'success' && res.result ? res.result : empty;
  }

  /** Activity histogram (day/count pairs, newest first); optional subtree root + created|updated field. */
  async function fetchActivity(limit = 15, root?: string, field?: 'created' | 'updated'): Promise<{ day: string; count: number }[]> {
    const params = new URLSearchParams({ limit: String(limit) });
    if (root) params.set('root', root);
    if (field) params.set('field', field);
    const res = await makeRequest<{ day: string; count: number }[]>(`nodes/activity?${params.toString()}`, 'GET', {});
    return res.status === 'success' && res.result ? res.result : [];
  }

  /** Distinct tag strings, unioned into suggestions. */
  async function fetchTags(): Promise<string[]> {
    const res = await makeRequest<string[]>('nodes/tags', 'GET', {});
    if (res.status !== 'success' || !res.result) return [...allTags.value];
    const tags = new Set(allTags.value);
    for (const raw of res.result) parseTags(raw).forEach(tag => tags.add(tag));
    allTags.value = Array.from(tags).sort();
    return allTags.value;
  }

  /** All direct children of one parent (TOC, footer, cards); loops directory pages, merges rows. */
  async function fetchAllChildren(id: string, sortKey = 'name', sortDir = 'asc'): Promise<Node[]> {
    if (!id) return [];
    await ensureChildren(id, sortKey, sortDir);
    let guard = 0;
    while (!isDirComplete(id) && guard++ < 50) await loadMoreChildren(id, sortKey, sortDir);
    return nodes.value
      .getChildrenIds(id)
      .map(cid => nodes.value.get(cid))
      .filter((n): n is Node => !!n);
  }

  /** Assemble nested picker items from flat rows (sidebar sort). Shared by the skeleton loaders. */
  function buildTreeItems(all: Node[]): TreeItem<Node>[] {
    const prefs = usePreferencesStore();
    const sortKey = prefs.get('sortKey');
    const sortOrder = prefs.get('sortOrder');
    const items = new Map<string, TreeItem<Node>>();
    for (const n of all) {
      items.set(n.id, {
        id: n.id,
        parentId: n.role === 1 ? null : (n.parent_id ?? null),
        label: n.name,
        route: resolveNodeLink(n),
        icon: resolveIcon(n),
        data: n,
        children: [],
      });
    }
    const roots: TreeItem<Node>[] = [];
    for (const item of items.values()) {
      const pid = item.data.role === 1 ? null : item.data.parent_id;
      const parent = pid ? items.get(pid) : undefined;
      if (parent) parent.children!.push(item);
      else roots.push(item);
    }
    const cmp = (a: TreeItem<Node>, b: TreeItem<Node>): number => {
      if (a.data.role !== b.data.role) return (b.data.role ?? 0) - (a.data.role ?? 0);
      const orderA = a.data.order ?? 0;
      const orderB = b.data.order ?? 0;
      if (orderA !== orderB) return orderA - orderB;
      let c = 0;
      if (sortKey.value === 'name') c = a.label.localeCompare(b.label);
      else if (sortKey.value === 'created') c = (a.data.created_timestamp ?? 0) - (b.data.created_timestamp ?? 0);
      else if (sortKey.value === 'modified') c = (a.data.updated_timestamp ?? 0) - (b.data.updated_timestamp ?? 0);
      return sortOrder.value === 'ascending' ? c : -c;
    };
    const sortRec = (list: TreeItem<Node>[]): void => {
      list.sort(cmp);
      for (const i of list) if (i.children?.length) sortRec(i.children);
    };
    sortRec(roots);
    return roots;
  }

  /**
   * Folder skeleton for parent pickers (roles 0..maxRole, nested, sorted
   * like the sidebar). Folder counts stay small while documents grow, so
   * one capped sweep per role replaces the in-memory tree; documents as
   * parents (maxRole 3) truncate past the cap.
   */
  async function fetchFolderTree(maxRole = 2, cap = 2000): Promise<TreeItem<Node>[]> {
    const seen = new Set<string>();
    const all: Node[] = [];
    for (let role = 0; role <= maxRole; role++) {
      const page = await fetchFlatAll(role, undefined, cap).catch(() => ({ rows: [], total: 0 }));
      for (const n of page.rows) {
        if (seen.has(n.id)) continue;
        seen.add(n.id);
        all.push(n);
      }
    }
    return buildTreeItems(all);
  }

  /** One subtree as nested picker items (root included, permission scopes), capped. */
  async function fetchSubtree(root: string, cap = 2000): Promise<TreeItem<Node>[]> {
    let self = nodes.value.get(root);
    if (!self) {
      try {
        self = await fetch({ id: root });
      } catch {
        self = undefined;
      }
    }
    const page = await fetchFlatAll(undefined, root, cap).catch(() => ({ rows: [], total: 0 }));
    return buildTreeItems(self ? [self, ...page.rows] : page.rows);
  }

  async function fetchFlatAll(role?: number, root?: string, cap = 1000): Promise<{ rows: Node[]; total: number }> {
    const limit = 500;
    const first = await fetchFlat({ role, root, offset: 0, limit, sortKey: 'name', sortDir: 'asc' });
    const rows = [...first.rows];
    const total = first.total;
    let offset = first.rows.length;
    let last = first.rows.length;
    while (offset < total && rows.length < cap && last > 0) {
      const page = await fetchFlat({ role, root, offset, limit, sortKey: 'name', sortDir: 'asc' });
      rows.push(...page.rows);
      offset += page.rows.length;
      last = page.rows.length;
    }
    return { rows: rows.slice(0, cap), total };
  }

  /**
   * Reload every directory page currently held (plugin writes, restores,
   * bulk imports): captures the loaded page sets, drops the states, then
   * re-pulls the same windows so visible trees refresh in place.
   */
  async function refreshLoadedDirs(): Promise<void> {
    const snapshot = new Map<string | null, number[]>();
    dirStates.value.forEach((st, key) => snapshot.set(key, [...st.pages]));
    dirStates.value.clear();
    await Promise.allSettled(
      [...snapshot.entries()].flatMap(([id, pages]) => pages.map(p => loadDirPage(id, p).catch(() => {}))),
    );
  }

  return {
    nodes: skipHydrate(nodes),
    allTags,
    isFetching,
    getAll,
    getAllTags,
    getById,
    links,
    linksByParent,
    fetchLinks,
    cloneTo,
    removeLink,
    ensureChildren,
    loadMoreChildren,
    isDirComplete,
    dirTotal,
    isDirLoading,
    invalidateDir,
    invalidateAllDirs,
    refreshDir,
    ensureAncestors,
    fetchNeighbors,
    searchMeta,
    fetchFlat,
    fetchFlatAll,
    fetchAllChildren,
    fetchFolderTree,
    fetchSubtree,
    fetchStats,
    fetchActivity,
    fetchTags,
    hasChildren,
    getTotalUsedStorage,
    teams,
    categories,
    workspaces,
    documents,
    resources,
    isDescendant,
    getDescendantIds,
    search,
    init,
    refreshLoadedDirs,
    fetch,
    fetchShared,
    searchFulltext,
    post,
    update,
    duplicate,
    remove,
    bulkDelete,
    clear,
    fetchTrash,
    restoreFromTrash,
    purgeFromTrash,
    emptyTrash,
  };
});
