<template>
  <div>
    <h2 class="page-title">{{ t('settings.snapshotTargets.title') }}</h2>
    <p class="page-subtitle">{{ t('settings.snapshotTargets.subtitle') }}</p>

    <div class="main">
      <p v-if="loadError" class="error-message">{{ loadError }}</p>

      <!-- target list -->
      <div v-if="targets.length > 0" class="target-list">
        <div v-for="tg in targets" :key="tg.id" class="target-card">
          <div class="target-head">
            <div class="target-title">
              <strong>{{ tg.name }}</strong>
              <span class="kind-badge">{{ tg.kind.toUpperCase() }}</span>
              <span class="endpoint">{{ tg.endpoint }}<template v-if="tg.bucket"> · {{ tg.bucket }}</template></span>
            </div>
            <span class="status" :class="tg.enabled ? 'on' : 'off'">
              {{ tg.enabled ? t('settings.snapshotTargets.enabled') : t('settings.snapshotTargets.disabled') }}
            </span>
          </div>

          <p class="meta">
            {{ t('settings.snapshotTargets.usersCount', { count: tg.users.length }) }}
            · {{ t('settings.snapshotTargets.keepLast') }} {{ tg.keep_last }}
            <template v-if="tg.schedule"> · cron {{ tg.schedule }}</template>
            <template v-if="tg.last_result"> · {{ tg.last_result }}</template>
          </p>

          <div class="row-actions">
            <AppButton type="secondary" small :disabled="busy === tg.id" @click="probe(tg)">
              {{ t('settings.snapshotTargets.test') }}
            </AppButton>
            <AppButton type="secondary" small :disabled="busy === tg.id" @click="sync(tg)">
              {{ t('settings.snapshotTargets.syncNow') }}
            </AppButton>
            <AppButton
              v-if="isForeign(tg.id)"
              type="danger"
              small
              :disabled="busy === tg.id"
              @click="takeOwnership(tg)"
            >
              {{ t('settings.snapshotTargets.takeover') }}
            </AppButton>
            <AppButton type="secondary" small @click="startEdit(tg)">
              {{ t('settings.snapshotTargets.edit') }}
            </AppButton>
            <AppButton type="danger" small :disabled="busy === tg.id" @click="removeTarget(tg)">
              {{ t('settings.snapshotTargets.delete') }}
            </AppButton>
          </div>

          <p v-if="probeLabel(tg.id)" class="probe" :class="probeClass(tg.id)">
            {{ probeLabel(tg.id) }}
          </p>
        </div>
      </div>
      <p v-else-if="!loading" class="hint">{{ t('settings.snapshotTargets.empty') }}</p>

      <!-- create / edit form -->
      <div class="options-section">
        <h3>{{ editing ? t('settings.snapshotTargets.editTitle') : t('settings.snapshotTargets.createTitle') }}</h3>

        <div class="form-grid">
          <label>
            {{ t('settings.snapshotTargets.kind') }}
            <AppSelect v-model="form.kind" :items="KINDS" :searchable="false" />
          </label>
          <label>
            {{ t('settings.snapshotTargets.name') }}
            <input v-model="form.name" type="text" :placeholder="'offsite-1'" />
          </label>
          <label>
            {{ t('settings.snapshotTargets.endpoint') }}
            <input v-model="form.endpoint" type="text" :placeholder="endpointHint" />
          </label>
          <label>
            {{ bucketLabel }}
            <input v-model="form.bucket" type="text" :placeholder="bucketHint" />
          </label>
          <label>
            {{ t('settings.snapshotTargets.prefix') }}
            <input v-model="form.prefix" type="text" placeholder="pergame" />
          </label>
          <label>
            {{ t('settings.snapshotTargets.accessKey') }}
            <input v-model="form.access_key" type="text" :placeholder="editing ? '***' : ''" />
          </label>
          <label>
            {{ t('settings.snapshotTargets.secretKey') }}
            <input v-model="form.secret_key" type="password" :placeholder="editing ? '***' : ''" />
          </label>
          <label>
            {{ t('settings.snapshotTargets.keepLast') }}
            <input v-model.number="form.keep_last" type="number" min="1" max="100" />
          </label>
          <label>
            {{ t('settings.snapshotTargets.schedule') }}
            <input v-model="form.schedule" type="text" placeholder="30 3 * * *" />
          </label>
          <label class="checkbox-row">
            <input v-model="form.secure" type="checkbox" />
            {{ t('settings.snapshotTargets.secure') }}
          </label>
          <label class="checkbox-row">
            <input v-model="form.enabled" type="checkbox" />
            {{ t('settings.snapshotTargets.enabled') }}
          </label>
        </div>

        <!-- owning users -->
        <fieldset class="users-box">
          <legend>{{ t('settings.snapshotTargets.users') }}</legend>
          <p class="hint">{{ t('settings.snapshotTargets.usersHint') }}</p>
          <label v-for="u in users" :key="u.id" class="user-row">
            <input v-model="form.users" type="checkbox" :value="u.id" />
            <span>{{ u.username }}</span>
            <span class="user-mail">{{ u.email }}</span>
          </label>
          <p v-if="users.length === 0" class="hint">{{ t('settings.snapshotTargets.noUsers') }}</p>
        </fieldset>

        <!-- Save gate: connection fields must be tested first (siyuan #15 §9) -->
        <div class="test-row">
          <AppButton type="secondary" :disabled="testing || saving" @click="testDraft">
            {{ testing ? t('settings.snapshotTargets.testing') : t('settings.snapshotTargets.testConnection') }}
          </AppButton>
          <span v-if="draftProbe" class="probe" :class="draftProbe.reachable ? 'ok' : 'bad'">
            {{ draftProbeLabel }}
          </span>
          <span v-else-if="needsTest" class="hint">{{ t('settings.snapshotTargets.mustTest') }}</span>
        </div>

        <div class="row-actions">
          <AppButton type="primary" :disabled="saving || needsTest" @click="save">
            {{ saving ? t('settings.snapshotTargets.saving') : t('settings.snapshotTargets.save') }}
          </AppButton>
          <AppButton v-if="editing" type="secondary" @click="cancelEdit">
            {{ t('common.actions.cancel') }}
          </AppButton>
        </div>
        <p v-if="formError" class="error-message">{{ formError }}</p>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
interface Target {
  id: string;
  kind: string;
  name: string;
  endpoint: string;
  bucket: string;
  prefix: string;
  access_key: string;
  secret_key: string;
  secure: boolean;
  region: string;
  provider: string;
  keep_last: number;
  max_age_days: number;
  schedule: string;
  enabled: boolean;
  last_result: string;
  users: string[];
}

interface Probe {
  reachable: boolean;
  snapshot_objects: number;
  fingerprint?: string | null;
  owner_instance?: string | null;
  owned_by_me?: boolean;
  /** Server-computed digest of the tested connection fields. */
  connection_token?: string;
}

const { t } = useI18nT();
const notifications = useNotifications();

const KINDS = ['s3', 'webdav'] as unknown as never[];

const targets = ref<Target[]>([]);
const users = ref<Array<{ id: string; username: string; email?: string }>>([]);
const loading = ref(false);
const loadError = ref('');
const busy = ref('');
const probeResults = ref<Record<string, Probe>>({});
const saving = ref(false);
const formError = ref('');
const editing = ref<Target | null>(null);

const form = reactive({
  kind: 's3',
  name: '',
  endpoint: '',
  bucket: '',
  prefix: '',
  access_key: '',
  secret_key: '',
  secure: true,
  keep_last: 10,
  schedule: '',
  enabled: true,
  users: [] as string[],
  region: 'us-east-1',
  provider: 'Other',
});

// Draft probe (siyuan #15 §9): a target may only be saved after its
// connection fields were tested, and the token pins the exact field values.
const draftProbe = ref<Probe | null>(null);
const testing = ref(false);

/** Connection-bearing form slice: these are what a test validates. */
const connectionFields = computed(() => ({
  kind: form.kind,
  endpoint: form.endpoint.trim(),
  bucket: form.bucket.trim(),
  prefix: form.prefix.trim(),
  access_key: form.access_key,
  secret_key: form.secret_key,
  secure: form.secure,
  region: form.region,
  provider: form.provider,
}));

/** Fields the successful test ran against; null until a test passes. */
const testedFields = ref<string | null>(null);

/** True while the form's connection fields differ from the tested ones. */
const needsTest = computed(
  () => testedFields.value === null || JSON.stringify(connectionFields.value) !== testedFields.value,
);

const endpointHint = computed(() =>
  form.kind === 's3' ? 'https://s3.example.com:9000' : 'https://nas.example.com/dav',
);

const bucketLabel = computed(() => t('settings.snapshotTargets.bucket'));

const bucketHint = 'pergame-offsite';

onMounted(async () => {
  await Promise.all([load(), loadUsers()]);
});

async function load() {
  loading.value = true;
  const result = await makeRequest<Target[]>('admin/dr-backup/targets', 'GET', {});
  loading.value = false;
  if (result.status === 'success') {
    targets.value = result.result ?? [];
    loadError.value = '';
  } else {
    loadError.value = result.message || t('settings.snapshotTargets.loadFailed');
  }
}

async function loadUsers() {
  // Admin user list; the endpoint is admin-guarded upstream.
  const result = await makeRequest<Array<{ id: string; username: string; email?: string }>>(
    'admin/dr-backup/backup-users',
    'GET',
    {},
  );
  if (result.status === 'success') {
    users.value = (result.result as Array<{ id: string; username: string; email?: string }>) ?? [];
  }
}

function probeFor(id: string): Probe | undefined {
  return probeResults.value[id];
}

/** Empty string when no probe yet (lets the template v-if on the text). */
function probeLabel(id: string): string {
  const p = probeFor(id);
  return p ? describeProbe(p) : '';
}

/** Probe result rendered as one line (shared by saved rows and the draft). */
function describeProbe(p: Probe): string {
  if (!p.reachable) return t('settings.snapshotTargets.probeUnreachable');
  return t('settings.snapshotTargets.probeOk', {
    count: p.snapshot_objects,
    fingerprint: p.fingerprint || '—',
  });
}

const draftProbeLabel = computed(() =>
  draftProbe.value ? describeProbe(draftProbe.value) : '',
);

function probeClass(id: string): string {
  const p = probeFor(id);
  return p?.reachable ? 'ok' : 'bad';
}

function isForeign(id: string): boolean {
  const p = probeFor(id);
  return !!p?.owner_instance && !p.owned_by_me;
}

async function probe(tg: Target) {
  busy.value = tg.id;
  const result = await makeRequest<Probe>(`admin/dr-backup/targets/${tg.id}/test`, 'POST', {});
  busy.value = '';
  if (result.status === 'success' && result.result) {
    probeResults.value = { ...probeResults.value, [tg.id]: result.result };
  } else {
    notifications.add({ type: 'error', title: t('common.errors.generic'), message: result.message });
  }
}

async function sync(tg: Target) {
  busy.value = tg.id;
  const result = await makeRequest(`admin/dr-backup/targets/${tg.id}/sync`, 'POST', {});
  busy.value = '';
  if (result.status === 'success') {
    notifications.add({ type: 'success', title: t('settings.snapshotTargets.syncDone') });
    await load();
  } else {
    notifications.add({ type: 'error', title: t('settings.snapshotTargets.syncFailed'), message: result.message });
  }
}

async function removeTarget(tg: Target) {
  busy.value = tg.id;
  // Gate first: an unclean target refuses deletion with the reason.
  const check = await makeRequest(
    `admin/dr-backup/targets/${tg.id}/delete-check`,
    'POST',
    {},
  );
  if (check.status !== 'success') {
    busy.value = '';
    notifications.add({
      type: 'error',
      title: t('settings.snapshotTargets.deleteBlocked'),
      message: check.message,
    });
    return;
  }
  const del = await makeRequest(`admin/dr-backup/targets/${tg.id}`, 'DELETE', {});
  busy.value = '';
  if (del.status === 'success') {
    notifications.add({ type: 'success', title: t('settings.snapshotTargets.deleted') });
    await load();
  } else {
    notifications.add({ type: 'error', title: t('common.errors.generic'), message: del.message });
  }
}

function startEdit(tg: Target) {
  editing.value = tg;
  Object.assign(form, {
    kind: tg.kind,
    name: tg.name,
    endpoint: tg.endpoint,
    bucket: tg.bucket,
    prefix: tg.prefix,
    access_key: tg.access_key, // already masked by the API
    secret_key: tg.secret_key,
    secure: tg.secure,
    keep_last: tg.keep_last,
    schedule: tg.schedule ?? '',
    enabled: tg.enabled,
    users: [...tg.users],
  });
  // An existing target's stored connection is already proven; a test is only
  // required again once a connection field is edited.
  draftProbe.value = null;
  testedFields.value = JSON.stringify({
    kind: tg.kind,
    endpoint: tg.endpoint.trim(),
    bucket: tg.bucket.trim(),
    prefix: tg.prefix.trim(),
    access_key: tg.access_key,
    secret_key: tg.secret_key,
    secure: tg.secure,
    region: 'us-east-1',
    provider: 'Other',
  });
}

function cancelEdit() {
  editing.value = null;
  formError.value = '';
  Object.assign(form, {
    kind: 's3',
    name: '',
    endpoint: '',
    bucket: '',
    prefix: '',
    access_key: '',
    secret_key: '',
    secure: true,
    keep_last: 10,
    schedule: '',
    enabled: true,
    users: [],
    });
  draftProbe.value = null;
  testedFields.value = null;
}

/** Probes the unsaved connection fields; a pass unlocks the save button. */
async function testDraft() {
  testing.value = true;
  formError.value = '';
  const body = {
    id: editing.value?.id ?? null,
    target: {
      kind: form.kind,
      name: form.name.trim() || 'draft',
      endpoint: form.endpoint.trim(),
      bucket: form.bucket.trim(),
      prefix: form.prefix.trim(),
          access_key: form.access_key,
      secret_key: form.secret_key,
      secure: form.secure,
      region: form.region,
      provider: form.provider,
      keep_last: form.keep_last,
      max_age_days: 0,
      schedule: form.schedule.trim(),
      enabled: form.enabled,
      users: form.users,
    },
  };
  const result = await makeRequest<Probe>('admin/dr-backup/targets/probe-draft', 'POST', body);
  testing.value = false;
  if (result.status === 'success' && result.result) {
    draftProbe.value = result.result;
    testedFields.value = result.result.reachable
      ? JSON.stringify(connectionFields.value)
      : null;
    if (!result.result.reachable) {
      formError.value = t('settings.snapshotTargets.probeUnreachable');
    } else if (result.result.owner_instance && !result.result.owned_by_me) {
      notifications.add({
        type: 'warning',
        title: t('settings.snapshotTargets.foreignOwner', { instance: result.result.owner_instance }),
      });
    }
  } else {
    formError.value = result.message || t('settings.snapshotTargets.probeUnreachable');
  }
}

/** Explicit takeover of a scope claimed by another deployment. */
async function takeOwnership(tg: Target) {
  busy.value = tg.id;
  const result = await makeRequest<{ instance_id: string; previous_instance?: string }>(
    `admin/dr-backup/targets/${tg.id}/take-ownership`,
    'POST',
    {},
  );
  busy.value = '';
  if (result.status === 'success') {
    notifications.add({
      type: 'success',
      title: t('settings.snapshotTargets.takeoverDone', {
        instance: result.result?.instance_id ?? '',
      }),
    });
    await load();
  } else {
    notifications.add({ type: 'error', title: t('common.errors.generic'), message: result.message });
  }
}

async function save() {
  saving.value = true;
  formError.value = '';
  const body = {
    kind: form.kind,
    name: form.name.trim(),
    endpoint: form.endpoint.trim(),
    bucket: form.bucket.trim(),
    prefix: form.prefix.trim(),
    access_key: form.access_key,
    secret_key: form.secret_key,
    secure: form.secure,
    region: 'us-east-1',
    provider: 'Other',
    keep_last: form.keep_last,
    max_age_days: 0,
    schedule: form.schedule.trim(),
    enabled: form.enabled,
    users: form.users,
      // Proves to the server that these exact connection fields were tested.
    connection_token: draftProbe.value?.connection_token ?? null,
  };
  const result = editing.value
    ? await makeRequest(`admin/dr-backup/targets/${editing.value.id}`, 'PUT', body)
    : await makeRequest('admin/dr-backup/targets', 'POST', body);
  saving.value = false;
  if (result.status === 'success') {
    notifications.add({ type: 'success', title: t('settings.snapshotTargets.saved') });
    cancelEdit();
    await load();
  } else {
    formError.value = result.message || t('settings.snapshotTargets.saveFailed');
  }
}
</script>

<style scoped lang="scss">
.main {
  max-width: 760px;
  margin: auto;
}

.target-list {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  margin-bottom: 1.5rem;
}

.target-card {
  padding: 0.75rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  text-align: left;
}

.target-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 0.5rem;
}

