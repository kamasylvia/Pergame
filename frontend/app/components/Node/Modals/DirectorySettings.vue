<template>
  <div class="modal">
    <h3>{{ t('nodes.directory.settings') }}</h3>
    <AppPreferenceInputSection :options="options" />
    <div class="footer">
      <AppButton type="primary" @click="emit('close')">{{ t('common.actions.close') }}</AppButton>
    </div>
  </div>
</template>

<script setup lang="ts">
const emit = defineEmits(['close']);

const { t } = useI18nT();

// Live-apply (Pergame prefs persist on write); the tree watches the
// auto-expand keys and re-seeds, so no save button is needed.
const options = computed<Array<{ label: string; options: Option[] }>>(() => [
  {
    label: t('nodes.directory.settings'),
    options: [
      {
        label: t('nodes.directory.maxDepth'),
        type: 'number',
        key: 'directoryMaxDepth',
        min: 0,
        max: 50,
      },
      {
        label: t('nodes.directory.expandedDepth'),
        type: 'number',
        key: 'directoryExpandedDepth',
        min: 0,
        max: 50,
      },
      { label: t('nodes.directory.autoExpand'), type: 'toggle', key: 'directoryAutoExpand' },
      { label: t('nodes.directory.showCount'), type: 'toggle', key: 'directoryShowCount' },
      { label: t('nodes.directory.showUpdated'), type: 'toggle', key: 'directoryShowUpdated' },
      { label: t('nodes.directory.showCreated'), type: 'toggle', key: 'directoryShowCreated' },
      { label: t('nodes.directory.showRefs'), type: 'toggle', key: 'directoryShowRefs' },
      { label: t('nodes.directory.showTags'), type: 'toggle', key: 'directoryShowTags' },
      {
        label: t('settings.other.confirmDirectoryMove'),
        description: t('settings.other.confirmDirectoryMoveDesc'),
        type: 'toggle',
        key: 'directoryConfirmMove',
      },
    ],
  },
]);
</script>

<style scoped lang="scss">
.modal {
  display: flex;
  flex-direction: column;
  gap: 12px;

  h3 {
    margin: 0;
    font-size: 15px;
  }

  .footer {
    display: flex;
    justify-content: flex-end;
  }
}
</style>
