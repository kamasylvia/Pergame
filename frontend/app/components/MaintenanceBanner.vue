<template>
  <div v-if="readOnly" class="maint-banner" role="status">
    <Icon name="clock" :size="14" />
    <span>{{ t('common.maintenance.readOnly') }}</span>
  </div>
</template>

<script setup lang="ts">
/**
 * Workspace read-only banner (spec backup-restore.md §14): the backend locks
 * every workspace write with 423 while a backup or restore runs, so the editor
 * tells the user instead of letting them lose an edit.
 */
const { t } = useI18nT();
const userStore = useUserStore();
const readOnly = ref(false);
let timer: ReturnType<typeof setInterval> | null = null;

async function poll() {
  // Admin-only endpoint. Polling it while anonymous would earn a 401 on every
  // tick, so stay silent unless an admin session is present; non-admins keep
  // the banner hidden because they cannot be told about instance maintenance
  // anyway.
  if (userStore.user?.role !== 2) {
    readOnly.value = false;
    return;
  }
  const res = await makeRequest<{ read_only: boolean }>('admin/dr-backup/status', 'GET', {});
  readOnly.value = res.status === 'success' ? Boolean(res.result?.read_only) : false;
}

onMounted(() => {
  poll();
  timer = setInterval(poll, 4000);
});

onUnmounted(() => {
  if (timer) clearInterval(timer);
});
</script>

<style scoped lang="scss">
.maint-banner {
  position: fixed;
  bottom: 16px;
  left: 50%;
  z-index: 9000;
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.5rem 0.85rem;
  border-radius: var(--radius-sm);
  font-size: 0.78rem;
  color: var(--orange);
  background: var(--orange-bg);
  box-shadow: 0 4px 14px rgb(0 0 0 / 12%);
  transform: translateX(-50%);
}
</style>
