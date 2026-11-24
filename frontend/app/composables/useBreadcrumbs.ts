import type { NodeRole } from '~/stores';

const ROUTE_ROLE_MAPPING: Record<NodeRole, string> = {
  0: '/dashboard/teams',
  1: '/dashboard/categories',
  2: '/dashboard/categories',
  3: '/dashboard/docs',
  4: '/dashboard/cdn',
};

export function useBreadcrumbs() {
  const nodesTree = useNodesTree();
  const nodesStore = useNodesStore();

  function generateBreadcrumbsById(id: string) {
    // Lazy fill: merge the ancestor chain on demand (no-op when complete);
    // callers re-render reactively once it lands. This also feeds the
    // permission-inheritance walks on the same pages.
    void nodesStore.ensureAncestors(id);
    const nodes = nodesTree.getAncestors(id);
    return nodes
      .map(node => ({
        name: node.label,
        path: `${ROUTE_ROLE_MAPPING[node.data.role]}/${node.id}`,
      }))
      .reverse();
  }

  return {
    generateBreadcrumbsById,
  };
}
