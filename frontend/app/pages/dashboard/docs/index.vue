<template>
  <div>
    <div class="docs-tabs">
      <button :class="{ active: tab === 'all' }" @click="selectTab('all')">
        {{ t('common.labels.all') }}
      </button>
      <button :class="{ active: tab === 'recent' }" @click="selectTab('recent')">
        {{ t('nodes.container.recentTab') }}
      </button>
    </div>
    <NodeContainerView :parent="parent" :nodes="rows" :parent-id="workspaceId" :total="total" :loading="loading" :disable-sort="tab === 'recent'" :show-viewed="tab === 'recent'" />
  </div>
</template>

<script setup lang="ts">
import type { Node } from '~/stores';
import localForage from 'localforage';
import { globalRecentSource, onlyViewed, runDqlQuery } from '~/helpers/dataview-render';

definePageMeta({ breadcrumb: { i18n: 'common.labels.all' } });

const nodesStore = useNodesStore();
const { t } = useI18nT();

const { workspaceId } = useSidebar();

const tab = ref<'all' | 'recent'>('all');
const parent = computed(() => nodesStore.getById(workspaceId.value || ''));
const rows = ref<Node[]>([]);
const total = ref(0);
const loading = ref(true);

let requestId = 0;
watch(
  [workspaceId, tab],
  async ([wid, current]) => {
    const id = ++requestId;
    rows.value = [];
    total.value = 0;
    loading.value = true;
    try {
      if (wid && !nodesStore.getById(wid)) await nodesStore.ensureAncestors(wid).catch(() => {});
      if (current === 'recent') {
        const apiBase = (useRuntimeConfig().public.baseApi as string) || '';
        const result = await runDqlQuery(apiBase, globalRecentSource()).catch(() => null);
        if (id !== requestId) return;
        rows.value = result ? onlyViewed(result.rows).slice(0, 50) : [];
        total.value = rows.value.length;
      } else {
        const page = await nodesStore.fetchFlatAll(3, wid || undefined).catch(() => ({ rows: [], total: 0 }));
        if (id !== requestId) return;
        rows.value = page.rows;
        total.value = page.total;
      }
    } finally {
      if (id === requestId) loading.value = false;
    }
  },
  { immediate: true },
);

function selectTab(next: 'all' | 'recent') {
  tab.value = next;
  localForage.setItem('docsTab', next);
}

onMounted(async () => {
  const stored = await localForage.getItem<unknown>('docsTab');
  if (stored === 'all' || stored === 'recent') tab.value = stored;
});
</script>

<style scoped lang="scss">
.docs-tabs {
  position: sticky;
  top: 0;
  z-index: 10;
  display: flex;
  gap: 8px;
  // Symmetric 16px pills gaps (8 margin + 8 padding each side); the bar keeps
  // 8px clear of the card so the frame never touches the tab zone.
  margin: 8px 4px 0;
  padding: 8px 0;
  background: var(--body-fixed);

  button {
    padding: 0.45rem 1.1rem;
    border: none;
    border-radius: var(--radius-sm);
    font-size: 14px;
    font-weight: 500;
    color: var(--text-secondary);
    background: transparent;
    cursor: pointer;
    transition:
      color $transition-fast ease,
      background $transition-fast ease;

    &:hover {
      color: var(--text-body);
    }

    &.active {
      color: var(--primary);
      background: var(--primary-bg);
    }
  }
}
</style>
