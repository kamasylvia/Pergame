<template>
  <div>
    <h2 class="page-title">{{ t('settings.plugins.title') }}</h2>
    <p class="page-subtitle">{{ t('settings.plugins.subtitle') }}</p>

    <section v-if="isAdmin" class="panel">
      <h3>{{ t('settings.plugins.install') }}</h3>
      <p class="hint">{{ t('settings.plugins.installHint') }}</p>
      <div class="install-row">
        <input ref="fileInput" type="file" accept=".zip" class="hidden" @change="onPick" />
        <AppButton type="secondary" :disabled="installing" @click="fileInput?.click()">
          {{ installing ? t('settings.plugins.installing') : t('settings.plugins.pickArchive') }}
        </AppButton>
        <span v-if="installMessage" :class="['message', installFailed ? 'error' : 'ok']">{{ installMessage }}</span>
      </div>
    </section>

    <section class="panel">
      <h3>{{ t('settings.plugins.installed') }}</h3>
      <p v-if="!plugins.length" class="hint">{{ t('settings.plugins.empty') }}</p>
      <div v-for="plugin in plugins" :key="plugin.id" class="plugin-row">
        <header>
          <div>
            <strong>{{ plugin.name }}</strong>
            <span class="version">v{{ plugin.version }} · {{ plugin.id }}</span>
          </div>
          <div class="row-actions">
            <AppToggle v-if="isAdmin" :model-value="plugin.enabled" @update:model-value="toggleEnabled(plugin, $event)" />
            <AppButton v-if="isAdmin" type="danger" @click="remove(plugin)">{{ t('common.actions.delete') }}</AppButton>
          </div>
        </header>
        <p v-if="plugin.manifest.description" class="description">{{ plugin.manifest.description }}</p>

        <div class="caps">
          <span class="caps-title">{{ t('settings.plugins.capabilities') }}</span>
          <label v-for="cap in plugin.capabilities" :key="cap" class="cap">
            <AppCheck :model-value="plugin.granted.includes(cap)" @update:model-value="toggleCapability(plugin, cap, $event)" />
            <span>{{ cap }}</span>
          </label>
          <span v-if="!plugin.capabilities.length" class="hint">{{ t('settings.plugins.noCapabilities') }}</span>
        </div>
        <p v-if="plugin.enabled && !plugin.granted.length" class="hint warn">{{ t('settings.plugins.notApproved') }}</p>
        <p v-else-if="!plugin.enabled" class="hint warn">{{ t('settings.plugins.disabled') }}</p>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import type { PluginManifest } from '~/types/plugin';

interface AdminPluginRow {
  id: string;
  name: string;
  version: string;
  enabled: boolean;
  capabilities: string[];
  granted: string[];
  manifest: PluginManifest;
}

const { t } = useI18nT();
const userStore = useUserStore();
const notifications = useNotifications();
const nuxtApp = useNuxtApp();

const isAdmin = computed(() => userStore.user?.role === 2);
const plugins = ref<AdminPluginRow[]>([]);
const fileInput = ref<HTMLInputElement>();
const installing = ref(false);
const installMessage = ref('');
const installFailed = ref(false);

/** Admin view needs disabled plugins too; fall back to the public list. */
async function loadPlugins() {
  const response = isAdmin.value
    ? await makeRequest<AdminPluginRow[]>('admin/plugins', 'GET')
    : await makeRequest<AdminPluginRow[]>('plugins', 'GET');
  if (response.status === 'success' && response.result) {
    plugins.value = response.result.map(row => ({
      ...row,
      enabled: row.enabled ?? true,
      capabilities: row.capabilities ?? [],
      granted: row.granted ?? [],
    }));
  }
}

onMounted(loadPlugins);

function report(title: string, message: string | undefined, type: 'success' | 'error') {
  notifications.add({ title, message, type });
}

