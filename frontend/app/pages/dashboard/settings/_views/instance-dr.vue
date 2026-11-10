<template>
  <div>
    <h2 class="page-title">{{ t('settings.instanceDR.title') }}</h2>
    <p class="page-subtitle">{{ t('settings.instanceDR.subtitle') }}</p>

    <div class="main">
      <!-- Structured config form -->
      <div class="options-section">
        <h3>{{ t('settings.instanceDR.configTitle') }}</h3>
        <p v-if="deployError" class="error-message">{{ deployError }}</p>
        <template v-else>
          <p v-if="!confExists" class="hint">{{ t('settings.instanceDR.formMissing') }}</p>

          <!-- PostgreSQL sources -->
          <fieldset class="group">
            <legend>{{ t('settings.instanceDR.sources') }}</legend>
            <p class="hint">{{ t('settings.instanceDR.sourcesHint') }}</p>
            <div v-for="(source, idx) in config.sources" :key="`src-${idx}`" class="record">
              <div class="record-head">
                <strong>{{ source.name || t('settings.instanceDR.unnamed') }}</strong>
                <div class="head-actions">
                  <button
                    class="probe"
                    type="button"
                    :disabled="probing === source.name"
                    @click="probe(source.name)"
                  >
                    {{ probing === source.name ? t('settings.instanceDR.probing') : t('settings.instanceDR.test') }}
                  </button>
                  <button class="remove" type="button" @click="config.sources.splice(idx, 1)">
                    {{ t('settings.instanceDR.remove') }}
                  </button>
                </div>
              </div>
              <p v-if="probeText(source.name)" class="probe-status" :class="probeClass(source.name)">
                {{ probeText(source.name) }}
              </p>
              <div class="fields">
                <label>
                  {{ t('settings.instanceDR.fieldName') }}
                  <input v-model="source.name" type="text" placeholder="pergame" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldHost') }}
                  <input v-model="source.host" type="text" placeholder="localhost" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldPort') }}
                  <input v-model.number="source.port" type="number" min="1" max="65535" placeholder="5432" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldDatabase') }}
                  <input v-model="source.database" type="text" placeholder="pergame" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldUsername') }}
                  <input v-model="source.username" type="text" autocomplete="off" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldPassword') }}
                  <input v-model="source.password" type="password" autocomplete="new-password" />
                </label>
              </div>
            </div>
            <AppButton type="secondary" small @click="addSource">{{ t('settings.instanceDR.addSource') }}</AppButton>
          </fieldset>

          <!-- Destinations -->
          <fieldset class="group">
            <legend>{{ t('settings.instanceDR.destinations') }}</legend>
            <p class="hint">{{ t('settings.instanceDR.destinationsHint') }}</p>

            <div v-for="(dest, idx) in config.s3_destinations" :key="`s3-${idx}`" class="record">
              <div class="record-head">
                <strong>S3 · {{ dest.name || t('settings.instanceDR.unnamed') }}</strong>
                <div class="head-actions">
                  <button
                    class="probe"
                    type="button"
                    :disabled="probing === dest.name || !dest.name"
                    @click="probe(dest.name)"
                  >
                    {{ probing === dest.name ? t('settings.instanceDR.probing') : t('settings.instanceDR.test') }}
                  </button>
                  <button class="remove" type="button" @click="removeDestination('s3', idx)">
                    {{ t('settings.instanceDR.remove') }}
                  </button>
                </div>
              </div>
              <p v-if="probeText(dest.name)" class="probe-status" :class="probeClass(dest.name)">
                {{ probeText(dest.name) }}
              </p>
              <div class="fields">
                <label>
                  {{ t('settings.instanceDR.fieldName') }}
                  <input v-model="dest.name" type="text" placeholder="primary" />
                </label>
                <label class="wide">
                  {{ t('settings.instanceDR.fieldEndpoint') }}
                  <input v-model="dest.endpoint" type="url" placeholder="https://s3.example.com:9000" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldBucket') }}
                  <input v-model="dest.bucket" type="text" placeholder="pergame-backups" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldPrefix') }}
                  <input v-model="dest.prefix" type="text" placeholder="pg" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldAccessKey') }}
                  <input v-model="dest.access_key" type="password" autocomplete="off" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldSecretKey') }}
                  <input v-model="dest.secret_key" type="password" autocomplete="new-password" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldRegion') }}
                  <input v-model="dest.region" type="text" placeholder="us-east-1" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldProvider') }}
                  <select v-model="dest.provider">
                    <option value="Other">Other (MinIO / RustFS / Garage)</option>
                    <option value="AWS">AWS</option>
                    <option value="Cloudflare">Cloudflare R2</option>
                    <option value="Backblaze">Backblaze B2</option>
                  </select>
                </label>
                <label class="inline">
                  <input v-model="dest.secure" type="checkbox" />
                  {{ t('settings.instanceDR.fieldSecure') }}
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldRetainDays') }}
                  <input v-model.number="dest.retain_days" min="0" max="3650" type="number" placeholder="0" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldKeepLast') }}
                  <input v-model.number="dest.keep_last" min="0" max="1000" type="number" placeholder="0" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldCron') }}
                  <input v-model="dest.cron" type="text" placeholder="30 3 * * *" spellcheck="false" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldCrypt') }}
                  <input
                    v-model="dest.crypt_pass"
                    type="password"
                    autocomplete="new-password"
                    :placeholder="t('settings.instanceDR.cryptPlaceholder')"
                  />
                </label>
              </div>
            </div>

            <div v-for="(dest, idx) in config.webdav_destinations" :key="`dav-${idx}`" class="record">
              <div class="record-head">
                <strong>WebDAV · {{ dest.name || t('settings.instanceDR.unnamed') }}</strong>
                <div class="head-actions">
                  <button
                    class="probe"
                    type="button"
                    :disabled="probing === dest.name || !dest.name"
                    @click="probe(dest.name)"
                  >
                    {{ probing === dest.name ? t('settings.instanceDR.probing') : t('settings.instanceDR.test') }}
                  </button>
                  <button class="remove" type="button" @click="removeDestination('dav', idx)">
                    {{ t('settings.instanceDR.remove') }}
                  </button>
                </div>
              </div>
              <p v-if="probeText(dest.name)" class="probe-status" :class="probeClass(dest.name)">
                {{ probeText(dest.name) }}
              </p>
              <div class="fields">
                <label>
                  {{ t('settings.instanceDR.fieldName') }}
                  <input v-model="dest.name" type="text" placeholder="nas" />
                </label>
                <label class="wide">
                  {{ t('settings.instanceDR.fieldUrl') }}
                  <input v-model="dest.url" type="url" placeholder="https://nas.example.com/dav/" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldUsername') }}
                  <input v-model="dest.username" type="text" autocomplete="off" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldPassword') }}
                  <input v-model="dest.password" type="password" autocomplete="new-password" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldPrefix') }}
                  <input v-model="dest.prefix" type="text" placeholder="pergame/pg" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldRetainDays') }}
                  <input v-model.number="dest.retain_days" min="0" max="3650" type="number" placeholder="0" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldKeepLast') }}
                  <input v-model.number="dest.keep_last" min="0" max="1000" type="number" placeholder="0" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldCron') }}
                  <input v-model="dest.cron" type="text" placeholder="30 3 * * *" spellcheck="false" />
                </label>
                <label>
                  {{ t('settings.instanceDR.fieldCrypt') }}
                  <input
                    v-model="dest.crypt_pass"
                    type="password"
                    autocomplete="new-password"
                    :placeholder="t('settings.instanceDR.cryptPlaceholder')"
                  />
                </label>
              </div>
            </div>

            <div class="actions-row">
              <AppButton type="secondary" small @click="addS3">{{ t('settings.instanceDR.addS3') }}</AppButton>
              <AppButton type="secondary" small @click="addWebdav">{{ t('settings.instanceDR.addWebdav') }}</AppButton>
            </div>
            <p class="hint">{{ t('settings.instanceDR.knobsHint') }}</p>
          </fieldset>

          <!-- Retention -->
          <fieldset class="group">
            <legend>{{ t('settings.instanceDR.retention') }}</legend>
            <div class="fields">
              <label>
                {{ t('settings.instanceDR.retainDays') }}
                <input v-model.number="config.retain_days" type="number" min="1" max="3650" />
              </label>
              <label class="inline">
                <input v-model="config.verify_upload" type="checkbox" />
                {{ t('settings.instanceDR.verifyUpload') }}
              </label>
            </div>
          </fieldset>

          <p class="hint">{{ t('settings.instanceDR.maskedNote') }}</p>
          <div class="actions-row">
            <AppButton type="primary" :disabled="savingConf" @click="saveConfig">
              <Icon name="save" display="sm" />
              {{ t('settings.instanceDR.save') }}
            </AppButton>
          </div>

          <!-- Advanced: raw conf escape hatch -->
          <details class="advanced">
            <summary>{{ t('settings.instanceDR.advanced') }}</summary>
            <p class="hint">{{ t('settings.instanceDR.advancedHint') }}</p>
            <textarea
              v-model="confContent"
              class="conf-editor"
              rows="14"
              spellcheck="false"
              :placeholder="t('settings.instanceDR.confMissing')"
            ></textarea>
            <div class="actions-row">
              <AppButton type="secondary" :disabled="savingConf" @click="saveRawConfig">
                {{ t('settings.instanceDR.saveRaw') }}
              </AppButton>
              <AppButton type="secondary" :disabled="savingConf" @click="loadConfig">
                {{ t('settings.instanceDR.reload') }}
              </AppButton>
            </div>
          </details>
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

