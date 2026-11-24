<template>
  <div class="page-card">
    <Teleport to="#navbar-title"><Icon name="settings" display="lg" />{{ t('components.sidebar.nav.settings') }}</Teleport>
    <Teleport to="#navbar-bottom"><NodeTeamNavbar :team-id="nodeId" /></Teleport>
    <form v-if="node" @submit.prevent>
      <Teleport to="#navbar-actions">
        <AppBtnIcon nav icon="save" :tooltip="t('common.actions.update')" @click="updateCategory" />
      </Teleport>
      <div class="form-row">
        <label>{{ t('common.labels.name') }}</label>
        <input id="name" v-model="node.name" type="text" required maxlength="50" />
      </div>

      <div class="form-row">
        <label>{{ t('common.labels.description') }}</label>
        <input id="description" v-model="node.description" type="text" maxlength="250" />
      </div>

      <div class="form-row">
        <label>{{ t('common.labels.tags') }}</label>
        <AppTagInput id="tags" v-model="node.tags" type="text" style="width: 100%" />
      </div>

      <div class="form-row">
        <div class="form-column">
          <label>{{ t('common.labels.parent') }}</label>
          <AppSelect
            v-model="node.parent_id"
            :items="categoriesItem"
            :placeholder="t('common.placeholder.parent')"
            nullable
            :disabled="i => i.id == node!.id || descendantIds.has(String(i.id))"
          />
        </div>
        <div class="form-column">
          <label for="order">{{ t('common.labels.order') }}</label>
          <input id="order" v-model.number="node.order" type="number" />
        </div>
      </div>
      <label style="display: flex; align-items: center">{{ t('common.labels.icon') }} <AppHint :text="t('nodes.category.iconHint')" /></label>
      <textarea v-model="node.icon" type="text" rows="5" />

      <label for="color">{{ t('common.labels.color') }}</label>
      <AppColorPicker v-model="node.color" name="color" nullable />
    </form>
  </div>
</template>

<script lang="ts" setup>
import type { TreeItem } from '~/helpers/TreeBuilder';
import type { Node } from '~/stores';

definePageMeta({ breadcrumb: { i18n: 'common.actions.edit' } });

const nodesStore = useNodesStore();

const { t } = useI18nT();
const notifications = useNotifications();
const route = useRoute();
const router = useRouter();

const nodeId = route.params.id as string;
const node = computed(() => nodesStore.getById(nodeId));
const categoriesItem = ref<TreeItem<Node>[]>([]);
const descendantIds = ref<Set<string>>(new Set());

// Actions
const updateCategory = async () => {
  if (node.value)
    nodesStore
      .update(node.value)
      .then(() => {
        notifications.add({ title: t('nodes.category.notifications.updated'), type: 'success' });
        router.push(`/dashboard/teams/${nodeId}`);
      })
      .catch(e => notifications.add({ message: e, title: t('common.status.error'), type: 'error' }));
};

// Shortcuts
function handleKeydown(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.key === 's') {
    event.preventDefault();
    updateCategory();
  }
}

onMounted(async () => {
  document.addEventListener('keydown', handleKeydown);
  if (nodeId && !nodesStore.getById(nodeId)) nodesStore.fetch({ id: nodeId }).catch(() => {});
  categoriesItem.value = await nodesStore.fetchFolderTree(2).catch(() => []);
  const sub = await nodesStore.fetchFlatAll(undefined, nodeId).catch(() => ({ rows: [] as Node[] }));
  descendantIds.value = new Set(sub.rows.map(n => n.id));
});

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeydown);
});
</script>

<style scoped lang="scss">
label {
  margin-top: 10px;
}

.form-row {
  display: flex;
  flex-wrap: wrap;
  gap: 0 20px;
}

.form-column {
  flex: 1;
  min-width: 200px;
}
</style>
