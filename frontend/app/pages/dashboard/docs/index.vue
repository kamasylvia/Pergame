<template>
  <NodeContainerView :parent="parent" :nodes="rows" :parent-id="workspaceId" :total="total" :loading="loading" />
</template>

<script setup lang="ts">
import type { Node } from '~/stores';

definePageMeta({ breadcrumb: { i18n: 'common.labels.all' } });

const nodesStore = useNodesStore();

const { workspaceId } = useSidebar();

const parent = computed(() => nodesStore.getById(workspaceId.value || ''));
const rows = ref<Node[]>([]);
const total = ref(0);
const loading = ref(true);

watch(
  workspaceId,
  async wid => {
    rows.value = [];
    total.value = 0;
    loading.value = true;
    try {
      if (wid && !nodesStore.getById(wid)) await nodesStore.ensureAncestors(wid).catch(() => {});
      const page = await nodesStore.fetchFlatAll(3, wid || undefined).catch(() => ({ rows: [], total: 0 }));
      rows.value = page.rows;
      total.value = page.total;
    } finally {
      loading.value = false;
    }
  },
  { immediate: true },
);
</script>
