<template>
  <section class="metadata-panel">
    <div class="metadata-panel-header">
      <span class="metadata-panel-title" :title="doc.name">
        <Icon :name="live?.icon || 'files'" display="sm" :class="['parent-icon', getAppAccent((live?.color ?? 0) as number, true)]" />
        {{ live?.name }}
      </span>
      <div class="metadata-panel-actions">
        <AppBtnIcon icon="close" :tooltip="t('common.actions.close')" @click="emit('close')" />
      </div>
    </div>
    <div class="metadata-panel-body">
      <NodeModalsMetadataForm :doc="doc" />
    </div>
  </section>
</template>

<script setup lang="ts">
import type { Node } from '~/stores';

const props = defineProps<{ doc: Node }>();
const emit = defineEmits(['close']);

const nodesStore = useNodesStore();
const { getAppAccent } = useAppColors();
const { t } = useI18nT();

const live = computed(() => nodesStore.getById(props.doc.id) ?? props.doc);
</script>

<style scoped lang="scss">
.metadata-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  border-left: 1px solid var(--border-color);
  background: var(--bg-color);

  .metadata-panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-bottom: 1px solid var(--border-color);
  }

  .metadata-panel-title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
    overflow: hidden;
  }

  .metadata-panel-actions {
    display: flex;
    flex-shrink: 0;
    gap: 4px;
  }

  .metadata-panel-body {
    flex: 1;
    min-height: 0;
    padding: 8px 12px;
    overflow-y: auto;
  }
}
</style>
