<template>
  <section class="pdf-panel">
    <div class="pdf-panel-header">
      <span class="pdf-panel-title" :title="title">{{ title }}</span>
      <div class="pdf-panel-actions">
        <AppBtnIcon
          icon="download"
          :tooltip="t('nodes.pdf.downloadWithAnnotations')"
          :disabled="downloading"
          @click="downloadWithAnnotations"
        />
        <AppBtnIcon icon="close" :tooltip="t('common.actions.close')" @click="emit('close')" />
      </div>
    </div>
    <div v-if="loading" class="pdf-panel-loading">{{ t('common.status.loading') }}</div>
    <div v-else-if="error" class="pdf-panel-error">{{ error }}</div>
    <iframe
      v-show="!loading && !error"
      ref="frame"
      class="pdf-panel-frame"
      :src="viewerSrc"
      :title="title"
    />
    <div v-if="editor" class="pdf-panel-editor">
      <p class="pdf-editor-quote">{{ editor.quote }}</p>
      <input
        v-model="editor.comment"
        type="text"
        class="pdf-editor-comment"
        :placeholder="t('nodes.pdf.commentPlaceholder')"
      />
      <div class="pdf-editor-row">
        <span class="pdf-editor-colors">
          <button
            v-for="(c, i) in colors"
            :key="i"
            class="pdf-color-dot"
            :class="{ active: editor.color === i }"
            :style="{ background: c }"
            @click="editor.color = i"
          />
        </span>
        <span class="pdf-editor-buttons">
          <template v-if="editor.mode === 'create'">
            <AppButton type="secondary" small @click="cancelEditor">{{ t('common.actions.cancel') }}</AppButton>
            <AppButton type="primary" small :disabled="saving" @click="saveCreate">{{ t('common.actions.save') }}</AppButton>
          </template>
          <template v-else>
            <AppButton type="secondary" small @click="copyLink">{{ t('common.actions.copyLink') }}</AppButton>
            <AppButton v-if="!confirmingDelete" type="danger" small @click="confirmingDelete = true">
              {{ t('common.actions.delete') }}
            </AppButton>
            <AppButton v-else type="danger" small :disabled="saving" @click="removeAnnotation">
              {{ t('common.actions.confirm') }}
            </AppButton>
            <AppButton type="primary" small :disabled="saving" @click="saveEdit">{{ t('common.actions.save') }}</AppButton>
          </template>
        </span>
      </div>
      <div v-if="editor.mode === 'edit' && editorRefs.length" class="pdf-editor-refs">
        <span class="pdf-refs-label">{{ t('nodes.pdf.refs') }}</span>
        <a v-for="ref in editorRefs" :key="ref.id" href="#" @click.prevent="openRef(ref)">{{ ref.name }}</a>
      </div>
    </div>
    <div v-else-if="hint && !editor" class="pdf-panel-editor">
      <p class="pdf-editor-hint">{{ t('nodes.pdf.selectionHint') }}</p>
    </div>
  </section>
</template>

<script setup lang="ts">
import { attachPdfAnnotations, type PdfAnn, type PdfAnnBridge, type PdfSelection } from '~/helpers/pdf-annots';
import { PDF_ANNOT_COLORS, pdfViewerUrl } from '~/helpers/pdf-mode';
import { downloadPdfWithAnnotations } from '~/helpers/pdf-annot-export';
import { resolveNodeLink } from '~/helpers/node';
import { makeRequest } from '~/stores/_utils';
import type { Node, RefNode } from '~/stores';

const props = defineProps<{
  resourceId: string;
  fileUrl: string;
  title: string;
  page?: number;
  annotationId?: string | null;
}>();
const emit = defineEmits(['close']);

const nodesStore = useNodesStore();
const notifications = useNotifications();
const router = useRouter();
const { t } = useI18nT();

const colors = PDF_ANNOT_COLORS;
const colorOf = (i: number): string => colors[i] ?? colors[0]!;

const frame = ref<HTMLIFrameElement | null>(null);
const loading = ref(true);
const error = ref('');
const downloading = ref(false);
const saving = ref(false);
const confirmingDelete = ref(false);
const hint = ref(false);

