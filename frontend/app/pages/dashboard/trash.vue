<template>
  <div class="page-card">
    <Teleport to="#navbar-title"><Icon name="archive" display="lg" />{{ t('trash.title') }}</Teleport>

    <Teleport to="#navbar-actions">
      <AppButton :disabled="loading" type="secondary" @click="reload">{{ t('trash.refresh') }}</AppButton>
      <AppButton v-if="entries.length" :disabled="busy" type="danger" @click="empty()">
        {{ t('trash.emptyTrash') }}
      </AppButton>
    </Teleport>

    <p class="subtitle">{{ t('trash.subtitle', { days: retentionDays }) }}</p>

    <LoaderSpinner v-if="loading" />

    <div v-else-if="!entries.length" class="empty">{{ t('trash.empty') }}</div>

    <div v-else class="list">
      <div v-for="entry in entries" :key="entry.id" class="row">
        <Icon :name="iconFor(entry.role)" display="lg" />
        <div class="info">
          <div class="name">{{ entry.name }}</div>
          <div class="meta">
            <span v-if="entry.batch_size > 1">{{ t('trash.children', { count: entry.batch_size - 1 }) }} · </span>
            <span v-if="entry.size">{{ formatSize(entry.size) }} · </span>
            <span>{{ t('trash.deletedAt', { date: formatDate(entry.deleted_at) }) }}</span>
          </div>
        </div>
        <div class="actions">
          <AppButton :disabled="busy" small type="primary" @click="restore(entry)">{{ t('trash.restore') }}</AppButton>
          <AppButton :disabled="busy" small type="danger" @click="purge(entry)">{{ t('trash.deleteForever') }}</AppButton>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { TrashEntry } from '~/stores';

const nodesStore = useNodesStore();
const { t } = useI18nT();
const notifications = useNotifications();

const entries = ref<TrashEntry[]>([]);
const loading = ref(false);
const busy = ref(false);
// Mirrors the backend default (TRASH_RETENTION_DAYS); display only.
const retentionDays = 30;

const iconsByRole: Record<number, string> = {
  0: 'dashboard',
  1: 'categories',
  2: 'categories',
  3: 'file',
  4: 'image',
};
const iconFor = (role: number) => iconsByRole[role] ?? 'file';

const formatDate = (ms: number) =>
  new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(ms));

function formatSize(bytes: number) {
  const units = ['B', 'KB', 'MB', 'GB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${value.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`;
}

async function reload() {
  loading.value = true;
  try {
    entries.value = await nodesStore.fetchTrash();
  } finally {
    loading.value = false;
  }
}

async function restore(entry: TrashEntry) {
  busy.value = true;
  try {
    const result = await nodesStore.restoreFromTrash(entry.id);
    if (result.restored > 0) {
      notifications.add({ type: 'success', title: t('trash.restored', { name: entry.name }) });
      if (result.reparented) notifications.add({ type: 'info', title: t('trash.reparented') });
      await reload();
    } else {
      notifications.add({ type: 'error', title: t('trash.restoreFailed') });
    }
  } finally {
    busy.value = false;
  }
}

/** Permanent deletion — irreversible (S3 objects go with it), so confirm first. */
async function purge(entry: TrashEntry) {
  const ok = confirm(t('trash.deleteForeverConfirm', { name: entry.name }));
  if (!ok) return;
  busy.value = true;
  try {
    const purged = await nodesStore.purgeFromTrash(entry.id);
    if (purged > 0) notifications.add({ type: 'success', title: t('trash.purged', { name: entry.name }) });
    await reload();
  } finally {
    busy.value = false;
  }
}

async function empty() {
  const ok = confirm(t('trash.emptyConfirm'));
  if (!ok) return;
  busy.value = true;
  try {
    const purged = await nodesStore.emptyTrash();
    notifications.add({ type: 'success', title: t('trash.emptied', { count: purged }) });
    await reload();
  } finally {
    busy.value = false;
  }
}

onMounted(reload);
</script>

<style scoped>
.subtitle {
  font-size: 13px;
  color: var(--text-secondary);
}

.list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 12px;
}

.row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
}

.info {
  flex: 1;
  min-width: 0;
}

.name {
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
  overflow: hidden;
}

.meta {
  font-size: 12px;
  color: var(--text-secondary);
}

.actions {
  display: flex;
  flex-shrink: 0;
  gap: 8px;
}

.empty {
  padding: 40px 0;
  font-style: italic;
  color: var(--text-secondary);
  text-align: center;
}
</style>
