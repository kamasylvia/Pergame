<template>
  <section class="code-panel">
    <div class="code-panel-header">
      <span class="code-panel-title" :title="title">{{ title }}</span>
      <div class="code-panel-actions">
        <AppBtnIcon icon="copy" :tooltip="t('common.actions.copy')" :disabled="!code" @click="copyCode" />
        <AppBtnIcon icon="download" :tooltip="t('common.actions.download')" @click="download" />
        <AppBtnIcon icon="close" :tooltip="t('common.actions.close')" @click="emit('close')" />
      </div>
    </div>
    <div v-if="loading" class="code-panel-loading">{{ t('common.status.loading') }}</div>
    <div v-else-if="error" class="code-panel-error">
      <p>{{ error }}</p>
      <AppButton type="secondary" small @click="download">{{ t('common.actions.download') }}</AppButton>
    </div>
    <!-- eslint-disable-next-line vue/no-v-html -->
    <pre v-else class="code-panel-body"><code class="hljs" :class="`language-${lang}`" v-html="html" /></pre>
  </section>
</template>

<script setup lang="ts">
import { fetchCodeText, highlightCode } from '~/helpers/code-embed';

const props = defineProps<{
  fileUrl: string;
  title: string;
}>();
const emit = defineEmits<{ (e: 'close'): void }>();

const { t } = useI18nT();
const notifications = useNotifications();

const loading = ref(true);
const error = ref('');
const code = ref('');
const lang = ref('plaintext');
const html = ref('');

onMounted(async () => {
  try {
    const text = await fetchCodeText(props.fileUrl);
    code.value = text;
    const out = highlightCode(props.title, text);
    lang.value = out.lang;
    html.value = out.html;
  } catch (e) {
    const reason = e instanceof Error ? e.message : '';
    error.value =
      reason === 'too large'
        ? t('nodes.code.tooLarge')
        : reason === 'binary'
          ? t('nodes.code.binary')
          : t('nodes.code.loadFailed');
  } finally {
    loading.value = false;
  }
});

async function copyCode() {
  try {
    await navigator.clipboard.writeText(code.value);
    notifications.add({ type: 'success', title: t('nodes.code.copied') });
  } catch {
    notifications.add({ type: 'error', title: t('common.errors.generic') });
  }
}
function download() {
  window.open(`${props.fileUrl}${props.fileUrl.includes('?') ? '&' : '?'}download=true`, '_blank', 'noopener');
}
</script>

<style scoped lang="scss">
.code-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface-base);
  overflow: hidden;
}

.code-panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  padding: 0.5rem 0.75rem;
  border-bottom: 1px solid var(--border);
}

.code-panel-title {
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
  overflow: hidden;
}

.code-panel-actions {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  gap: 4px;
}

.code-panel-loading,
.code-panel-error {
  padding: 1.5rem;
  color: var(--text-secondary);
}

.code-panel-error {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.75rem;
}

.code-panel-body {
  flex: 1;
  margin: 0;
  padding: 0;
  font-size: 0.85rem;
  // The inner code.hljs only covers its content height; paint the pre
  // itself so short files don't leave a gap below the code.
  background: var(--code-bg);
  overflow: auto;
  scrollbar-color: rgb(127 127 127 / 45%) var(--code-bg);
  // Scrollbar gutter: element backgrounds don't paint under classic
  // scrollbars, and transparent there reveals the white section beneath
  // (the pale bottom-right strip). Paint the gutter code-bg instead.
  scrollbar-width: thin;

  // Single scroll container on the pre (doc-block parity).
  code.hljs {
    overflow-x: visible;
  }

  &::-webkit-scrollbar {
    width: 8px;
    height: 8px;
  }

  &::-webkit-scrollbar-thumb {
    border-radius: 4px;
    background: rgb(127 127 127 / 45%);
  }

  &::-webkit-scrollbar-track,
  &::-webkit-scrollbar-corner {
    background: var(--code-bg);
  }
}
</style>
