<template>
  <section class="directory-tree">
    <div class="tree-header">
      <h2>目录</h2>
      <div class="header-actions">
        <label v-if="renderedIds.size" class="select-all" :title="t('nodes.directory.selected', { count: selection.size })">
          <input type="checkbox" :checked="allSelected" @change="toggleAll" />
        </label>
        <AppButton v-if="selection.size" type="danger" small @click="deleteSelected">
          {{ t('nodes.directory.deleteSelected') }} ({{ selection.size }})
        </AppButton>
        <AppSelect v-model="sortId" :items="sortOptions" size="170px" />
      </div>
    </div>
    <div v-if="loading && !children.length" class="tree-loading">{{ t('common.status.loading') }}</div>
    <ul v-else-if="children.length" class="tree-roots">
      <NodeDirectoryTreeRow v-for="child in children" :key="child.id" :node="child" :depth="0" :sort="sort">
        <template #leading="{ node }">
          <input type="checkbox" class="row-check" :checked="selection.has(node.id)" @change="toggle(node.id)" @click.stop />
        </template>
        <template #below="{ node, depth }">
          <div v-if="visibleOutgoing(node).length" class="refs-line" :style="{ paddingLeft: `${depth * 18 + 30}px` }">
            <Icon name="link" display="xsm" class="refs-icon" />
            <span class="refs-label">{{ t('nodes.directory.references') }}</span>
            <a v-for="ref in visibleOutgoing(node)" :key="ref.id" href="#" @click.prevent="openRef(ref)">{{ ref.name }}</a>
            <button v-if="hiddenOutgoing(node) > 0" class="refs-more" @click="outFull.add(node.id)">+{{ hiddenOutgoing(node) }}</button>
          </div>
          <div v-if="visibleIncoming(node).length" class="refs-line refs-incoming" :style="{ paddingLeft: `${depth * 18 + 30}px` }">
            <Icon name="link" display="xsm" class="refs-icon" />
            <span class="refs-label">{{ t('nodes.directory.referencedBy') }}</span>
            <a v-for="ref in visibleIncoming(node)" :key="ref.id" href="#" @click.prevent="openRef(ref)">{{ ref.name }}</a>
            <button v-if="hiddenIncoming(node) > 0" class="refs-more" @click="inFull.add(node.id)">+{{ hiddenIncoming(node) }}</button>
          </div>
        </template>
        <template v-for="(_, name) in $slots" :key="name" #[name]="slotProps">
          <slot :name="name" v-bind="slotProps" />
        </template>
      </NodeDirectoryTreeRow>
      <li v-if="!complete" class="load-more-row">
        <AppButton type="link" :disabled="loading" @click="loadMore">
          {{ loading ? t('common.status.loading') : t('nodes.directory.loadMore') }}
        </AppButton>
      </li>
    </ul>
    <span v-else class="dataview-empty">No results</span>
  </section>
</template>

<script setup lang="ts">
import NodeDeleteModal from '~/components/Node/Modals/Delete.vue';
import { TREE_SORT, useDirectoryLevel, type DirectorySort } from '~/composables/useDirectoryLevel';
import { resolveNodeLink } from '~/helpers/node';
import type { Node, NodeRefs, RefNode } from '~/stores';

const props = defineProps<{ containerId: string }>();

const nodesStore = useNodesStore();
const modals = useModal();
const router = useRouter();
const { t } = useI18nT();

type SortId = 'tree' | 'name-asc' | 'name-desc' | 'created-asc' | 'created-desc' | 'modified-asc' | 'modified-desc';

const sortId = ref<SortId>('tree');

const sortOptions = computed(() => [
  { id: 'tree', label: t('nodes.directory.treeOrder') },
  { id: 'name-asc', label: `${t('common.labels.name')} ↑` },
  { id: 'name-desc', label: `${t('common.labels.name')} ↓` },
  { id: 'created-desc', label: `${t('components.filter.created')} ↓` },
  { id: 'created-asc', label: `${t('components.filter.created')} ↑` },
  { id: 'modified-desc', label: `${t('components.filter.modified')} ↓` },
  { id: 'modified-asc', label: `${t('components.filter.modified')} ↑` },
]);

const sort = computed<DirectorySort>(() => {
  if (sortId.value === 'tree') return TREE_SORT;
  const [key, dir] = sortId.value.split('-') as [DirectorySort['key'], DirectorySort['dir']];
  return { key, dir };
});

const { children, complete, loading, ensure, loadMore } = useDirectoryLevel(
  () => props.containerId,
  sort,
);

