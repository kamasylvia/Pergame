<template>
  <div>
    <h2 class="page-title">{{ t('settings.backups.title') }}</h2>
    <p class="page-subtitle">{{ t('settings.backups.subtitle') }}</p>

    <div class="main">
      <!-- No-targets hint: snapshots package for download only -->
      <p v-if="targetsChecked && targetCount === 0" class="hint no-targets">
        {{ t('settings.backups.noTargetsHint') }}
        <AppButton type="secondary" small @click="goToTargets">
          {{ t('settings.backups.goToTargets') }}
        </AppButton>
      </p>
      <!-- Backup Options -->
      <div class="options-section">
        <h3>{{ t('settings.backups.options') }}</h3>
        <div class="options-grid">
          <input
            v-model="options.memo"
            class="memo-input"
            type="text"
            :placeholder="t('settings.backups.memoPlaceholder')"
            maxlength="200"
          />
          <AppCheck v-model="options.includeDocuments">
            <span class="option-label">
              <Icon name="folder" display="sm" />
              {{ t('settings.backups.includeDocuments') }}
            </span>
          </AppCheck>
          <AppCheck v-model="options.includeFiles">
            <span class="option-label">
              <Icon name="image" display="sm" />
              {{ t('settings.backups.includeFiles') }}
            </span>
          </AppCheck>
          <AppCheck v-model="options.includeSettings">
            <span class="option-label">
              <Icon name="settings" display="sm" />
              {{ t('settings.backups.includeSettings') }}
            </span>
          </AppCheck>
          <AppCheck v-model="options.includeMetadata">
            <span class="option-label">
              <Icon name="layers" display="sm" />
              {{ t('settings.backups.includeMetadata') }}
            </span>
          </AppCheck>
          <label class="passphrase-row">
            <span>{{ t('settings.backups.passphraseLabel') }}</span>
            <input v-model="options.passphrase" class="memo-input" type="password" autocomplete="new-password" />
            <small class="hint">{{ t('settings.backups.passphraseHint') }}</small>
          </label>
          <label v-if="targetCount === 0" class="passphrase-row">
            <span>{{ t('settings.backups.expiresInLabel') }}</span>
            <AppSelect v-model="options.expiresIn" :items="EXPIRY_OPTIONS" :searchable="false" />
            <small class="hint">{{ t('settings.backups.expiresInHint') }}</small>
          </label>
        </div>
      </div>

      <!-- Start Backup Button -->
      <AppButton type="primary" :disabled="isProcessing || !hasAnyOption" @click="startBackup">
        <Icon v-if="!isProcessing" name="backup" display="sm" />
        {{ isProcessing ? t('settings.backups.creatingBackup') : t('settings.backups.createBackup') }}
      </AppButton>
      <LoaderSpinner v-if="isProcessing" />
      <!-- Progress Section -->
      <div v-if="currentJob" class="progress-section">
        <div v-if="currentJob.status !== 'completed'" class="progress-header">
          <span class="status-badge" :class="currentJob.status">
            {{ statusLabel }}
          </span>
          <span v-if="currentJob.status === 'processing'" class="progress-percent"> {{ currentJob.progress }}% </span>
        </div>

        <div v-if="currentJob.status === 'processing'" class="progress-bar-container">
          <div class="progress-bar" :style="{ width: currentJob.progress + '%' }"></div>
        </div>

        <p class="progress-message">{{ currentJob.message }}</p>

        <!-- Cancel Button -->
        <AppButton v-if="currentJob.status === 'processing' || currentJob.status === 'pending'" type="secondary" @click="cancelBackup">
          <Icon name="close" display="sm" />
          {{ t('common.actions.cancel') }}
        </AppButton>

        <!-- Error Message -->
        <div v-if="currentJob.status === 'failed' && currentJob.error" class="error-message">
          {{ currentJob.error }}
        </div>
      </div>

      <!-- Download Section -->
      <div v-if="currentJob?.status === 'completed' && currentJob.download_url" class="link-section">
        <p class="expiry-notice">
          <Icon name="clock" :size="14" />
          {{ t('settings.backups.expiryNotice', { date: numericDate(currentJob.expires_at || '') }) }}
        </p>
        <input v-model="currentJob.download_url" type="text" readonly :placeholder="t('settings.backups.backupLink')" />
        <div class="actions-row">
          <AppButton type="secondary" @click="copyLink">
            <Icon name="copy" display="sm" />
            {{ t('common.actions.copyLink') }}
          </AppButton>
          <a :href="currentJob.download_url" download>
            <AppButton type="primary">
              <Icon name="download" display="sm" />
              {{ t('settings.backups.downloadBackup') }}
            </AppButton>
          </a>
        </div>
      </div>

      <!-- Snapshots Section -->
      <div class="options-section snapshots-section">
        <div class="snapshots-header">
          <h3>{{ t('settings.backups.snapshots') }}</h3>
          <AppButton type="secondary" small :disabled="loadingSnapshots" @click="loadSnapshots">
            <Icon name="refresh" display="sm" />
          </AppButton>
        </div>
        <p v-if="loadingSnapshots" class="snapshot-empty">
          <LoaderSpinner />
        </p>
        <p v-else-if="snapshots.length === 0" class="snapshot-empty">
          {{ targetCount === 0 ? t('settings.backups.snapshotsEmptyNoTargets') : t('settings.backups.snapshotsEmpty') }}
        </p>
        <ul v-else class="snapshot-list">
          <li v-for="snap in snapshots" :key="snap.id" class="snapshot-row">
            <Icon name="backup" display="sm" />
            <span class="snapshot-date">
              {{ numericDate(snap.last_modified) }}
              <small v-if="snap.memo" class="snapshot-memo">{{ snap.memo }}</small>
              <small v-if="snap.encrypted" class="snapshot-encrypted" :title="t('settings.backups.encryptedBadge')">🔒</small>
            </span>
            <span class="snapshot-size">{{ fmtSize(snap.size) }}</span>
            <span class="snapshot-actions">
              <button class="snapshot-btn" :title="t('settings.backups.snapshotDownload')" @click="downloadSnapshot(snap.id)">
                <Icon name="download" display="sm" />
              </button>
              <button
                class="snapshot-btn"
                :class="{ 'confirm-restore': pendingRestore === snap.id }"
                :title="t('settings.backups.snapshotRestore')"
                @click="restoreSnapshot(snap)"
              >
                <Icon :name="pendingRestore === snap.id ? 'warn' : 'restore'" display="sm" />
              </button>
              <button class="snapshot-btn" :title="t('settings.backups.snapshotDelete')" @click="deleteSnapshot(snap.id)">
                <Icon name="delete" display="sm" />
              </button>
            </span>
          </li>
        </ul>
        <div v-if="pendingRestore" class="restore-warning">
          <p>{{ t('settings.backups.confirmRestore') }}</p>
          <p v-if="pendingPlan" class="restore-plan">
            {{ t('settings.backups.planDetail', {
              incoming: pendingPlan.incoming_nodes,
              current: pendingPlan.current_nodes,
              replaced: pendingPlan.nodes_replaced,
              added: pendingPlan.nodes_added,
              removed: pendingPlan.nodes_removed,
            }) }}
          </p>
          <p v-if="pendingPlan && pendingPlan.files_colliding > 0" class="restore-plan">
            {{ t('settings.backups.planFiles', { count: pendingPlan.files_colliding }) }}
          </p>
          <p v-if="pendingPlan && pendingPlan.incoming_permissions > 0" class="restore-plan">
            {{ t('settings.backups.planPermissions', { count: pendingPlan.incoming_permissions, unresolvable: pendingPlan.permissions_unresolvable }) }}
          </p>
          <p v-if="pendingPlan && pendingPlan.incoming_invitations > 0" class="restore-plan">
            {{ t('settings.backups.planInvitations', { count: pendingPlan.incoming_invitations, conflicting: pendingPlan.invitations_conflicting }) }}
          </p>
          <p v-if="pendingPlan && pendingPlan.incoming_trashed > 0" class="restore-plan">
            {{ t('settings.backups.planTrash', { count: pendingPlan.incoming_trashed, present: pendingPlan.trashed_present }) }}
          </p>
          <p v-if="pendingPlan && pendingPlan.profile_in_archive" class="restore-plan">
            {{ t('settings.backups.planProfile') }}
          </p>
          <label v-if="pendingSnapEncrypted" class="passphrase-row">
            <span>{{ t('settings.backups.restorePassphraseLabel') }}</span>
            <input v-model="restorePassphrase" class="memo-input" type="password" autocomplete="off" />
          </label>
          <label class="force-row">
            <input v-model="restoreForce" type="checkbox" />
            <span>{{ t('settings.backups.forceConfirm') }}</span>
          </label>
        </div>

        <!-- Upload restore -->
        <div class="upload-restore">
          <input ref="uploadInput" type="file" accept=".zip" class="hidden-input" @change="onRestoreFile" />
          <AppButton type="secondary" :disabled="isRestoring" @click="uploadInput?.click()">
            <Icon name="import" display="sm" />
            {{ isRestoring ? t('settings.backups.restoring') : t('settings.backups.uploadRestore') }}
          </AppButton>
          <p class="upload-hint">{{ t('settings.backups.uploadRestoreHint') }}</p>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
