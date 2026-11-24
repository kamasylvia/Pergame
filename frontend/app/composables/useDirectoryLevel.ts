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
  if (sort.key === 'tree') return rows;
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

export function useDirectoryLevel(parentId: MaybeRef<string>, sort: MaybeRef<DirectorySort>) {
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
