<template>
  <div v-if="pending" class="plugin-modal-mask" @click.self="cancel">
    <div class="plugin-modal" role="dialog" :aria-label="pending.spec.title">
      <h3>{{ pending.spec.title }}</h3>
      <p v-if="pending.spec.description" class="description">{{ pending.spec.description }}</p>

      <form @submit.prevent="submit">
        <label v-for="field in pending.spec.fields" :key="field.key" class="field">
          <span class="label">{{ field.label }}</span>
          <textarea
            v-if="field.type === 'textarea'"
            :value="asText(field.key)"
            :placeholder="field.placeholder"
            :required="field.required"
            rows="6"
            @input="setText(field.key, $event)"
          />
          <select v-else-if="field.type === 'select'" :value="asText(field.key)" :required="field.required" @change="setText(field.key, $event)">
            <option value="">{{ field.placeholder || '—' }}</option>
            <option v-for="choice in field.choices ?? []" :key="choice.value" :value="choice.value">{{ choice.label }}</option>
          </select>
          <input
            v-else-if="field.type === 'checkbox'"
            type="checkbox"
            :checked="values[field.key] === true"
            @change="setChecked(field.key, $event)"
          />
          <input
            v-else
            type="text"
            :value="asText(field.key)"
            :placeholder="field.placeholder"
            :required="field.required"
            @input="setText(field.key, $event)"
          />
        </label>

        <div class="footer">
          <AppButton type="secondary" @click="cancel">{{ pending.spec.cancelLabel || 'Cancel' }}</AppButton>
          <AppButton type="primary" @click="submit">{{ pending.spec.submitLabel || 'Save' }}</AppButton>
        </div>
      </form>
    </div>
  </div>
</template>

<script setup lang="ts">
import { pluginModalState, settlePluginModal } from '~/services/plugins/modal';

const pending = computed(() => pluginModalState.current);
// Field values are bound directly by v-model (string or boolean depending on
// the field type); `submit` narrows what it hands back.
const values = reactive<Record<string, string | boolean>>({});

watch(
  () => pluginModalState.current?.id,
  () => {
    for (const key of Object.keys(values)) {
      values[key] = '';
    }
    for (const field of pluginModalState.current?.spec.fields ?? []) {
      values[field.key] = field.value ?? (field.type === 'checkbox' ? false : '');
    }
  },
  { immediate: true },
);

/** Field value as text (checkboxes keep booleans in `values`). */
function asText(key: string): string {
  const value = values[key];
  return typeof value === 'string' ? value : '';
}

function setText(key: string, event: Event) {
  const target = event.target as HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement;
  values[key] = target.value;
}

function setChecked(key: string, event: Event) {
  values[key] = (event.target as HTMLInputElement).checked;
}

function cancel() {
  settlePluginModal(null);
}

function submit() {
  const spec = pluginModalState.current?.spec;
  if (!spec) return;
  const out: Record<string, string | boolean> = {};
  for (const field of spec.fields) {
    const value = values[field.key];
    if (field.required && (value === '' || value === undefined)) return;
    out[field.key] = value ?? '';
  }
  settlePluginModal(out);
}
</script>

<style scoped lang="scss">
.plugin-modal-mask {
  position: fixed;
  z-index: 400;
  display: flex;
  justify-content: center;
  align-items: center;
  background: rgb(0 0 0 / 45%);
  inset: 0;
}

.plugin-modal {
  width: min(560px, 92vw);
  max-height: 86vh;
  padding: 1.25rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface-base);
  overflow-y: auto;

  h3 {
    margin-bottom: 0.35rem;
  }
}

.description {
  margin-bottom: 0.75rem;
  font-size: 0.9rem;
  color: var(--text-secondary);
}

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
}

.footer {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  margin-top: 1rem;
}
</style>
