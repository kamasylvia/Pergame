<template>
  <div class="orphan-preview-modal">
    <h3 class="orphan-preview-title">{{ fileKey }}</h3>
    <div v-if="loading" class="orphan-preview-loading">{{ t('cdn.orphans.loading') }}</div>
    <div v-else-if="error" class="orphan-preview-error">{{ error }}</div>
    <div v-else class="orphan-preview-body">
      <img v-if="kind === 'image'" :src="objectUrl ?? undefined" alt="Preview" />
      <video v-else-if="kind === 'video'" :src="objectUrl ?? undefined" controls />
      <audio v-else-if="kind === 'audio'" :src="objectUrl ?? undefined" controls />
      <LazyPDFViewer v-else-if="kind === 'pdf' && objectUrl" :src="objectUrl" />
      <pre v-else-if="kind === 'text'">{{ textContent }}</pre>
      <p v-else>{{ t('cdn.preview.unavailable') }}</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { isAudioFile, isImageFile, isPdfFile, isVideoFile } from '~/helpers/resources';

const props = defineProps<{ fileKey: string }>();

const { t } = useI18nT();

const loading = ref(true);
const error = ref('');
const kind = ref<'image' | 'video' | 'audio' | 'pdf' | 'text' | 'other'>('other');
const objectUrl = ref<string | null>(null);
const textContent = ref('');

onMounted(async () => {
  try {
    const resp = await fetch(`/api/resources/orphans/preview?key=${encodeURIComponent(props.fileKey)}`);
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
    const mime = resp.headers.get('Content-Type') ?? '';
    const blob = await resp.blob();
    if (isImageFile(mime)) kind.value = 'image';
    else if (isVideoFile(mime)) kind.value = 'video';
    else if (isAudioFile(mime)) kind.value = 'audio';
    else if (isPdfFile(mime)) kind.value = 'pdf';
    else if (mime.startsWith('text/')) {
      kind.value = 'text';
      textContent.value = (await blob.text()).slice(0, 20000);
    } else if (blob.size > 0 && blob.size < 65536) {
      // Extension-less blobs (content hashes): sniff for printable text.
      const sample = await blob.text();
      const printable = [...sample].filter(c => c === '\n' || c === '\r' || c === '\t' || (c >= ' ' && c <= '~') || (c.codePointAt(0) ?? 0) > 127).length;
      if (sample.length > 0 && printable / sample.length > 0.9) {
        kind.value = 'text';
        textContent.value = sample.slice(0, 20000);
      } else kind.value = 'other';
    } else kind.value = 'other';
    if (kind.value !== 'text') objectUrl.value = URL.createObjectURL(blob);
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    loading.value = false;
  }
});

onUnmounted(() => {
  if (objectUrl.value) URL.revokeObjectURL(objectUrl.value);
});
</script>

<style scoped lang="scss">
.orphan-preview-modal {
  display: flex;
  flex-direction: column;
  gap: 12px;
  max-height: 80vh;
}

.orphan-preview-title {
  font-size: 13px;
  word-break: break-all;
  color: var(--text-secondary);
}

.orphan-preview-body {
  overflow: auto;

  img,
  video {
    max-width: 100%;
  }

  audio {
    width: 100%;
  }

  pre {
    max-height: 60vh;
    overflow: auto;
    white-space: pre-wrap;
  }
}
</style>
