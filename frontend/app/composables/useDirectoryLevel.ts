/**
 * One directory-tree level: reactive children of a container with
 * view-only sorting (tree order keeps server paging order; temp sorts
 * pin-first then key/direction, never persisted).
 */
import type { Node } from '~/stores';

export interface DirectorySort {
  key: 'tree' | 'name' | 'created' | 'modified';
  dir: 'asc' | 'desc';
}

export const TREE_SORT: DirectorySort = { key: 'tree', dir: 'asc' };

function sortLevel(rows: Node[], sort: DirectorySort): Node[] {
  // Tree order mirrors the server (role DESC, order ASC, name ASC) over
  // live field values — collection positions never re-sort on merge.
  if (sort.key === 'tree') {
    return [...rows].sort(
      (a, b) => (b.role ?? 0) - (a.role ?? 0) || (a.order ?? 0) - (b.order ?? 0) || a.name.localeCompare(b.name),
    );
  }
  const pinned = rows.filter(n => n.order === -1);
  const rest = rows.filter(n => n.order !== -1);
  const value = (n: Node): number | string => {
    if (sort.key === 'name') return n.name.toLowerCase();
    if (sort.key === 'created') return n.created_timestamp ?? 0;
    return n.updated_timestamp ?? 0;
  };
  rest.sort((a, b) => {
    const va = value(a);
    const vb = value(b);
    const c = typeof va === 'string' ? va.localeCompare(vb as string) : va - (vb as number);
    return sort.dir === 'asc' ? c : -c;
  });
  return [...pinned, ...rest];
}

export function useDirectoryLevel(parentId: MaybeRefOrGetter<string>, sort: MaybeRefOrGetter<DirectorySort>) {
  const nodesStore = useNodesStore();
  const pid = toRef(parentId);
  const ord = toRef(sort);

  const children = computed(() => {
    const rows = nodesStore.nodes
      .getChildrenIds(pid.value)
      .map(id => nodesStore.getById(id))
      .filter((n): n is Node => !!n && (n.role ?? 99) <= 3);
    return sortLevel(rows, ord.value);
  });

  const total = computed(() => nodesStore.dirTotal(pid.value));
  const complete = computed(() => nodesStore.isDirComplete(pid.value));
  const loading = computed(() => nodesStore.isDirLoading(pid.value));

  const ensure = () => nodesStore.ensureChildren(pid.value).catch(() => {});
  const loadMore = () => nodesStore.loadMoreChildren(pid.value).catch(() => {});

  return { children, total, complete, loading, ensure, loadMore };
}

export type DropZone = 'before' | 'after' | 'into';

const ORDER_STEP = 1024;

/**
 * Directory-tree drop: edge zones reorder within the target's level
 * (moving across parents when needed), the middle zone moves into the
 * target. Orders use 1024-gaps so a typical drop costs one write;
 * exhausted gaps renumber the level. Pins (-1) never take part.
 */
export async function moveDirectoryNode(draggedId: string, targetId: string, zone: DropZone): Promise<void> {
  const nodesStore = useNodesStore();
  const dragged = nodesStore.getById(draggedId);
  const target = nodesStore.getById(targetId);
  if (!dragged || !target || draggedId === targetId) return;

  if (zone === 'into') {
    if (dragged.parent_id === targetId) return;
    await nodesStore.update({ ...dragged, parent_id: targetId });
    return;
  }

  const levelParent = target.parent_id ?? null;
  // Reordering needs the whole level; user-initiated, so pull it fully.
  if (levelParent) await nodesStore.fetchAllChildren(levelParent).catch(() => []);
  const level = nodesStore.nodes
    .getChildrenIds(levelParent)
    .map(id => nodesStore.getById(id))
    .filter((n): n is Node => !!n && (n.role ?? 99) <= 3 && n.order !== -1)
    .sort((a, b) => (a.order ?? 0) - (b.order ?? 0));
  const rest = level.filter(n => n.id !== draggedId);
  const at = rest.findIndex(n => n.id === targetId);
  if (at < 0) return;
  const prev = zone === 'before' ? rest[at - 1] : rest[at];
  const next = zone === 'before' ? rest[at] : rest[at + 1];

  const writeOrder = async (node: Node, order: number, parentId: string | null): Promise<void> => {
    if ((node.order ?? 0) === order && (node.parent_id ?? null) === parentId) return;
    await nodesStore.update({ ...node, order, parent_id: parentId ?? undefined });
  };

  const prevOrder = prev?.order ?? 0;
  const nextOrder = next?.order ?? 0;
  if (prev && next && nextOrder - prevOrder > 1) {
    await writeOrder(dragged, Math.floor((prevOrder + nextOrder) / 2), levelParent);
    return;
  }
  if (prev && !next) {
    await writeOrder(dragged, prevOrder + ORDER_STEP, levelParent);
    return;
  }
  if (!prev && next && nextOrder > ORDER_STEP) {
    await writeOrder(dragged, nextOrder - ORDER_STEP, levelParent);
    return;
  }
  // No gap (or empty level): renumber the level around the drop.
  const seq = [...rest];
  seq.splice(zone === 'before' ? at : at + 1, 0, dragged);
  await Promise.allSettled(
    seq.map((node, i) => {
      const pid = node.id === draggedId ? levelParent : (node.parent_id ?? null);
      return writeOrder(node, (i + 1) * ORDER_STEP, pid);
    }),
  );
}

/**
 * Deep-copy nodes (docs + folders) under a target folder. Children are
 * snapshotted before each insert so a copy landing inside its own source
 * subtree cannot recurse forever (the picker also disables those
 * targets); resource attachments stay with the originals.
 */
export async function deepCopyNodes(ids: string[], targetParentId: string): Promise<{ copied: number; failed: string[] }> {
  const nodesStore = useNodesStore();
  let copied = 0;
  const failed: string[] = [];
  const copyOne = async (id: string, parentId: string): Promise<void> => {
    let node = nodesStore.getById(id);
    if (!node) throw new Error(id);
    if (node.partial) node = await nodesStore.fetch({ id });
    if (node.id === parentId) throw new Error(node.name || id);
    const kids = await nodesStore.fetchAllChildren(id).catch(() => [] as Node[]);
    const created = await nodesStore.post({
      name: node.name,
      description: node.description,
      role: node.role,
      parent_id: parentId,
      tags: node.tags,
      accessibility: node.accessibility,
      access: node.access,
      content: node.content,
      content_compiled: node.content_compiled,
    });
    copied += 1;
    for (const kid of kids) {
      if ((kid.role ?? 99) > 3) continue;
      await copyOne(kid.id, created.id);
    }
  };
  for (const id of ids) {
    try {
      await copyOne(id, targetParentId);
    } catch (e) {
      failed.push(e instanceof Error ? e.message : String(e));
    }
  }
  return { copied, failed };
}