.target-title {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.9rem;
}

.kind-badge {
  padding: 0.1rem 0.4rem;
  border-radius: var(--radius-xs);
  font-size: 0.65rem;
  font-weight: 700;
  color: var(--text-secondary);
  background: var(--surface-raised);
}

.endpoint {
  font-size: 0.75rem;
  color: var(--text-secondary);
}

.status {
  font-size: 0.7rem;

  &.on {
    color: var(--green);
  }

  &.off {
    color: var(--text-secondary);
  }
}

.meta {
  margin: 0.35rem 0;
  font-size: 0.75rem;
  color: var(--text-secondary);
}

.row-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  margin-top: 0.5rem;
}

.probe {
  margin-top: 0.5rem;
  padding: 0.35rem 0.5rem;
  border-radius: var(--radius-xs);
  font-size: 0.72rem;

  &.ok {
    color: var(--green);
    background: var(--green-bg);
  }

  &.bad {
    color: var(--red);
    background: var(--red-bg);
  }
}

.options-section {
  margin-bottom: 1.5rem;

  h3 {
    margin-bottom: 0.75rem;
    font-size: 1rem;
    color: var(--text-body);
  }
}

.form-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 0.6rem;

  label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.78rem;
    color: var(--text-secondary);
  }

  input[type='text'],
  input[type='password'],
  input[type='number'] {
    padding: 0.4rem 0.6rem;
    font-size: 0.82rem;
  }
}

