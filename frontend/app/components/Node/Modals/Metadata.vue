<template>
  <div class="modal-content">
    <h2><Icon :name="live?.icon || 'files'" display="xl" :class="['parent-icon', getAppAccent((live?.color ?? 0) as number, true)]" /> {{ live?.name }}</h2>
    <NodeModalsMetadataForm :doc="doc" />
  </div>
</template>

<script setup lang="ts">
import type { Node } from '~/stores';

const props = defineProps<{ doc: Node }>();

const nodesStore = useNodesStore();
const { getAppAccent } = useAppColors();

const live = computed(() => nodesStore.getById(props.doc.id) ?? props.doc);
</script>

<style scoped lang="scss">
.modal-content {
  display: flex;
  flex-direction: column;
  font-family: $font-ui;
}

h2 {
  display: flex;
}
</style>