interface PgSource {
  name: string;
  host: string;
  port: number;
  database: string;
  username: string;
  password: string;
}
interface DestinationKnobs {
  retain_days: number;
  keep_last: number;
  cron: string;
  crypt_pass: string;
}
interface S3Destination extends DestinationKnobs {
  name: string;
  endpoint: string;
  bucket: string;
  prefix: string;
  access_key: string;
  secret_key: string;
  secure: boolean;
  region: string;
  provider: string;
}
interface WebdavDestination extends DestinationKnobs {
  name: string;
  url: string;
  username: string;
  password: string;
  prefix: string;
}
interface DrConfig {
  sources: PgSource[];
  s3_destinations: S3Destination[];
  webdav_destinations: WebdavDestination[];
  retain_days: number;
  verify_upload: boolean;
}
/** Structured probe result (backend parses the script's PROBE| line). */
interface ProbeResult {
  kind: string;
  name: string;
  reachable: string;
  snapshots: number | null;
  fingerprint: string;
  note: string;
}

const emptyConfig = (): DrConfig => ({
  sources: [],
  s3_destinations: [],
  webdav_destinations: [],
  retain_days: 14,
  verify_upload: true,
});

const config = ref<DrConfig>(emptyConfig());
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

// "Test connection" state, keyed by destination/source name.
const probeResults = ref<Record<string, ProbeResult>>({});
const probing = ref('');