interface EditorState {
  mode: 'create' | 'edit';
  quote: string;
  comment: string;
  color: number;
  selection: PdfSelection | null;
  annId: string | null;
}
const editor = ref<EditorState | null>(null);
const editorRefs = ref<RefNode[]>([]);
const annotations = ref<PdfAnn[]>([]);

let bridge: PdfAnnBridge | null = null;

const viewerSrc = computed(() => pdfViewerUrl(props.fileUrl, props.page));

const notifyError = (e: unknown) => {
  notifications.add({ type: 'error', title: t('common.status.error'), message: String(e) });
};

const reloadAnnotations = async (): Promise<void> => {
  const res = await makeRequest<PdfAnn[]>(`resources/${props.resourceId}/annotations`, 'GET', {});
  if (res.status !== 'success' || !res.result) throw new Error(res.message || 'annotations load failed');
  annotations.value = res.result;
  bridge?.setAnnotations(res.result);
};

const openEditorForSelection = () => {
  const sel = bridge?.getSelection() ?? null;
  if (!sel) {
    if (editor.value?.mode === 'create') editor.value = null;
    return;
  }
  editor.value = { mode: 'create', quote: sel.quote, comment: '', color: 0, selection: sel, annId: null };
  editorRefs.value = [];
  hint.value = false;
};

const openEditorForAnnotation = async (id: string): Promise<void> => {
  const ann = annotations.value.find(a => a.id === id);
  if (!ann) return;
  confirmingDelete.value = false;
  editor.value = { mode: 'edit', quote: ann.quote, comment: ann.comment, color: ann.color, selection: null, annId: id };
  editorRefs.value = [];
  try {
    const res = await makeRequest<RefNode[]>(`annotations/${id}/refs`, 'GET', {});
    if (res.status === 'success' && res.result) editorRefs.value = res.result;
  } catch {
    // Refs are a bonus; the editor works without them.
  }
};

const cancelEditor = () => {
  editor.value = null;
  confirmingDelete.value = false;
  bridge?.clearSelection();
};

const saveCreate = async () => {
  const ed = editor.value;
  if (!ed?.selection || saving.value) return;
  saving.value = true;
  try {
    const res = await makeRequest<PdfAnn>(`resources/${props.resourceId}/annotations`, 'POST', {
      page: ed.selection.page,
      quads: ed.selection.quads,
      quote: ed.selection.quote,
      comment: ed.comment,
      color: ed.color,
    });
    if (res.status !== 'success' || !res.result) throw new Error(res.message || 'annotation save failed');
    await reloadAnnotations();
    editor.value = null;
    bridge?.clearSelection();
  } catch (e) {
    notifyError(e);
  } finally {
    saving.value = false;
  }
};

const saveEdit = async () => {
  const ed = editor.value;
  if (!ed?.annId || saving.value) return;
  saving.value = true;
  try {
    const res = await makeRequest<PdfAnn>(`annotations/${ed.annId}`, 'PATCH', {
      comment: ed.comment,
      color: ed.color,
    });
    if (res.status !== 'success' || !res.result) throw new Error(res.message || 'annotation save failed');
    await reloadAnnotations();
  } catch (e) {
    notifyError(e);
  } finally {
    saving.value = false;
  }
};

const removeAnnotation = async () => {
  const ed = editor.value;
  if (!ed?.annId || saving.value) return;
  saving.value = true;
  try {
    const res = await makeRequest(`annotations/${ed.annId}`, 'DELETE', {});
    if (res.status !== 'success') throw new Error(res.message || 'annotation delete failed');
    await reloadAnnotations();
    editor.value = null;
    confirmingDelete.value = false;
  } catch (e) {
    notifyError(e);
  } finally {
    saving.value = false;
  }
};

const copyLink = async () => {
  const ed = editor.value;
  if (!ed?.annId) return;
  const label = ed.quote.length > 60 ? `${ed.quote.slice(0, 60)}…` : ed.quote;
  try {
    await navigator.clipboard.writeText(`[${label}](#annotation:${ed.annId})`);
  } catch (e) {
    notifyError(e);
  }
};