interface BackupJob {
  id: string;
  user_id: string;
  status: 'pending' | 'processing' | 'completed' | 'failed';
  options: BackupOptions;
  progress: number;
  message: string;
  download_url?: string;
  error?: string;
  created_at: string;
  updated_at: string;
  expires_at?: string;
}

interface BackupOptions {
  include_documents: boolean;
  include_files: boolean;
  local_data?: object;
  include_settings?: boolean;
  include_metadata: boolean;
}

const options = reactive({
  includeDocuments: true,
  includeFiles: true,
  includeSettings: true,
  includeMetadata: true,
  memo: '',
  passphrase: '',
  expiresIn: '1h',
});

const EXPIRY_OPTIONS = computed(() => [
  { id: '1h', label: t('settings.backups.expiry1h') },
  { id: '1d', label: t('settings.backups.expiry1d') },
  { id: '1w', label: t('settings.backups.expiry1w') },
]);

const { t } = useI18nT();
const { numericDate } = useDateFormatters();
const notifications = useNotifications();

const currentJob = ref<BackupJob | null>(null);
const pollingInterval = ref<ReturnType<typeof setInterval> | null>(null);

interface BackupSnapshot {
  id: string;
  size: number;
  last_modified: number;
  memo?: string;
  encrypted?: boolean;
}

