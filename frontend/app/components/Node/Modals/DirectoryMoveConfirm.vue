<template>
  <div class="modal">
    <h3>{{ t('nodes.modals.delete.moveTitle') }}</h3>
    <p>{{ t('nodes.directory.moveMessage', { name: nodeName, target: targetName }) }}</p>
    <label class="dont-ask">
      <AppToggle v-model="dontAsk" />
      <span>{{ t('nodes.directory.dontAskAgain') }}</span>
    </label>
    <div class="footer">
      <AppButton type="secondary" @click="emit('close')">{{ t('common.actions.cancel') }}</AppButton>
      <AppButton type="primary" @click="confirm">{{ t('common.actions.confirm') }}</AppButton>
    </div>
  </div>
</template>

<script setup lang="ts">
const props = defineProps<{ nodeName: string; targetName: string; onConfirm: () => void }>();
const emit = defineEmits(['close']);

const preferences = usePreferencesStore();
const { t } = useI18nT();

const dontAsk = ref(false);

const confirm = () => {
  if (dontAsk.value) preferences.set('directoryConfirmMove', false);
  emit('close');
  props.onConfirm();
};
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

  p {
    margin: 0;
    color: var(--text-secondary);
  }
}

.dont-ask {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 0.85rem;
  color: var(--text-secondary);
  cursor: pointer;
}

.footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