const openRef = (ref: RefNode) => {
  const target = nodesStore.getById(ref.id);
  router.push(resolveNodeLink((target ?? ref) as unknown as Node));
};

const downloadWithAnnotations = async () => {
  if (downloading.value) return;
  downloading.value = true;
  try {
    await downloadPdfWithAnnotations(props.fileUrl, props.title, annotations.value);
  } catch (e) {
    notifyError(e);
  } finally {
    downloading.value = false;
  }
};

const gotoRequested = () => {
  if (!props.annotationId) return;
  const ann = annotations.value.find(a => a.id === props.annotationId);
  if (ann) {
    bridge?.gotoAnnotation(ann);
    void openEditorForAnnotation(ann.id);
  }
};

watch(
  () => props.resourceId,
  async () => {
    bridge?.destroy();
    bridge = null;
    editor.value = null;
    error.value = '';
    loading.value = true;
    // Wait for the new document to commit navigation; attaching to the
    // dying window would bind a dead viewer.
    await waitForFrameLoad();
    await mountBridge();
  },
);

const waitForFrameLoad = async (): Promise<void> => {
  const el = frame.value;
  if (!el) return;
  await new Promise<void>(resolve => {
    const timer = window.setTimeout(resolve, 15000);
    el.addEventListener('load', () => {
      window.clearTimeout(timer);
      resolve();
    }, { once: true });
  });
};

watch(
  () => props.annotationId,
  () => gotoRequested(),
);

const mountBridge = async (): Promise<void> => {
  if (!frame.value) return;
  try {
    bridge = await attachPdfAnnotations(frame.value, {
      colorOf,
      onAnnotationClick: id => void openEditorForAnnotation(id),
      onSelectionChange: () => openEditorForSelection(),
    });
    await reloadAnnotations();
    hint.value = annotations.value.length === 0;
    gotoRequested();
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    loading.value = false;
  }
};

onMounted(() => {
  void mountBridge();
});

onUnmounted(() => {
  bridge?.destroy();
  bridge = null;
});
</script>

<style scoped lang="scss">
.pdf-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  border-left: 1px solid var(--border-color);
  background: var(--bg-color);

  .pdf-panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-bottom: 1px solid var(--border-color);
  }

  .pdf-panel-title {
    font-size: 13px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
    overflow: hidden;
  }

  .pdf-panel-actions {
    display: flex;
    flex-shrink: 0;
    gap: 4px;
  }

  .pdf-panel-loading,
  .pdf-panel-error {
    padding: 16px;
    font-size: 13px;
    color: var(--text-secondary);
  }

  .pdf-panel-frame {
    flex: 1;
    width: 100%;
    min-height: 0;
    border: 0;
    background: #fff;
  }

  .pdf-panel-editor {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-height: 40%;
    padding: 8px 10px;
    border-top: 1px solid var(--border-color);
    overflow-y: auto;
  }

  .pdf-editor-quote {
    display: -webkit-box;
    margin: 0;
    padding-left: 8px;
    border-left: 3px solid var(--primary-color);
    font-size: 12px;
    color: var(--text-secondary);
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
    overflow: hidden;
  }

  .pdf-editor-comment {
    width: 100%;
    padding: 6px 8px;
    border: 1px solid var(--border-color);
    border-radius: 6px;
    font-size: 13px;
    color: var(--text-color);
    background: var(--bg-color);
  }

  .pdf-editor-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
  }

  .pdf-editor-colors {
    display: flex;
    gap: 6px;
  }

  .pdf-color-dot {
    width: 18px;
    height: 18px;
    padding: 0;
    border: 2px solid transparent;
    border-radius: 50%;
    cursor: pointer;

    &.active {
      border-color: var(--text-color);
    }
  }

  .pdf-editor-buttons {
    display: flex;
    gap: 6px;
  }

  .pdf-editor-refs {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 8px;
    font-size: 12px;
  }

  .pdf-refs-label {
    color: var(--text-secondary);
  }

  .pdf-editor-hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-secondary);
  }
}
</style>
