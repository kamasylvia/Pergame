<template>
  <div>
    <h2 class="page-title">{{ t('settings.apiTokens.title') }}</h2>
    <p class="page-subtitle">{{ t('settings.apiTokens.subtitle') }}</p>

    <div class="main">
      <p v-if="loadError" class="error-message">{{ loadError }}</p>

      <!-- create -->
      <div class="create-row">
        <input
          v-model="newName"
          type="text"
          maxlength="60"
          :placeholder="t('settings.apiTokens.namePlaceholder')"
          @keydown.enter="create"
        />
        <AppButton type="primary" :disabled="creating || !newName.trim()" @click="create">
          {{ t('settings.apiTokens.create') }}
        </AppButton>
      </div>

      <!-- one-time plaintext -->
      <div v-if="minted" class="minted">
        <p class="minted-title">{{ t('settings.apiTokens.mintedTitle') }}</p>
        <p class="hint">{{ t('settings.apiTokens.mintedHint') }}</p>
        <div class="minted-row">
          <code class="token">{{ minted }}</code>
          <AppButton type="secondary" small @click="copyToken">
            {{ t('settings.apiTokens.copy') }}
          </AppButton>
        </div>
        <AppButton type="secondary" small @click="minted = ''">
          {{ t('settings.apiTokens.dismiss') }}
        </AppButton>
      </div>

      <!-- list -->
      <div v-if="tokens.length > 0" class="token-list">
        <div v-for="tk in tokens" :key="tk.id" class="token-card">
          <div class="token-head">
            <strong>{{ tk.name }}</strong>
            <AppButton type="danger" small :disabled="busy === tk.id" @click="revoke(tk)">
              {{ t('settings.apiTokens.revoke') }}
            </AppButton>
          </div>
          <p class="meta">
            {{ t('settings.apiTokens.created', { date: formatDate(tk.created_timestamp) }) }}
            · {{ lastUsedLabel(tk) }}
          </p>
        </div>
      </div>
      <p v-else-if="!loading" class="hint">{{ t('settings.apiTokens.empty') }}</p>

      <p class="hint endpoint">
        {{ t('settings.apiTokens.endpointHint') }} <code>/api/mcp</code>
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { makeRequest } from '~/stores/_utils';

interface ApiToken {
  id: string;
  name: string;
  created_timestamp: number;
  last_used_timestamp: number | null;
}

const { t, locale } = useI18nT();
const notifications = useNotifications();

const tokens = ref<ApiToken[]>([]);
const loading = ref(true);
const loadError = ref('');
const creating = ref(false);
const busy = ref<string | null>(null);
const newName = ref('');
const minted = ref('');

const formatDate = (ms: number) =>
  new Date(ms).toLocaleString(locale.value === 'en' ? 'en-GB' : locale.value);

const lastUsedLabel = (tk: ApiToken) =>
  tk.last_used_timestamp
    ? t('settings.apiTokens.lastUsed', { date: formatDate(tk.last_used_timestamp) })
    : t('settings.apiTokens.neverUsed');

async function load() {
  loading.value = true;
  loadError.value = '';
  const response = await makeRequest<ApiToken[]>('users/@me/api-tokens', 'GET');
  if (response.status === 'success' && response.result) {
    tokens.value = response.result;
  } else {
    loadError.value = response.message || t('settings.apiTokens.loadFailed');
  }
  loading.value = false;
}

async function create() {
  const name = newName.value.trim();
  if (!name || creating.value) return;
  creating.value = true;
  try {
    const response = await makeRequest<{ id: string; name: string; token: string }>(
      'users/@me/api-tokens',
      'POST',
      { name },
    );
    if (response.status === 'success' && response.result) {
      // The plaintext is shown exactly once — never returned by the list API.
      minted.value = response.result.token;
      newName.value = '';
      notifications.add({
        type: 'success',
        title: t('settings.apiTokens.createdTitle'),
        message: name,
      });
      await load();
    } else {
      notifications.add({
        type: 'error',
        title: t('settings.apiTokens.createFailed'),
        message: response.message,
      });
    }
  } finally {
    creating.value = false;
  }
}

async function revoke(tk: ApiToken) {
  busy.value = tk.id;
  try {
    const response = await makeRequest<{ id: string }>(`users/@me/api-tokens/${tk.id}`, 'DELETE');
    if (response.status === 'success') {
      tokens.value = tokens.value.filter(item => item.id !== tk.id);
      notifications.add({
        type: 'success',
        title: t('settings.apiTokens.revokedTitle'),
        message: tk.name,
      });
    } else {
      notifications.add({
        type: 'error',
        title: t('settings.apiTokens.revokeFailed'),
        message: response.message,
      });
    }
  } finally {
    busy.value = null;
  }
}

const copyToken = () => {
  if (!minted.value) return;
  navigator.clipboard.writeText(minted.value);
  notifications.add({
    type: 'success',
    title: t('common.notifications.copiedTitle'),
    message: t('common.notifications.copiedMessage'),
  });
};

onMounted(load);
</script>

<style scoped lang="scss">
.main {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  max-width: 720px;
}

.create-row {
  display: flex;
  gap: 0.5rem;

  input {
    flex: 1;
    padding: 0.5rem 0.75rem;
    font-size: 0.875rem;
  }
}

.minted {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding: 0.75rem;
  border: 1px solid var(--border-warning, var(--border));
  border-radius: 8px;

  .minted-title {
    font-weight: 600;
  }

  .minted-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .token {
    flex: 1;
    padding: 0.5rem;
    border-radius: 6px;
    font-size: 0.8125rem;
    background: var(--surface-secondary);
    overflow-wrap: anywhere;
  }
}

.token-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.token-card {
  padding: 0.75rem;
  border: 1px solid var(--border);
  border-radius: 8px;

  .token-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.5rem;
  }

  .meta {
    margin-top: 0.35rem;
    font-size: 0.8125rem;
    color: var(--text-secondary);
  }
}

.endpoint code {
  padding: 0.1rem 0.35rem;
  border-radius: 4px;
  background: var(--surface-secondary);
}
</style>
