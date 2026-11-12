<template>
  <div>
    <h2 class="page-title">{{ t('settings.mcp.title') }}</h2>
    <p class="page-subtitle">{{ t('settings.mcp.subtitle') }}</p>

    <div class="main">
      <!-- ------------------------------------------------- tokens -->
      <section class="block">
        <h3 class="block-title">{{ t('settings.mcp.tokensTitle') }}</h3>
        <p v-if="loadError" class="error-message">{{ loadError }}</p>

        <div class="create-row">
          <input
            v-model="newName"
            type="text"
            maxlength="60"
            :placeholder="t('settings.mcp.namePlaceholder')"
            @keydown.enter="create"
          />
          <AppButton type="primary" :disabled="creating" @click="create">
            {{ t('settings.mcp.create') }}
          </AppButton>
        </div>
        <p v-if="!newName.trim()" class="hint">
          {{ t('settings.mcp.defaultNameHint', { name: defaultName() }) }}
        </p>

        <!-- one-time plaintext -->
        <div v-if="minted" class="minted">
          <p class="minted-title">{{ t('settings.mcp.mintedTitle') }}</p>
          <p class="hint">{{ t('settings.mcp.mintedHint') }}</p>
          <div class="minted-row">
            <code class="token">{{ minted }}</code>
            <AppButton type="secondary" small @click="copy(minted, 'token')">
              {{ copied === 'token' ? t('settings.mcp.copied') : t('settings.mcp.copy') }}
            </AppButton>
          </div>
          <AppButton type="secondary" small @click="minted = ''">
            {{ t('settings.mcp.dismiss') }}
          </AppButton>
        </div>

        <div v-if="tokens.length > 0" class="token-list">
          <div v-for="tk in tokens" :key="tk.id" class="token-card">
            <div class="token-head">
              <strong>{{ tk.name }}</strong>
              <AppButton type="danger" small :disabled="busy === tk.id" @click="revoke(tk)">
                {{ t('settings.mcp.revoke') }}
              </AppButton>
            </div>
            <p class="meta">
              {{ t('settings.mcp.created', { date: formatDate(tk.created_timestamp) }) }}
              · {{ lastUsedLabel(tk) }}
            </p>
          </div>
        </div>
        <p v-else-if="!loading" class="hint">{{ t('settings.mcp.empty') }}</p>
      </section>

      <!-- ------------------------------------------------ connect -->
      <section class="block">
        <h3 class="block-title">{{ t('settings.mcp.connectTitle') }}</h3>
        <p class="hint">{{ t('settings.mcp.connectHint') }}</p>

        <div class="endpoint">
          <span class="endpoint-label">{{ t('settings.mcp.endpoint') }}</span>
          <code>{{ endpoint }}</code>
          <AppButton type="secondary" small @click="copy(endpoint, 'endpoint')">
            {{ t('settings.mcp.copy') }}
          </AppButton>
        </div>

        <div class="snippet">
          <div class="snippet-head">
            <span>{{ t('settings.mcp.clientJson') }}</span>
            <AppButton type="secondary" small @click="copy(jsonSnippet, 'json')">
              {{ copied === 'json' ? t('settings.mcp.copied') : t('settings.mcp.copy') }}
            </AppButton>
          </div>
          <pre><code class="hljs">{{ jsonSnippet }}</code></pre>
        </div>

        <div class="snippet">
          <div class="snippet-head">
            <span>{{ t('settings.mcp.clientCli') }}</span>
            <AppButton type="secondary" small @click="copy(cliSnippet, 'cli')">
              {{ copied === 'cli' ? t('settings.mcp.copied') : t('settings.mcp.copy') }}
            </AppButton>
          </div>
          <pre><code class="hljs">{{ cliSnippet }}</code></pre>
        </div>

        <p v-if="!minted" class="hint">{{ t('settings.mcp.tokenPlaceholderHint') }}</p>
        <p v-else class="hint ok">{{ t('settings.mcp.tokenIncluded') }}</p>
      </section>

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
const copied = ref('');
const origin = ref('');

