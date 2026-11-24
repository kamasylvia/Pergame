<template>
  <section class="directory-tree">
    <div class="tree-header">
      <h2>目录</h2>
      <AppSelect v-model="sortId" :items="sortOptions" size="170px" />
    </div>
    <div v-if="loading && !children.length" class="tree-loading">{{ t('common.status.loading') }}</div>
    <ul v-else-if="children.length" class="tree-roots">
      <NodeDirectoryTreeRow v-for="child in children" :key="child.id" :node="child" :depth="0" :sort="sort">
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
import { TREE_SORT, useDirectoryLevel, type DirectorySort } from '~/composables/useDirectoryLevel';

const props = defineProps<{ containerId: string }>();

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

watch(
  () => props.containerId,
  () => {
    // Temporary sorting resets when the container changes (plugin parity).
    sortId.value = 'tree';
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