.checkbox-row {
  flex-direction: row !important;
  align-items: center;
  gap: 0.4rem !important;

  // The global form styles give every input width:100%, which turns a
  // checkbox into a full-width box centred in its cell. Keep it box-sized.
  input[type='checkbox'] {
    flex: 0 0 auto;
    width: auto;
    min-width: 0;
    margin: 0;
    padding: 0;
  }
}

.users-box {
  margin-top: 1rem;
  padding: 0.6rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);

  legend {
    padding: 0 0.35rem;
    font-size: 0.78rem;
    color: var(--text-secondary);
  }
}

.user-row {
  display: flex;
  justify-content: flex-start;
  align-items: center;
  gap: 0.45rem;
  padding: 0.2rem 0;
  font-size: 0.8rem;

  input[type='checkbox'] {
    flex: 0 0 auto;
    width: auto;
    min-width: 0;
    margin: 0;
    padding: 0;
  }

  // Username hugs the checkbox; the email trails it.
  span:first-of-type {
    flex: 0 0 auto;
  }
}

.user-mail {
  font-size: 0.72rem;
  color: var(--text-secondary);
}

.hint {
  margin: 0.4rem 0;
  font-size: 0.74rem;
  color: var(--text-secondary);
}

.error-message {
  padding: 0.6rem;
  border-radius: var(--radius-sm);
  color: var(--error);
  background: var(--error-bg);
}
</style>
