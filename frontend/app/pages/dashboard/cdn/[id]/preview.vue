<template>
  <div class="page-card">
    <template v-if="resource">
      <Teleport to="#navbar-title"
        >{{ t('cdn.preview.title') }} • <span class="page-subtitle">{{ resource.name }}</span></Teleport
      >
      <Teleport to="#navbar-actions">
        <AppSelect v-if="isPdfFile(mimeType)" v-model="zoom" :items="PDF_SCALES" :searchable="false" label="Scale" style="width: 200px" />
        <AppBtnIcon nav icon="edit" :to="`/dashboard/cdn/${resource.id}`" :tooltip="t('common.actions.edit')" />
        <AppBtnIcon v-if="resource.metadata?.drawio" nav icon="format/diagrams" :tooltip="t('common.actions.edit')" @click="openDrawioEditor" />
        <AppBtnIcon nav icon="copy" :tooltip="t('common.actions.copyLink')" @click="copyLink" />
        <AppBtnIcon nav :href="resourceURL(resource, true)" download icon="download" :tooltip="t('common.actions.download')" />
        <AppBtnIcon nav icon="delete" :tooltip="t('common.actions.delete')" @click="openDeleteModal" />
      </Teleport>
      <div ref="previewElement" class="preview">
        <object v-if="resource.metadata?.drawio" :data="resourceURL(resource)" alt="Preview" />
        <img v-else-if="isImageFile(mimeType)" :src="resourceURL(resource)" crossorigin="use-credentials" alt="Preview" />
        <LazyPDFViewer v-else-if="isPdfFile(mimeType)" :src="resourceURL(resource)" :zoom="zoom" />
        <video v-else-if="isVideoFile(mimeType)" :src="resourceURL(resource)" crossorigin="use-credentials" controls />
        <audio v-else-if="isAudioFile(mimeType)" :src="resourceURL(resource)" crossorigin="use-credentials" controls />
        <pre v-else-if="isCodeFile(resource.name || '')" class="code-preview"><code v-html="codeHtml"></code></pre>
        <div v-else class="no-preview">
          <p>{{ t('cdn.preview.unavailable') }}</p>
          <p>
            <NuxtLink :href="resourceURL(resource, true)" download rel="noopener">
              <AppButton type="primary">{{ t('cdn.actions.download') }}</AppButton>
            </NuxtLink>
          </p>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import DeleteNodeModal from '~/components/Node/Modals/Delete.vue';
import DrawioEditorModal from '~/components/Node/Modals/DrawioEditor.vue';
import { PDF_SCALES } from '~/helpers/constants';
import { isImageFile, isPdfFile, isVideoFile, isAudioFile, isCodeFile, codeLanguage } from '~/helpers/resources';
import hljs from 'highlight.js/lib/common';
import { rerenderImages } from '~/helpers/DOM';

definePageMeta({ breadcrumb: { i18n: 'common.actions.preview' } });

const nodesStore = useNodesStore();

const { t } = useI18nT();
const { resourceURL } = useApi();
const modals = useModal();
const route = useRoute();

const previewElement = ref<HTMLElement | null>();
const zoom = ref<(typeof PDF_SCALES)[number]['id']>('automatic_zoom');

const resource = computed(() => nodesStore.getById(route.params.id as string));
const mimeType = computed(() => resource.value?.metadata?.filetype || '');

// Code/text files render inline (256 KiB cap mirrors code-embed.ts).
const codeHtml = ref('');
watchEffect(async () => {
  codeHtml.value = '';
  const node = resource.value;
  if (!node || !isCodeFile(node.name || '')) return;
  try {
    const resp = await fetch(resourceURL(node), { credentials: 'include' });
    if (!resp.ok) return;
    const text = await resp.text();
    if (text.length > 256 * 1024 || (text.length > 0 && text.includes('\uFFFD'))) return;
    const lang = codeLanguage(node.name || '');
    codeHtml.value = lang && hljs.getLanguage(lang) ? hljs.highlight(text, { language: lang }).value : text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  } catch {
    codeHtml.value = '';
  }
});

// Actions
const copyLink = () => navigator.clipboard.writeText(resourceURL(resource.value!));

const openDrawioEditor = () => {
  if (!resource.value) return;
  modals.add(
    new Modal(shallowRef(DrawioEditorModal), {
      props: {
        node: resource.value,
      },
      onClose: () => rerenderImages(previewElement.value!),
      size: 'large',
      noPadding: true,
    }),
  );
};
const openDeleteModal = () => {
  if (!resource.value) return;
  modals.add(new Modal(shallowRef(DeleteNodeModal), { props: { node: resource.value, redirectTo: '/dashboard/cdn' }, size: 'small' }));
};

// Shortcuts
function handleKeydown(event: KeyboardEvent) {
  if (event.key !== 'Delete') return;
  event.preventDefault();
  openDeleteModal();
}

onMounted(() => {
  document.addEventListener('keydown', handleKeydown);
});

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeydown);
});
</script>
<style scoped lang="scss">
header {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  align-items: center;
}

.preview {
  display: flex;
  flex: 1;
  justify-content: center;
  overflow: hidden;

  img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }
}

.code-preview {
  width: 100%;
  margin: 0;
  padding: 16px;
  overflow: auto;
  text-align: left;
}

.no-preview {
  display: flex;
  flex-direction: column;
  text-align: center;
}
</style>
