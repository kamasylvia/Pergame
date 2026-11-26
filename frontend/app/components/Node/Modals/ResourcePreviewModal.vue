<template>
  <div class="modal resource-preview">
    <h3 class="resource-preview-title">{{ title }}</h3>
    <div class="resource-preview-body">
      <LazyPDFViewer :src="src" />
    </div>
    <div class="footer">
      <AppButton type="secondary" @click="emit('close')">{{ t('common.actions.close') }}</AppButton>
    </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{ title: string; src: string }>();
const emit = defineEmits(['close']);

const { t } = useI18nT();
</script>

<style scoped lang="scss">
.resource-preview {
  display: flex;
  flex-direction: column;
  gap: 12px;

  .resource-preview-title {
    margin: 0;
    font-size: 15px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .resource-preview-body {
    min-height: 40vh;
    max-height: 70vh;
    overflow-y: auto;
  }

  .footer {
    display: flex;
    justify-content: flex-end;
  }
}
</style>
