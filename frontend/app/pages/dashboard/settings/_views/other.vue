<template>
  <div>
    <h2 class="page-title">{{ t('settings.other.title') }}</h2>
    <p class="page-subtitle">{{ t('settings.other.subtitle') }}</p>

    <AppPreferenceInputSection :options="options" />
  </div>
</template>

<script setup lang="ts">
import type { TreeItem } from '~/helpers/TreeBuilder';
import type { Node } from '~/stores';
type InterfaceOption = Option & {
  tag?: string;
};

const { t } = useI18nT();
const nodesStore = useNodesStore();
const categories = ref<TreeItem<Node>[]>([]);

onMounted(async () => {
  categories.value = await nodesStore.fetchFolderTree(2).catch(() => []);
});

const options = computed<Array<{ label: string; options: InterfaceOption[] }>>(() => [
  {
    label: t('settings.other.uploads'),
    options: [
      {
        label: t('settings.other.defaultUploadFolder'),
        description: t('settings.other.defaultUploadFolderDesc'),
        type: 'select',
        key: 'defaultUploadFolder',
        choices: categories,
      },
    ],
  },
  {
    label: t('settings.other.sort'),
    options: [
      {
        label: t('settings.other.sortKey'),
        description: t('settings.other.sortKeyDesc'),
        tag: t('settings.nav.new'),
        type: 'select',
        key: 'sortKey',
        choices: [
          { label: t('common.labels.name'), id: 'name' },
          { label: t('components.filter.created'), id: 'created' },
          { label: t('components.filter.modified'), id: 'modified' },
        ],
      },
      {
        label: t('settings.other.sortOrder'),
        description: t('settings.other.sortOrderDesc'),
        tag: t('settings.nav.new'),
        type: 'select',
        key: 'sortOrder',
        choices: [
          { label: t('components.filter.ascending'), id: 'ascending' },
          { label: t('components.filter.descending'), id: 'descending' },
        ],
      },
    ],
  },
]);
</script>