onMounted(loadConfig);

onUnmounted(() => {
  if (pollTimer) clearInterval(pollTimer);
  if (restoreTimer) clearTimeout(restoreTimer);
});

function addSource() {
  config.value.sources.push({ name: '', host: 'localhost', port: 5432, database: 'pergame', username: '', password: '' });
}

function addS3() {
  config.value.s3_destinations.push({
    name: '',
    endpoint: '',
    bucket: '',
    prefix: 'pg',
    access_key: '',
    secret_key: '',
    secure: true,
    region: 'us-east-1',
    provider: 'Other',
    retain_days: 0,
    keep_last: 30,
    cron: '',
    crypt_pass: '',
  });
}

function addWebdav() {
  config.value.webdav_destinations.push({
    name: '',
    url: '',
    username: '',
    password: '',
    prefix: 'pergame/pg',
    retain_days: 0,
    keep_last: 30,
    cron: '',
    crypt_pass: '',
  });
}

/** Probe status lookup by name — empty string when no result yet, so the
 * template can v-if on the text without index-signature undefined leaks. */
function probeFor(name?: string): ProbeResult | undefined {
  const key = (name || '').trim();
  return key ? probeResults.value[key] : undefined;
}

function probeText(name?: string): string {
  const r = probeFor(name);
  if (!r) return '';
  if (r.reachable === 'true') {
    const snaps = r.snapshots !== null ? ` · ${t('settings.instanceDR.probeSnapshots', { count: r.snapshots })}` : '';
    const fp = r.fingerprint ? ` · ${r.fingerprint}` : '';
    return `${t('settings.instanceDR.probeOk')}${snaps}${fp}`;
  }
  if (r.reachable === 'dry-run') return t('settings.instanceDR.probeDryRun');
  return `${t('settings.instanceDR.probeFail')}${r.note ? ` (${r.note})` : ''}`;
}

function probeClass(name?: string): string {
  return probeFor(name)?.reachable ?? '';
}

/**
 * Runs dr-backup.sh -p <name> via the job protocol and stores the structured
 * PROBE result under the target name. Works for destinations and sources.
 */
async function probe(rawName: string) {
  const name = (rawName || '').trim();
  if (!name) {
    notifications.add({ type: 'error', title: t('settings.instanceDR.nameRequired') });
    return;
  }
  probing.value = name;
  try {
    const created = await makeRequest<{ job_id: string }>('admin/dr-backup/jobs', 'POST', {
      action: 'probe',
      name,
    });
    if (created.status !== 'success' || !created.result?.job_id) {
      notifications.add({ type: 'error', title: t('common.errors.generic'), message: created.message });
      return;
    }
    const jobId = created.result.job_id;
    for (let tick = 0; tick < 60; tick++) {
      await new Promise(resolve => setTimeout(resolve, 2000));
      const res = await makeRequest<{
        status: string;
        exit?: number;
        output_tail?: string;
        probe?: ProbeResult;
      }>(`admin/dr-backup/jobs/${jobId}`, 'GET', {});
      if (res.status !== 'success') {
        notifications.add({ type: 'error', title: t('common.errors.generic'), message: res.message });
        return;
      }
      if (res.result?.status === 'done') {
        if (res.result.probe) {
          probeResults.value = { ...probeResults.value, [name]: res.result.probe };
          notifications.add({
            type: res.result.probe.reachable === 'true' ? 'success' : 'error',
            title:
              res.result.probe.reachable === 'true'
                ? t('settings.instanceDR.probeOk')
                : t('settings.instanceDR.probeFail'),
          });
        } else {
          notifications.add({ type: 'error', title: t('settings.instanceDR.probeNoOutput') });
        }
        return;
      }
    }
    notifications.add({ type: 'error', title: t('common.errors.generic'), message: 'probe timeout' });
  } finally {
    probing.value = '';
  }
}