// Endpoint and snippets are built from the origin the user is actually on, so
// a copy-paste works on localhost and on a public deployment alike.
const endpoint = computed(() => `${origin.value}/api/mcp`);
const snippetToken = computed(() => minted.value || 'pgm_YOUR_TOKEN');

const jsonSnippet = computed(() =>
  JSON.stringify(
    {
      mcpServers: {
        pergame: {
          type: 'http',
          url: endpoint.value,
          headers: { Authorization: `Bearer ${snippetToken.value}` },
        },
      },
    },
    null,
    2,
  ),
);

const cliSnippet = computed(
  () =>
    `claude mcp add --transport http pergame ${endpoint.value} \\\n  --header "Authorization: Bearer ${snippetToken.value}"`,
);

const formatDate = (ms: number) =>
  new Date(ms).toLocaleString(locale.value === 'en' ? 'en-GB' : locale.value);

const lastUsedLabel = (tk: ApiToken) =>
  tk.last_used_timestamp
    ? t('settings.mcp.lastUsed', { date: formatDate(tk.last_used_timestamp) })
    : t('settings.mcp.neverUsed');

async function copy(value: string, key: string) {
  try {
    await navigator.clipboard.writeText(value);
    copied.value = key;
    setTimeout(() => {
      if (copied.value === key) copied.value = '';
    }, 2000);
  } catch {
    notifications.add({
      type: 'error',
      title: t('settings.mcp.copyFailed'),
      message: value,
    });
  }
}

async function load() {
  loading.value = true;
  loadError.value = '';
  const response = await makeRequest<ApiToken[]>('users/@me/api-tokens', 'GET');
  if (response.status === 'success' && response.result) {
    tokens.value = response.result;
  } else {
    loadError.value = response.message || t('settings.mcp.loadFailed');
  }
  loading.value = false;
}

const defaultName = () => `mcp-${new Date().toISOString().slice(0, 10)}`;

async function create() {
  if (creating.value) return;
  // An unnamed token still gets a stable label, so the button never dies on an
  // empty field (the old disabled state was invisible and simply did nothing).
  const name = newName.value.trim() || defaultName();
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
        title: t('settings.mcp.createdTitle'),
        message: name,
      });
      await load();
    } else {
      notifications.add({
        type: 'error',
        title: t('settings.mcp.createFailed'),
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
      if (minted.value && !tokens.value.length) minted.value = '';
      notifications.add({
        type: 'success',
        title: t('settings.mcp.revokedTitle'),
        message: tk.name,
      });
    } else {
      notifications.add({
        type: 'error',
        title: t('settings.mcp.revokeFailed'),
        message: response.message,
      });
    }
  } finally {
    busy.value = null;
  }
}

onMounted(() => {
  origin.value = window.location.origin;
  load();
});
</script>

<style scoped lang="scss">
.main {
  display: flex;
  flex-direction: column;
  gap: 1.75rem;
  max-width: 760px;
}

.block {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.block-title {
  padding-bottom: 0.5rem;
  border-bottom: 1px solid var(--border);
  font-size: 1rem;
  font-weight: 600;
}

.endpoint {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.5rem;

  .endpoint-label {
    font-size: 0.8125rem;
    color: var(--text-secondary);
  }

  code {
    flex: 1;
    min-width: 0;
    padding: 0.4rem 0.6rem;
    border-radius: 6px;
    font-size: 0.8125rem;
    background: var(--surface-secondary);
    overflow-wrap: anywhere;
  }
}

.snippet {
  border: 1px solid var(--border);
  border-radius: 8px;
  overflow: hidden;

  .snippet-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 0.75rem;
    border-bottom: 1px solid var(--border);
    font-size: 0.8125rem;
    color: var(--text-secondary);
    background: var(--surface-secondary);
  }

  // Global `pre` paints a dark background while `code` keeps the light-theme
  // text colour, so an unclassed block renders dark-on-dark. `.hljs` is the
  // class the rest of the app uses for that surface (markdown preview).
  pre {
    margin: 0;
  }

  code.hljs {
    padding: 0.75rem;
    font-size: 0.8125rem;
    line-height: 1.5;
  }
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

.hint.ok {
  color: var(--success, var(--text-secondary));
}
</style>
