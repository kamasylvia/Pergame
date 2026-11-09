<template>
  <div>
    <h2 class="page-title">{{ t('settings.instanceDR.title') }}</h2>
    <p class="page-subtitle">{{ t('settings.instanceDR.subtitle') }}</p>

    <div class="main">
      <!-- dr-backup.conf editor -->
      <div class="options-section">
        <h3>{{ t('settings.instanceDR.confTitle') }}</h3>
        <p v-if="deployError" class="error-message">{{ deployError }}</p>
        <template v-else>
          <p v-if="!configExists" class="hint">{{ t('settings.instanceDR.confMissing') }}</p>
          <textarea
            v-model="confContent"
            class="conf-editor"
            rows="14"
            spellcheck="false"
            :placeholder="t('settings.instanceDR.confMissing')"
          ></textarea>
          <p class="hint">{{ t('settings.instanceDR.maskedNote') }}</p>
          <div class="actions-row">
            <AppButton type="primary" :disabled="savingConf" @click="saveConfig">
              <Icon name="save" display="sm" />
              {{ t('settings.instanceDR.save') }}
            </AppButton>
          </div>
        </template>
      </div>

      <!-- Job actions -->
      <div class="options-section">
        <h3>{{ t('settings.instanceDR.actionsTitle') }}</h3>
        <p class="hint">{{ t('settings.instanceDR.cronHint') }}</p>
        <div class="actions-row">
          <AppButton type="primary" :disabled="jobRunning" @click="createJob('backup')">
            <Icon name="backup" display="sm" />
            {{ t('settings.instanceDR.backupNow') }}
          </AppButton>
          <AppButton type="secondary" :disabled="jobRunning" @click="createJob('list')">
            <Icon name="view" display="sm" />
            {{ t('settings.instanceDR.listSnapshots') }}
          </AppButton>
        </div>

        <!-- Restore: two-step inline confirm, same pattern as the snapshots list -->
        <div class="restore-box">
          <div class="restore-fields">
            <input v-model="restoreSource" class="restore-input" type="text" :placeholder="t('settings.instanceDR.source')" />
            <input v-model="restoreTs" class="restore-input" type="text" :placeholder="t('settings.instanceDR.timestampOptional')" />
            <AppButton
              type="danger"
              :class="{ 'confirm-restore': restoreArmed }"
              :disabled="jobRunning"
              @click="restoreClick"
            >
              {{ restoreArmed ? t('settings.instanceDR.confirmRestore') : t('settings.instanceDR.restore') }}
            </AppButton>
          </div>
          <p v-if="restoreArmed" class="restore-warning">{{ t('settings.instanceDR.restoreWarning') }}</p>
        </div>

        <LoaderSpinner v-if="jobRunning" />
        <pre v-if="jobOutput" class="job-output">{{ jobOutput }}</pre>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
const { t } = useI18nT();
const notifications = useNotifications();

const confExists = ref(true);
const confContent = ref('');
const savingConf = ref(false);
const deployError = ref('');

const jobRunning = ref(false);
const jobOutput = ref('');

const restoreSource = ref('');
const restoreTs = ref('');
const restoreArmed = ref(false);
let restoreTimer: ReturnType<typeof setTimeout> | null = null;
let pollTimer: ReturnType<typeof setInterval> | null = null;

onMounted(loadConfig);

onUnmounted(() => {
  if (pollTimer) clearInterval(pollTimer);
  if (restoreTimer) clearTimeout(restoreTimer);
});

async function loadConfig() {
  const result = await makeRequest<{ exists: boolean; content: string }>('admin/dr-backup/config', 'GET', {});
  if (result.status === 'success' && result.result) {
    deployError.value = '';
    confExists.value = result.result.exists;
    confContent.value = result.result.content ?? '';
  } else {
    deployError.value = result.message || t('settings.instanceDR.notDeployed');
  }
}

async function saveConfig() {
  savingConf.value = true;
  try {
    const result = await makeRequest('admin/dr-backup/config', 'PUT', { content: confContent.value });
    if (result.status === 'success') {
      notifications.add({ type: 'success', title: t('settings.instanceDR.saved') });
      await loadConfig();
    } else {
      notifications.add({ type: 'error', title: t('settings.instanceDR.saveFailed'), message: result.message });
    }
  } finally {
    savingConf.value = false;
  }
}