interface RestorePlan {
  incoming_nodes: number;
  current_nodes: number;
  nodes_replaced: number;
  nodes_added: number;
  nodes_removed: number;
  files_colliding: number;
  incoming_permissions: number;
  permissions_unresolvable: number;
  incoming_invitations: number;
  invitations_conflicting: number;
  incoming_trashed: number;
  trashed_present: number;
  profile_in_archive: boolean;
  warnings?: string[];
}

const pendingPlan = ref<RestorePlan | null>(null);

const snapshots = ref<BackupSnapshot[]>([]);
const loadingSnapshots = ref(false);
const pendingRestore = ref<string | null>(null);
const restorePassphrase = ref('');
const restoreForce = ref(false);
const pendingSnapEncrypted = computed(() => snapshots.value.find(s => s.id === pendingRestore.value)?.encrypted === true);
const isRestoring = ref(false);
const targetCount = ref(-1);
const targetsChecked = computed(() => targetCount.value >= 0);
const uploadInput = ref<HTMLInputElement | null>(null);

let restoreConfirmTimer: ReturnType<typeof setTimeout> | null = null;

function fmtSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

async function loadSnapshots() {
  loadingSnapshots.value = true;
  try {
    const result = await makeRequest<BackupSnapshot[]>('backup/snapshots', 'GET', {});
    if (result.status === 'success') {
      snapshots.value = result.result ?? [];
    }
  } finally {
    loadingSnapshots.value = false;
  }
}

