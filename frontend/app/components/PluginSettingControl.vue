<template>
  <label class="field">
    <span class="label">{{ field.label }}</span>
    <textarea v-if="field.type === 'textarea'" :value="text" :placeholder="field.placeholder" :rows="field.rows ?? 6" @input="emitText" />
    <select v-else-if="field.type === 'select'" :value="text" @change="emitText">
      <option value="">{{ field.placeholder || '—' }}</option>
      <option v-for="choice in field.choices ?? []" :key="choice.value" :value="choice.value">{{ choice.label }}</option>
    </select>
    <input v-else-if="field.type === 'checkbox'" type="checkbox" :checked="checked" @change="emitChecked" />
    <input v-else type="text" :value="text" :placeholder="field.placeholder" @input="emitText" />
    <span v-if="field.hint" class="hint">{{ field.hint }}</span>
  </label>
</template>

<script setup lang="ts">
import type { PluginSettingField } from '~/types/plugin';

const props = defineProps<{ field: PluginSettingField; modelValue: string | boolean | undefined }>();
const emit = defineEmits<{ 'update:modelValue': [value: string | boolean] }>();

const text = computed(() => (typeof props.modelValue === 'string' ? props.modelValue : ''));
const checked = computed(() => props.modelValue === true);

function emitText(event: Event) {
  const target = event.target as HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement;
  emit('update:modelValue', target.value);
}

function emitChecked(event: Event) {
  emit('update:modelValue', (event.target as HTMLInputElement).checked);
}
</script>

<style scoped lang="scss">
.field {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  margin-bottom: 0.85rem;

  .label {
    font-size: 0.85rem;
    font-weight: 600;
  }

  input[type='text'],
  textarea,
  select {
    padding: 0.45rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font: inherit;
    color: inherit;
    background: var(--surface-raised);
  }

  textarea {
    font-family: var(--font-mono, monospace);
    font-size: 0.85rem;
    resize: vertical;
  }
}

.hint {
  font-size: 0.8rem;
  color: var(--text-secondary);
}
</style>