async function createJob(action: 'backup' | 'list' | 'restore') {
  jobRunning.value = true;
  jobOutput.value = '';
  try {
    const body: Record<string, unknown> = { action };
    if (action === 'restore') {
      body.source = restoreSource.value.trim();
      if (restoreTs.value.trim()) body.ts = restoreTs.value.trim();
    }
    const result = await makeRequest<{ job_id: string }>('admin/dr-backup/jobs', 'POST', body);
    if (result.status === 'success' && result.result?.job_id) {
      pollJob(result.result.job_id, action);
    } else {
      notifications.add({ type: 'error', title: t('common.errors.generic'), message: result.message });
      jobRunning.value = false;
    }
  } catch (e) {
    notifications.add({ type: 'error', title: t('common.errors.generic'), message: String(e) });
    jobRunning.value = false;
  }
}

async function pollJob(jobId: string, action: string) {
  if (pollTimer) clearInterval(pollTimer);
  let ticks = 0;
  pollTimer = setInterval(async () => {
    ticks += 1;
    const result = await makeRequest<{ status: string; exit?: number; output_tail?: string; message?: string }>(
      `admin/dr-backup/jobs/${jobId}`,
      'GET',
      {},
    );
    if (result.status !== 'success') {
      if (pollTimer) clearInterval(pollTimer);
      jobRunning.value = false;
      notifications.add({ type: 'error', title: t('common.errors.generic'), message: result.message });
      return;
    }
    const job = result.result;
    if (job.status === 'done') {
      if (pollTimer) clearInterval(pollTimer);
      jobRunning.value = false;
      jobOutput.value = `[exit ${job.exit ?? '?'}] ${action}\n\n${job.output_tail ?? ''}`;
      notifications.add({
        type: job.exit === 0 ? 'success' : 'error',
        title: t('settings.instanceDR.jobDone'),
        message: t('settings.instanceDR.jobDoneDetail', { action, exit: job.exit ?? '?' }),
      });
    } else if (ticks > 300) {
      if (pollTimer) clearInterval(pollTimer);
      jobRunning.value = false;
      notifications.add({ type: 'error', title: t('common.errors.generic'), message: 'job timeout' });
    }
  }, 2000);
}

function restoreClick() {
  if (!restoreSource.value.trim()) {
    notifications.add({ type: 'error', title: t('settings.instanceDR.restoreFailed'), message: t('settings.instanceDR.source') });
    return;
  }
  // Two-step inline confirm: this restore replaces an entire database.
  if (!restoreArmed.value) {
    restoreArmed.value = true;
    restoreTimer = setTimeout(() => (restoreArmed.value = false), 8000);
    return;
  }
  if (restoreTimer) clearTimeout(restoreTimer);
  restoreArmed.value = false;
  createJob('restore');
}
</script>

<style scoped lang="scss">
.main {
  max-width: 720px;
  margin: auto;
}

.options-section {
  margin-bottom: 1.5rem;

  h3 {
    margin-bottom: 0.75rem;
    font-size: 1rem;
    color: var(--text-body);
  }
}

.conf-editor {
  width: 100%;
  padding: 0.75rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  font-family: var(--font-mono, monospace);
  font-size: 0.8rem;
  color: var(--text-body);
  background: var(--surface);
  resize: vertical;
}

.hint {
  margin: 0.5rem 0;
  font-size: 0.75rem;
  color: var(--text-secondary);
}

.error-message {
  padding: 0.75rem;
  border-radius: var(--radius-sm);
  color: var(--error);
  background: var(--error-bg);
}

.actions-row {
  display: flex;
  gap: 0.5rem;
  margin-top: 0.75rem;
}

.restore-box {
  margin-top: 1.25rem;
}

.restore-fields {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.5rem;
}

.restore-input {
  flex: 1;
  min-width: 160px;
  padding: 0.5rem 0.75rem;
  font-size: 0.875rem;
}

.restore-warning {
  margin-top: 0.5rem;
  padding: 0.5rem;
  border-radius: var(--radius-sm);
  font-size: 0.75rem;
  color: var(--orange);
  background: var(--orange-bg);
}

.job-output {
  max-height: 320px;
  margin-top: 1rem;
  padding: 0.75rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  font-family: var(--font-mono, monospace);
  font-size: 0.75rem;
  color: var(--text-body);
  text-align: left;
  white-space: pre-wrap;
  background: var(--surface);
  overflow: auto;
}
</style>