async function downloadSnapshot(id: string) {
  // The endpoint streams the snapshot zip directly (cookie-authenticated);
  // v2 snapshots are assembled server-side from their blobs.
  const baseUrl = useRuntimeConfig().public.baseApi;
  window.location.href = `${baseUrl}/api/backup/snapshots/${id}/download`;
}

async function deleteSnapshot(id: string) {
  const result = await makeRequest(`backup/snapshots/${id}`, 'DELETE', {});
  if (result.status === 'success') {
    notifications.add({ type: 'success', title: t('settings.backups.snapshotDeleted') });
    await loadSnapshots();
  } else {
    notifications.add({ type: 'error', title: t('common.errors.generic'), message: result.message });
  }
}

async function restoreSnapshot(snap: BackupSnapshot) {
  // Inline two-step confirm: the first click arms it, the second executes.
  if (pendingRestore.value !== snap.id) {
    pendingRestore.value = snap.id;
    pendingPlan.value = null;
    restoreConfirmTimer = setTimeout(() => {
      pendingRestore.value = null;
      pendingPlan.value = null;
    }, 8000);
    // Dry-run plan while the user reads the warning (best-effort).
    const plan = await makeRequest<RestorePlan>(`backup/snapshots/${snap.id}/preview`, 'POST', {
      passphrase: restorePassphrase.value.trim() || undefined,
    });
    if (pendingRestore.value === snap.id && plan.status === 'success' && plan.result) {
      pendingPlan.value = plan.result;
    }
    return;
  }
  pendingPlan.value = null;
  if (restoreConfirmTimer) clearTimeout(restoreConfirmTimer);
  pendingRestore.value = null;
  isRestoring.value = true;
  try {
    const result = await makeRequest<{ nodes: number; links: number; files: number; permissions: number; invitations: number; trashed: number; links_skipped: string[]; files_skipped: string[] }>(
      `backup/snapshots/${snap.id}/restore`,
      'POST',
      {
        passphrase: restorePassphrase.value.trim() || undefined,
        force: restoreForce.value,
      },
    );
    if (result.status === 'success') {
      const r = result.result;
      notifications.add({
        type: 'success',
        title: t('settings.backups.restored'),
        message: t('settings.backups.restoredDetailFull', { nodes: r?.nodes ?? 0, links: r?.links ?? 0, files: r?.files ?? 0, permissions: r?.permissions ?? 0, invitations: r?.invitations ?? 0, trashed: r?.trashed ?? 0 }),
      });
    } else {
      notifications.add({ type: 'error', title: t('settings.backups.restoreFailed'), message: result.message });
    }
  } finally {
    isRestoring.value = false;
  }
}

async function onRestoreFile(event: Event) {
  const file = (event.target as HTMLInputElement).files?.[0];
  (event.target as HTMLInputElement).value = '';
  if (!file) return;
  isRestoring.value = true;
  try {
    const form = new FormData();
    form.append('file', file);
    form.append('force', 'true');
    const result = await makeRequest<{ nodes: number; links: number; files: number }>('backup/restore/upload', 'POST', form);
    if (result.status === 'success') {
      const r = result.result;
      notifications.add({
        type: 'success',
        title: t('settings.backups.restored'),
        message: t('settings.backups.restoredDetail', { nodes: r?.nodes ?? 0, links: r?.links ?? 0, files: r?.files ?? 0 }),
      });
      await loadSnapshots();
    } else {
      notifications.add({ type: 'error', title: t('settings.backups.restoreFailed'), message: result.message });
    }
  } finally {
    isRestoring.value = false;
  }
}

const isProcessing = computed(() => currentJob.value?.status === 'pending' || currentJob.value?.status === 'processing');

const hasAnyOption = computed(() => options.includeDocuments || options.includeFiles || options.includeSettings || options.includeMetadata);

const statusLabel = computed(() => {
  switch (currentJob.value?.status) {
    case 'pending':
      return t('common.status.pending');
    case 'processing':
      return t('common.status.processing');
    case 'completed':
      return t('common.status.completed');
    case 'failed':
      return t('common.status.failed');
    default:
      return '';
  }
});