async function onPick(event: Event) {
  const file = (event.target as HTMLInputElement).files?.[0];
  if (!file) return;
  installing.value = true;
  installMessage.value = '';
  try {
    const body = new FormData();
    body.append('file', file);
    const response = await makeRequest<{ id: string; version: string }>('admin/plugins', 'POST', body);
    if (response.status !== 'success' || !response.result) throw new Error(response.message || 'install failed');
    installMessage.value = t('settings.plugins.installedOne', { name: response.result.id, version: response.result.version });
    installFailed.value = false;
    await loadPlugins();
    report(t('settings.plugins.install'), installMessage.value, 'success');
  } catch (err) {
    installFailed.value = true;
    installMessage.value = err instanceof Error ? err.message : String(err);
    report(t('settings.plugins.install'), installMessage.value, 'error');
  } finally {
    installing.value = false;
    if (fileInput.value) fileInput.value.value = '';
  }
}

async function toggleEnabled(plugin: AdminPluginRow, enabled: boolean) {
  const response = await makeRequest(`admin/plugins/${plugin.id}`, 'PATCH', { enabled });
  if (response.status === 'success') {
    plugin.enabled = enabled;
    if (!enabled) await unloadHosted(plugin.id);
  } else {
    report(t('settings.plugins.updateFailed'), response.message, 'error');
  }
}

async function toggleCapability(plugin: AdminPluginRow, cap: string, granted: boolean) {
  if (!plugin.enabled) return;
  const next = granted ? [...plugin.granted, cap] : plugin.granted.filter(c => c !== cap);
  const response = await makeRequest<{ granted: string[] }>(`plugins/${plugin.id}/grants`, 'PUT', { capabilities: next });
  if (response.status !== 'success' || !response.result) {
    report(t('settings.plugins.updateFailed'), response.message, 'error');
    return;
  }
  plugin.granted = response.result.granted;
  // Grants change what the plugin may do: reload it so the running instance
  // sees the new capability set instead of the old one.
  await unloadHosted(plugin.id);
  if (plugin.granted.length) await reloadHosted(plugin);
}

async function remove(plugin: AdminPluginRow) {
  const response = await makeRequest(`admin/plugins/${plugin.id}`, 'DELETE');
  if (response.status !== 'success') {
    report(t('settings.plugins.updateFailed'), response.message, 'error');
    return;
  }
  await unloadHosted(plugin.id);
  await loadPlugins();
}

type Host = { unload(id: string): Promise<void>; load(manifest: PluginManifest, entryUrl: string, granted?: string[]): Promise<unknown> };
const host = () => nuxtApp.$pluginHost as Host | undefined;

async function unloadHosted(id: string) {
  await host()?.unload(id);
}

async function reloadHosted(plugin: AdminPluginRow) {
  await host()?.load(plugin.manifest, `/api/plugins/${plugin.id}/entry`, plugin.granted);
}
</script>

<style scoped lang="scss">
.panel {
  padding: 1rem 0;
  border-bottom: 1px solid var(--border);

  h3 {
    margin-bottom: 0.5rem;
  }
}

.hint {
  margin: 0.25rem 0 0.75rem;
  font-size: 0.9rem;
  color: var(--text-secondary);

  &.warn {
    color: var(--orange-dark, var(--text-secondary));
  }
}

.install-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.hidden {
  display: none;
}

.message {
  font-size: 0.9rem;

  &.error {
    color: var(--red-dark);
  }

  &.ok {
    color: var(--green-dark, var(--text-secondary));
  }
}

.plugin-row {
  padding: 0.75rem 0;
  border-top: 1px solid var(--border);

  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
  }
}

.version {
  margin-left: 0.5rem;
  font-size: 0.85rem;
  color: var(--text-secondary);
}

.row-actions {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.description {
  margin: 0.35rem 0;
  font-size: 0.9rem;
  color: var(--text-secondary);
}

.caps {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.5rem 1rem;
  margin-top: 0.5rem;
}

.caps-title {
  font-size: 0.85rem;
  font-weight: 600;
}

.cap {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  font-family: $font-mono;
  font-size: 0.85rem;
}
</style>