// Multi-select (plugin parity): rows register while rendered; select-all
// covers rendered rows only, never unloaded pages.
const selection = ref<Set<string>>(new Set());
const renderedIds = ref<Set<string>>(new Set());
const allSelected = computed(() => renderedIds.value.size > 0 && selection.value.size >= renderedIds.value.size);

const toggle = (id: string) => {
  const next = new Set(selection.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  selection.value = next;
};

const toggleAll = () => {
  selection.value = allSelected.value ? new Set() : new Set(renderedIds.value);
};

const deleteSelected = () => {
  const nodes = [...selection.value]
    .map(id => nodesStore.getById(id))
    .filter((n): n is Node => !!n);
  if (!nodes.length) return;
  modals.add(
    new Modal(shallowRef(NodeDeleteModal), {
      props: { nodes },
      size: 'small',
      onClose: () => {
        // Prune whatever the modal deleted; survivors stay selected.
        selection.value = new Set([...selection.value].filter(id => nodesStore.getById(id)));
      },
    }),
  );
};

provide('directoryRegister', (id: string) => {
  renderedIds.value.add(id);
});
provide('directoryUnregister', (id: string) => {
  renderedIds.value.delete(id);
});
provide('directoryResetSort', () => {
  sortId.value = 'tree';
});

// Inline references (plugin parity): one batch per newly rendered set,
// cached per container. Outgoing targets already below the row as
// children are omitted (best-effort on partial data).
const refsCache = ref<Record<string, NodeRefs>>({});
const outFull = ref<Set<string>>(new Set());
const inFull = ref<Set<string>>(new Set());
let refsReq = 0;

watch(
  () => [...renderedIds.value],
  async ids => {
    const missing = ids.filter(id => !refsCache.value[id]).slice(0, 100);
    if (!missing.length) return;
    const req = ++refsReq;
    const batch = await nodesStore.fetchRefs(missing).catch(() => ({} as Record<string, NodeRefs>));
    if (req !== refsReq) return;
    refsCache.value = { ...refsCache.value, ...batch };
  },
);

const outgoingOf = (node: Node): RefNode[] => {
  const all = refsCache.value[node.id]?.outgoing ?? [];
  return all.filter(r => r.id !== node.id && !nodesStore.isDescendant(node, r.id));
};

const incomingOf = (node: Node): RefNode[] => refsCache.value[node.id]?.incoming ?? [];

const visibleOutgoing = (node: Node): RefNode[] => {
  const list = outgoingOf(node);
  return outFull.value.has(node.id) ? list : list.slice(0, 2);
};

const hiddenOutgoing = (node: Node): number =>
  outFull.value.has(node.id) ? 0 : Math.max(0, outgoingOf(node).length - 2);

const visibleIncoming = (node: Node): RefNode[] => {
  const list = incomingOf(node);
  return inFull.value.has(node.id) ? list : list.slice(0, 2);
};

const hiddenIncoming = (node: Node): number =>
  inFull.value.has(node.id) ? 0 : Math.max(0, incomingOf(node).length - 2);

const openRef = (ref: RefNode) => router.push(resolveNodeLink(ref as unknown as Node));

watch(
  () => props.containerId,
  () => {
    // Temporary sorting resets when the container changes (plugin parity).
    sortId.value = 'tree';
    selection.value = new Set();
    refsCache.value = {};
    outFull.value = new Set();
    inFull.value = new Set();
    ensure();
  },
  { immediate: true },
);
</script>

<style scoped lang="scss">
.directory-tree {
  .tree-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 8px;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .select-all {
    display: inline-flex;
    align-items: center;
    cursor: pointer;
  }

  .row-check {
    flex-shrink: 0;
    cursor: pointer;
  }

  .refs-line {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    padding: 1px 4px 3px 30px;
    font-size: 0.78rem;
    color: var(--text-secondary);

    a {
      color: var(--primary);
      text-decoration: none;

      &:hover {
        text-decoration: underline;
      }
    }
  }

  .refs-icon {
    flex-shrink: 0;
  }

  .refs-label {
    flex-shrink: 0;
  }

  .refs-more {
    border: none;
    background: transparent;
    color: var(--primary);
    cursor: pointer;
    font-size: 0.78rem;
    padding: 0;
  }

  h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }

  .tree-roots {
    margin: 0;
    padding: 0;
  }

  .tree-loading {
    color: var(--text-secondary);
    font-size: 0.85rem;
  }

  .load-more-row {
    list-style: none;
  }
}
</style>