const copyLink = () => {
  if (currentJob.value?.download_url) {
    navigator.clipboard.writeText(currentJob.value.download_url);
    notifications.add({ type: 'success', title: t('common.notifications.copiedTitle'), message: t('common.notifications.copiedMessage') });
  }
};

async function startBackup() {
  // Clear previous job
  stopPolling();
  currentJob.value = null;

  const result = await makeRequest<{ job_id: string; message: string }>('backup', 'POST', {
    include_documents: options.includeDocuments,
    include_files: options.includeFiles,
    include_settings: options.includeSettings,
    include_metadata: options.includeMetadata,
    memo: options.memo.trim() || undefined,
    passphrase: options.passphrase.trim() || undefined,
    expires_in: options.expiresIn,
  });

  if (result.status !== 'success' || !result.result?.job_id) {
    error('Failed to start backup');
    return;
  }

  notifications.add({ type: 'success', title: t('settings.backups.notifications.startedTitle'), message: t('settings.backups.notifications.started') });
  // Start polling for status
  startPolling(result.result.job_id);
}

async function checkStatus(jobId: string) {
  const result = await makeRequest<BackupJob>(`backup/${jobId}`, 'GET', {});

  if (result.status !== 'success' || !result.result) {
    stopPolling();
    error();
    return;
  }
  currentJob.value = result.result;

  // Stop polling if job is finished
  if (result.result.status === 'completed' || result.result.status === 'failed') {
    stopPolling();

    if (result.result.status === 'completed') {
      if (result.result.download_url) {
        notifications.add({ type: 'info', title: t('settings.backups.notifications.packagedTitle'), message: t('settings.backups.notifications.packaged') });
      } else {
        notifications.add({ type: 'success', title: t('settings.backups.notifications.completedTitle'), message: t('settings.backups.notifications.completed') });
      }
      await loadSnapshots();
    } else if (result.result.status === 'failed') {
      error(result.result.message, result.result.error);
    }
  }
}

function startPolling(jobId: string) {
  // Initial check
  checkStatus(jobId);

  // Poll every 2 seconds
  pollingInterval.value = setInterval(() => {
    checkStatus(jobId);
  }, 2000);
}

function stopPolling() {
  if (pollingInterval.value) {
    clearInterval(pollingInterval.value);
    pollingInterval.value = null;
  }
}

async function cancelBackup() {
  if (!currentJob.value?.id) return;

  const result = await makeRequest(`backup/${currentJob.value.id}`, 'DELETE', {});

  if (result.status !== 'success') {
    notifications.add({
      type: 'error',
      title: t('common.errors.generic'),
      message: result.message || t('settings.backups.notifications.failedToCancelBackup'),
    });
    return;
  }

  stopPolling();
  notifications.add({ type: 'info', title: t('settings.backups.notifications.canceled') });
  currentJob.value = null;
}

function error(message?: string, error?: string) {
  notifications.add({ type: 'error', title: t('common.errors.generic'), message: message || t('common.errors.unknown') });
  currentJob.value = {
    status: 'failed',
    id: '',
    user_id: '',
    options: { include_documents: false, include_files: false, include_metadata: false },
    progress: 0,
    message: message || t('settings.backups.notifications.errorFetching'),
    error: error,
    created_at: '',
    updated_at: '',
  };
}

onMounted(async () => {
  await loadSnapshots();
  try {
    const result = await makeRequest<unknown[]>('backup/targets', 'GET', {});
    targetCount.value = result.status === 'success' ? (result.result ?? []).length : 0;
  } catch {
    targetCount.value = 0;
  }
});

function goToTargets() {
  const router = useRouter();
  const route = useRoute();
  router.push({ query: { ...route.query, p: 'snapshotTargets' } });
}

// Cleanup on unmount
onUnmounted(() => {
  stopPolling();
  if (restoreConfirmTimer) clearTimeout(restoreConfirmTimer);
});
</script>

<style scoped lang="scss">
.main {
  max-width: 600px;
  margin: auto;
  text-align: center;
}

.options-section {
  padding: 1rem;
  text-align: left;

  h3 {
    margin-bottom: 1rem;
    font-size: 1rem;
    color: var(--text-body);
  }
}

