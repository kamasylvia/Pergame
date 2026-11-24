import { IndexedCollection } from '~/helpers/IndexedCollection';
import { parseTags, mergeNode } from '~/helpers/node';
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
    await Promise.all([syncLocalDeltas(), fetch(), fetchShared(), fetchLinks()]);
    nodes.value.endBulk();
    seedDirStates();
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

  function recomputeTags() {
    const tags = new Set<string>();

    nodes.value.startBulk();

    nodes.value.forEach(node => {
      if (node.tags) {
        parseTags(node.tags).forEach(tag => tags.add(tag));
      }
      // NOTE: no orphan-detach here anymore (lazy loading keeps parents
      // unloaded routinely — detaching would corrupt them). Dangling rows
      // float to the root in the tree builder without writes.
    });

    nodes.value.endBulk();
    allTags.value = Array.from(tags).sort();
  }

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

  async function fetch<T extends FetchOptions>(opts?: T): Promise<'id' extends keyof T ? Node : IndexedCollection> {
    if (opts?.id && !nodes.value.get(opts.id)?.partial) return nodes.value.get(opts.id) as 'id' extends keyof T ? Node : IndexedCollection;
    console.debug(`[store/nodes] Fetching nodes with options: ${JSON.stringify(opts)}`);
    const request = await makeRequest(`nodes/${opts?.id ? opts.id : 'user/@me'}`, 'GET', {});
    if (request.status == 'success') {
      if (opts?.id) {
        const result = request.result as { node: DB_Node; permissions: Permission[] };
        const shared = nodes.value.get(opts.id)?.shared ?? false;
        // Keep the server parent_id verbatim (lazy loading: an unloaded
        // parent is routine, not corruption — never detach).
        const finalParentId = result.node.parent_id;

        const updatedNode: Node = {
          ...(result.node as DB_Node),
          parent_id: finalParentId,
          partial: false,
          synced: true,
          shared: shared,
          permissions: result.permissions,
        };
        nodes.value.set(opts.id, updatedNode, true);
        return updatedNode as 'id' extends keyof T ? Node : IndexedCollection;
      } else {
        if (!request.result) return nodes.value as 'id' extends keyof T ? Node : IndexedCollection;
        nodes.value.startBulk();
        for (const node of request.result as DB_Node[]) {
          nodes.value.set(node.id, { ...node, partial: true, synced: true, shared: false, permissions: [] });
        }
        nodes.value.endBulk();
        recomputeTags();
        seedDirStates();
        return nodes.value as 'id' extends keyof T ? Node : IndexedCollection;
      }
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
      invalidateDir(created.parent_id ?? null);
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
      invalidateDir(oldParent);
      invalidateDir(node.parent_id ?? null);
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
    invalidateDir(parent);
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
      if (deletedIds.includes(node.id)) invalidateDir(node.parent_id ?? null);
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
      // Re-fetch the tree so the restored subtree reappears with fresh state.
      await fetch();
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

  interface DirPage {
    rows: (DB_Node & { has_children?: boolean })[];
    total: number;
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
      const me = userStore.user?.id;
      const tags = new Set(allTags.value);
      nodes.value.startBulk();
      for (const row of res.result.rows) {
        nodes.value.set(
          row.id,
          {
            ...row,
            partial: true,
            synced: true,
            shared: row.user_id !== me,
            permissions: nodes.value.get(row.id)?.permissions ?? [],
          },
          true,
        );
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
      const me = userStore.user?.id;
      nodes.value.startBulk();
      for (const n of res.result) {
        if (nodes.value.has(n.id)) continue;
        nodes.value.set(n.id, { ...n, partial: true, synced: true, shared: n.user_id !== me, permissions: [] }, true);
      }
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
    const me = userStore.user?.id;
    const merge = (n: DB_Node | null): Node | null => {
      if (!n) return null;
      const ex = nodes.value.get(n.id);
      if (ex) return ex;
      const row = { ...n, partial: true, synced: true, shared: n.user_id !== me, permissions: [] };
      nodes.value.set(n.id, row, true);
      return row as Node;
    };
    return { prev: merge(res.result.prev), next: merge(res.result.next) };
  }

  /**
   * Migration bridge: after a full load, mark every directory complete with
   * locally derived totals so the lazy layer stays idle. Retires when init()
   * flips to roots-only.
   */
  function seedDirStates(): void {
    const counts = new Map<string | null, number>();
    let roots = 0;
    nodes.value.forEach(node => {
      const key = node.parent_id ?? null;
      counts.set(key, (counts.get(key) ?? 0) + 1);
      if ((node.role ?? 99) <= 1 || !node.parent_id || !nodes.value.has(node.parent_id)) roots++;
      const kids = nodes.value.getChildrenIds(node.id).length > 0 || (linksByParent.value.get(node.id)?.length ?? 0) > 0;
      hasChildren.value.set(node.id, kids);
    });
    counts.forEach((total, key) => {
      const pages = new Set<number>();
      for (let p = 0; p < Math.ceil(total / PAGE_SIZE); p++) pages.add(p);
      dirStates.value.set(key, { total, pages, loading: false });
    });
    const rootPages = new Set<number>();
    for (let p = 0; p < Math.ceil(roots / PAGE_SIZE); p++) rootPages.add(p);
    dirStates.value.set(null, { total: roots, pages: rootPages, loading: false });
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
    isDirLoading,
    invalidateDir,
    invalidateAllDirs,
    ensureAncestors,
    fetchNeighbors,
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
    recomputeTags,
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
