<template>
  <div v-if="open" class="plugin-settings-mask" @click.self="close">
    <div class="plugin-settings" role="dialog" :aria-label="plugin.name">
      <header>
        <h3>{{ plugin.name }}</h3>
        <span class="version">v{{ plugin.version }}</span>
      </header>
      <p v-if="plugin.manifest.description" class="description">{{ plugin.manifest.description }}</p>

      <p v-if="loading" class="hint">{{ t('settings.plugins.settingsLoading') }}</p>
      <template v-else>
        <PluginSettingControl
          v-for="field in ungrouped"
          :key="field.key"
          :field="field"
          v-model="values[field.key]"
        />

        <details v-for="group in groups" :key="group.name" class="group">
          <summary>{{ group.name }}</summary>
          <PluginSettingControl
            v-for="field in group.fields"
            :key="field.key"
            :field="field"
            v-model="values[field.key]"
          />
        </details>

        <p v-if="!fields.length" class="hint">{{ t('settings.plugins.noSettings') }}</p>
      </template>

      <div class="footer">
        <AppButton type="secondary" @click="close">{{ t('common.actions.close') }}</AppButton>
        <AppButton v-if="fields.length" type="primary" :disabled="saving" @click="save">
          {{ t('common.actions.save') }}
        </AppButton>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { PluginManifest, PluginSettingField } from '~/types/plugin';

const props = defineProps<{ plugin: { id: string; name: string; version: string; manifest: PluginManifest } }>();
const open = defineModel<boolean>('open', { default: false });

const { t } = useI18nT();
const notifications = useNotifications();

/** Dynamic select options loaded for fields that declare `choicesFrom`. */
const loadedChoices = ref<Record<string, Array<{ value: string; label: string }>>>({});
/** The manifest's fields with dynamic choices folded in. */
const fields = computed<PluginSettingField[]>(() =>
  (props.plugin.manifest.settings ?? []).map(field =>
    field.choicesFrom && loadedChoices.value[field.key]
      ? { ...field, choices: loadedChoices.value[field.key] }
      : field,
  ),
);
/** Fields without a `group` render above the collapsible sections. */
const ungrouped = computed(() => fields.value.filter(f => !f.group));
/** Collapsible sections in declaration order. */
const groups = computed(() => {
  const out: Array<{ name: string; fields: PluginSettingField[] }> = [];
  for (const field of fields.value) {
    if (!field.group) continue;
    const existing = out.find(g => g.name === field.group);
    if (existing) existing.fields.push(field);
    else out.push({ name: field.group, fields: [field] });
  }
  return out;
});

const values = reactive<Record<string, string | boolean>>({});
const loading = ref(false);
const saving = ref(false);

// Values live in the plugin's own KV namespace, keyed by field.key — the same
// keys the plugin reads with ctx.settings.get().
watch(open, async isOpen => {
  if (!isOpen) return;
  loading.value = true;
  try {
    // Container-backed selects (workspace / folder pickers) read the user's
    // own nodes: one flat query, so this stays cheap even on 1000+ nodes.
    const dynamic = (props.plugin.manifest.settings ?? []).filter(f => f.choicesFrom);
    if (dynamic.length) {
      const response = await makeRequest<Array<{ id: string; name: string; role: number }>>('nodes/containers', 'GET');
      const rows = response.status === 'success' && response.result ? response.result : [];
      for (const field of dynamic) {
        const roles = field.choicesFrom === 'workspaces' ? [1] : field.choicesFrom === 'folders' ? [2] : [1, 2];
        loadedChoices.value[field.key] = rows
          .filter(row => roles.includes(row.role))
          .map(row => ({ value: row.id, label: row.name }));
      }
    }
    const response = await makeRequest<Record<string, unknown>>(`plugins/${props.plugin.id}/kv`, 'GET');
    const stored = response.status === 'success' && response.result ? response.result : {};
    for (const field of fields.value) {
      const fallback = field.type === 'checkbox' ? false : '';
      values[field.key] = (stored[field.key] as string | boolean | undefined) ?? field.value ?? fallback;
    }
  } finally {
    loading.value = false;
  }
});

async function save() {
  saving.value = true;
  try {
    for (const field of fields.value) {
      const response = await makeRequest(`plugins/${props.plugin.id}/kv/${encodeURIComponent(field.key)}`, 'PUT', values[field.key] as unknown as object);
      if (response.status !== 'success') throw new Error(response.message || 'save failed');
    }
    notifications.add({ title: props.plugin.name, message: t('settings.plugins.settingsSaved'), type: 'success' });
    open.value = false;
  } catch (err) {
    notifications.add({
      title: props.plugin.name,
      message: err instanceof Error ? err.message : String(err),
      type: 'error',
    });
  } finally {
    saving.value = false;
  }
}

function close() {
  open.value = false;
}
</script>

<style scoped lang="scss">
.plugin-settings-mask {
  position: fixed;
  z-index: 400;
  display: flex;
  justify-content: center;
  align-items: center;
  background: rgb(0 0 0 / 45%);
  inset: 0;
}

.plugin-settings {
  width: min(640px, 92vw);
  max-height: 86vh;
  padding: 1.25rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface-base);
  overflow-y: auto;

  header {
    display: flex;
    gap: 0.5rem;
    align-items: baseline;
    margin-bottom: 0.35rem;
  }

  .version {
    font-size: 0.8rem;
    color: var(--text-secondary);
  }
}

.description {
  margin-bottom: 1rem;
  font-size: 0.9rem;
  color: var(--text-secondary);
}

.group {
  margin-bottom: 0.5rem;
  border-top: 1px solid var(--border);

  summary {
    padding: 0.6rem 0;
    font-size: 0.9rem;
    font-weight: 600;
    cursor: pointer;
  }

  &:not([open]) {
    padding-bottom: 0.2rem;
  }
}

.hint {
  font-size: 0.8rem;
  color: var(--text-secondary);
}

.footer {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  margin-top: 1rem;
}
</style>