.options-grid {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.memo-input {
  padding: 0.5rem 0.75rem;
  font-size: 0.875rem;
}

.snapshot-memo {
  margin-left: 0.5rem;
  font-size: 0.75rem;
  color: var(--text-secondary);
}

.snapshot-encrypted {
  margin-left: 0.35rem;
  font-size: 0.75rem;
}

.no-targets {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.passphrase-row {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  font-size: 0.875rem;
}

.force-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-top: 0.5rem;
  font-size: 0.875rem;
  font-weight: 600;
}

.restore-warning p {
  margin: 0.25rem 0;
}

.restore-plan {
  font-size: 0.75rem;
  opacity: 0.9;
}

.option-label {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
}

.progress-section {
  padding: 1rem;
  border-radius: var(--radius-sm);
}

.progress-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 0.75rem;
}

.status-badge {
  padding: 0.25rem 0.75rem;
  border-radius: var(--radius-sm);
  font-size: 0.875rem;
  font-weight: 500;

  &.pending {
    color: var(--orange);
    background: var(--orange-bg);
  }

  &.processing {
    color: var(--primary);
    background: var(--primary-bg);
  }

  &.completed {
    color: var(--green);
    background: var(--green-bg);
  }

  &.failed {
    color: var(--red);
    background: var(--red-bg);
  }
}

.progress-percent {
  font-weight: 600;
  color: var(--primary);
}

.progress-bar-container {
  width: 100%;
  height: 8px;
  margin-bottom: 0.75rem;
  border-radius: var(--radius-xs);
  background: var(--border);
  overflow: hidden;
}

.progress-bar {
  height: 100%;
  border-radius: var(--radius-xs);
  background: var(--primary);
  transition: width $transition-medium ease;
}

.progress-message {
  margin-bottom: 0.75rem;
  font-size: 1rem;
  font-weight: bold;
  color: var(--text-primary);
}

.error-message {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-top: 0.75rem;
  padding: 0.75rem;
  border-radius: var(--radius-sm);
  color: var(--error);
  text-align: left;
  background: var(--error-bg);
}

.link-section {
  padding: 1rem;
}

.expiry-notice {
  display: flex;
  justify-content: center;
  align-items: center;
  gap: 0.25rem;
  margin-bottom: 1rem;
  font-size: 0.75rem;
  color: var(--text-primary);
}

input[type='text'] {
  padding: 0.75rem;
}

.actions-row {
  justify-content: center;
  margin-top: 0.75rem;

  a {
    text-decoration: none;
  }
}

.snapshots-section {
  margin-top: 1.5rem;
}

.snapshots-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 0.75rem;
}

.snapshot-empty {
  font-size: 0.875rem;
  color: var(--text-secondary);
}

.snapshot-list {
  margin: 0;
  padding: 0;
  text-align: left;
  list-style: none;
}

.snapshot-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 0.25rem;

  & + & {
    border-top: 1px solid var(--border);
  }
}

.snapshot-date {
  font-size: 0.875rem;
  color: var(--text-body);
}

.snapshot-size {
  margin-left: auto;
  font-size: 0.75rem;
  color: var(--text-secondary);
}

.snapshot-actions {
  display: flex;
  gap: 0.25rem;
}

.snapshot-btn {
  padding: 0.25rem;
  border: none;
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  background: none;
  cursor: pointer;
  transition:
    color 0.1s,
    background 0.1s;

  &:hover {
    color: var(--primary);
    background: var(--surface-raised);
  }

  &.confirm-restore {
    color: var(--orange);
    background: var(--orange-bg);
  }
}

.restore-warning {
  margin-top: 0.5rem;
  padding: 0.5rem;
  border-radius: var(--radius-sm);
  font-size: 0.75rem;
  color: var(--orange);
  background: var(--orange-bg);
}

.upload-restore {
  margin-top: 1rem;
  text-align: center;
}

.hidden-input {
  display: none;
}

.upload-hint {
  margin-top: 0.5rem;
  font-size: 0.75rem;
  color: var(--text-secondary);
}
</style>