/**
 * Destination removal with the Siyuan delete-check spirit: a probe showing
 * snapshots on the destination forces a type-the-name confirmation, plain
 * removal always warns that remote snapshots are not touched.
 */
function removeDestination(kind: 's3' | 'dav', idx: number) {
  const list = kind === 's3' ? config.value.s3_destinations : config.value.webdav_destinations;
  const dest = list[idx];
  if (!dest) return;
  const name = dest.name.trim();
  const doRemove = () => list.splice(idx, 1);
  const known = name ? probeResults.value[name] : undefined;
  if (known && known.snapshots !== null && known.snapshots > 0) {
    const typed = window.prompt(
      t('settings.instanceDR.deleteGatePrompt', { name, count: known.snapshots }),
    );
    if (typed === null) return;
    if (typed.trim() === name) {
      doRemove();
    } else {
      notifications.add({ type: 'error', title: t('settings.instanceDR.deleteGateMismatch') });
    }
    return;
  }
  if (window.confirm(t('settings.instanceDR.removeDestConfirm', { name: name || '?' }))) {
    doRemove();
  }
}

async function loadConfig() {
  // Structured form and raw editor read the same file; load both so the
  // advanced section always reflects what is actually saved.
  const [structured, raw] = await Promise.all([
    makeRequest<{ exists: boolean; config: DrConfig }>('admin/dr-backup/config/structured', 'GET', {}),
    makeRequest<{ exists: boolean; content: string }>('admin/dr-backup/config', 'GET', {}),
  ]);

  if (structured.status === 'success' && structured.result) {
    deployError.value = '';
    confExists.value = structured.result.exists;
    config.value = structured.result.config ?? emptyConfig();
  } else {
    deployError.value = structured.message || t('settings.instanceDR.notDeployed');
  }

  if (raw.status === 'success' && raw.result) {
    confContent.value = raw.result.content ?? '';
  }
}

async function saveConfig() {
  savingConf.value = true;
  try {
    const result = await makeRequest('admin/dr-backup/config/structured', 'PUT', config.value);
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

/** Raw conf save — for settings the form does not expose. */
async function saveRawConfig() {
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

.group {
  margin-bottom: 1rem;
  padding: 0.75rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);

  legend {
    padding: 0 0.4rem;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-body);
  }
}

.record {
  margin-bottom: 0.75rem;
  padding: 0.6rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
}

.record-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 0.5rem;
  font-size: 0.8rem;
}

.head-actions {
  display: flex;
  gap: 0.35rem;
}

.probe {
  padding: 0.2rem 0.4rem;
  border-radius: var(--radius-sm);
  font-size: 0.75rem;
  color: var(--primary);
  background: transparent;
  cursor: pointer;

  &:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
}

.probe-status {
  margin: 0 0 0.5rem;
  padding: 0.35rem 0.5rem;
  border-radius: var(--radius-sm);
  font-size: 0.7rem;
  word-break: break-all;

  &.true {
    color: var(--success, #2e7d32);
    background: rgb(46 125 50 / 8%);
  }

  &.false {
    color: var(--error);
    background: var(--error-bg, rgb(211 47 47 / 8%));
  }

  &.dry-run {
    color: var(--text-secondary);
    background: var(--surface-raised-light, var(--surface));
  }
}

.remove {
  padding: 0.2rem 0.4rem;
  border-radius: var(--radius-sm);
  font-size: 0.75rem;
  color: var(--error);
  background: transparent;
  cursor: pointer;
}

.fields {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
  gap: 0.5rem;

  label {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    font-size: 0.7rem;
    color: var(--text-secondary);
  }

  input,
  select {
    padding: 0.4rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font-size: 0.8rem;
    color: var(--text-body);
    background: var(--surface-raised-light, var(--surface));
  }
}

.fields .wide {
  grid-column: span 2;
}

.fields .inline {
  flex-direction: row;
  align-items: center;
  align-self: end;
  gap: 0.35rem;
  padding-bottom: 0.45rem;
}

.advanced {
  margin-top: 1rem;

  summary {
    font-size: 0.85rem;
    color: var(--text-secondary);
    cursor: pointer;
  }
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
