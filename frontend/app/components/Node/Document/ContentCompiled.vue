<template>
  <div v-if="properties.length" class="doc-properties">
    <div v-for="prop in properties" :key="prop.key" class="doc-property">
      <span class="doc-property-key">{{ prop.key }}</span>
      <span class="doc-property-value">{{ prop.value }}</span>
    </div>
  </div>
  <!-- eslint-disable vue/no-v-html -->
  <article
    ref="rootElement"
    :class="['markdown-preview', `${theme}-theme`, `document-content`, { numbering: documentTitlesNumbering }]"
    :style="{ fontSize: documentFontSize + 'px', fontFamily: documentFontFamily, lineHeight: documentLineHeight }"
    @change="handleCheckboxChange"
    @click="handleClick"
    v-html="html"
  />
</template>

<script setup lang="ts">
import compile from '~/helpers/markdown';
import { parseFrontmatter } from '~/helpers/markdown/frontmatter';
import { rerenderImages } from '~/helpers/DOM';
import { renderMermaidDiagrams } from '~/helpers/mermaid-render';
import { renderExtEmbeds } from '~/helpers/markdown/embeds';
import { renderAbcBlocks } from '~/helpers/abc-render';
import { renderDataviewBlocks } from '~/helpers/dataview-render';
import { renderCodeEmbeds } from '~/helpers/code-embed';
import { resolveNodeLink } from '~/helpers/node';
import { createInlinePdf, type PdfPanelPayload } from '~/helpers/pdf-inline';
import { PDF_ANNOT_COLORS, resolvePdfMode } from '~/helpers/pdf-mode';
import type { PdfAnn } from '~/helpers/pdf-annots';
import { makeRequest } from '~/stores/_utils';
import type { Node } from '~/stores';

const props = defineProps<{ node?: Partial<Node> }>();

const preferences = usePreferencesStore();
const nodesStore = useNodesStore();
const router = useRouter();
const colorMode = useColorMode();
const notifications = useNotifications();
const { t } = useI18nT();

// Provided by the doc page; absent on surfaces without a side panel
// (public docs), where PDF/annotation links open a new tab instead.
const openPdfPanel = inject<((p: PdfPanelPayload) => void) | undefined>('openPdfPanel', undefined);

const pdfDefault = preferences.get('pdfPreviewDefault');
const pdfMode = computed(() => resolvePdfMode(props.node, pdfDefault.value ?? 'side'));

const theme = computed(() => {
  if (props.node?.theme) return props.node.theme;
  return preferences.get('theme').value;
});
const documentFontSize = preferences.get('documentFontSize');
const documentFontFamily = preferences.get('documentFontFamily');
const documentLineHeight = preferences.get('documentLineHeight');
const documentTitlesNumbering = preferences.get('documentTitlesNumbering');

const rootElement = ref<HTMLElement>();

// Documents written by an API client (MCP `create_note`, imports) carry the
// markdown source but no compiled HTML — the editor only compiles on save.
// Rendering straight from the source keeps such a note readable instead of
// blank, without re-compiling what the editor already produced.
const frontmatter = computed(() => parseFrontmatter(props.node?.content ?? ''));
const properties = computed(() => frontmatter.value.properties);
const html = computed(() => {
  if (props.node?.content_compiled?.trim()) return props.node.content_compiled;
  if (!props.node?.content?.trim()) return '';
  return compile(frontmatter.value.content, id => nodesStore.getById(id)?.name);
});

defineExpose({ rootElement });

const updateDocumentContent = debounce(() => {
  // Content-only: never moves rows, skip the directory re-pulls.
  nodesStore.update(nodesStore.nodes.get(props.node!.id!) as Node, { skipRefresh: true });
}, 1000);

const updateMarkdownCheckbox = (markdown: string, indexToTarget: number, shouldCheck: boolean): string => {
  let currentIndex = 0;

  return markdown.replace(/^([ \t]*[-*+]\s+\[)([ xX]?)(\].*)/gm, (match, before, status, after) => {
    if (currentIndex === indexToTarget) {
      currentIndex++;
      const newStatus = shouldCheck ? 'x' : ' ';
      return `${before}${newStatus}${after}`;
    }
    currentIndex++;
    return match;
  });
};

// Handle the "Copy code" button click event
const handleCodeCopyClick = async (e: MouseEvent) => {
  const btn = (e.target as HTMLElement)?.closest('.code-copy-btn') as HTMLButtonElement | null;
  if (!btn || btn.classList.contains('copied')) return;

  const wrapper = btn.closest('.code-block-wrapper');
  const codeEl = wrapper?.querySelector('pre code');
  if (!codeEl) return;

  const codeToCopy = codeEl.textContent || '';

  let success = false;

  if (navigator.clipboard && window.isSecureContext) {
    try {
      await navigator.clipboard.writeText(codeToCopy);
      success = true;
    } catch (err) {
      console.warn('Cannot copy text to clipboard', err);
    }
  }

  if (success) {
    btn.classList.add('copied');
    setTimeout(() => {
      btn.classList.remove('copied');
    }, 1500);
  }
};

// Expand/collapse a long code block (rendered by the code-block plugin for
// blocks over the threshold). Toggling the wrapper class is all it takes —
// max-height and the label swap live in CSS.
const handleCodeToggleClick = (e: MouseEvent) => {
  const btn = (e.target as HTMLElement)?.closest('.code-toggle-btn') as HTMLButtonElement | null;
  if (!btn) return;
  const wrapper = btn.closest('.code-block-wrapper');
  if (!wrapper) return;
  const collapsed = wrapper.classList.toggle('collapsed');
  btn.setAttribute('aria-expanded', collapsed ? 'false' : 'true');
};

// Plain PDF links to our own resources open the side panel (or a new tab
// where no panel exists). Named URLs match by extension; legacy bare
// /file URLs resolve the stored mime through the nodes store.
const handlePdfLinkClick = (event: MouseEvent) => {
  if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
  const link = (event.target as HTMLElement).closest<HTMLAnchorElement>('a[href]');
  if (!link || link.hasAttribute('data-internal-link') || link.hasAttribute('data-annotation-link')) return;
  const href = link.getAttribute('href') ?? '';
  let url: URL;
  try {
    url = new URL(href, window.location.origin);
  } catch {
    return;
  }
  if (url.origin !== window.location.origin) return;
  const path = url.pathname;
  if (!path.startsWith('/api/resources/')) return;
  const title = link.textContent?.trim() || href;
  const open = (resourceId: string, fileUrl: string) => {
    if (openPdfPanel) openPdfPanel({ resourceId, fileUrl, title });
    else window.open(fileUrl, '_blank', 'noopener');
  };
  if (/\.pdf([?#].*)?$/i.test(href)) {
    const id = path.match(/^\/api\/resources\/([0-9a-fA-F-]{36})\/file/)?.[1];
    if (!id) return;
    event.preventDefault();
    open(id, href);
    return;
  }
  const bare = path.match(/^\/api\/resources\/([0-9a-fA-F-]{36})\/file\/?$/);
  const id = bare?.[1];
  if (!bare || !id) return;
  event.preventDefault();
  const node = nodesStore.getById(id);
  if (isPdfNode(node)) {
    open(id, namedFileUrl(id));
    return;
  }
  if (!node) {
    void nodesStore.fetch({ id }).then(
      () => {
        if (isPdfNode(nodesStore.getById(id))) open(id, namedFileUrl(id));
        else window.open(href, '_blank', 'noopener');
      },
      () => window.open(href, '_blank', 'noopener'),
    );
    return;
  }
  window.open(href, '_blank', 'noopener');
};

const isPdfNode = (n: Partial<Node> | undefined): boolean => {
  const meta = n?.metadata;
  return !!meta && typeof meta === 'object' && (meta as Record<string, unknown>).filetype === 'application/pdf';
};

const namedFileUrl = (id: string): string => {
  const name = nodesStore.getById(id)?.name || 'file.pdf';
  return `/api/resources/${id}/file/${encodeURIComponent(name)}`;
};

// Annotation links (`#annotation:<uuid>`, bare or pasted with text) open
// the side panel at the annotation (or the file at its page elsewhere).
const handleAnnotationLinkClick = (event: MouseEvent) => {
  if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
  const link = (event.target as HTMLElement).closest<HTMLAnchorElement>('a[href]');
  if (!link) return;
  const id = link.dataset.annotationLink || /^#annotation:([0-9a-fA-F-]{36})$/.exec(link.getAttribute('href') ?? '')?.[1];
  if (!id) return;
  event.preventDefault();
  void (async () => {
    try {
      const res = await makeRequest<PdfAnn>(`annotations/${id}`, 'GET', {});
      if (res.status !== 'success' || !res.result) throw new Error(res.message || 'annotation load failed');
      const ann = res.result;
      let node = nodesStore.getById(ann.resource_id);
      if (!node) node = await nodesStore.fetch({ id: ann.resource_id }).catch(() => undefined);
      const fileUrl = `/api/resources/${ann.resource_id}/file/${encodeURIComponent(node?.name || 'file.pdf')}`;
      if (openPdfPanel) {
        openPdfPanel({ resourceId: ann.resource_id, fileUrl, title: node?.name || 'PDF', page: ann.page, annotationId: ann.id });
      } else {
        window.open(`${fileUrl}#page=${ann.page}`, '_blank', 'noopener');
      }
    } catch (e) {
      notifications.add({ type: 'error', title: t('common.status.error'), message: String(e) });
    }
  })();
};

// Navigate internal document links (#<id>) through the router, without a full page reload
const handleInternalLinkClick = (event: MouseEvent) => {
  if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
  const link = (event.target as HTMLElement).closest<HTMLAnchorElement>('a[data-internal-link]');
  if (!link) return;
  const id = link.dataset.internalLink;
  if (!id) return;
  event.preventDefault();
  const target = nodesStore.getById(id);
  if (target) router.push(resolveNodeLink(target));
  else router.push(`/dashboard/docs/${id}`);
};

const handleClick = (event: MouseEvent) => {
  handleCodeCopyClick(event);
  handleCodeToggleClick(event);
  handleInternalLinkClick(event);
  handleAnnotationLinkClick(event);
  handlePdfLinkClick(event);
};

const handleCheckboxChange = (event: Event) => {
  const target = event.target as HTMLInputElement;
  if (!target || target.type !== 'checkbox' || !target.hasAttribute('data-checkbox-index')) return;
  if (!props.node?.content) return;

  const checkboxIndex = parseInt(target.getAttribute('data-checkbox-index') || '0', 10);
  const updatedMarkdown = updateMarkdownCheckbox(props.node.content, checkboxIndex, target.checked);

  // Recompile from the updated source rather than snapshotting the rendered DOM, so the
  // persisted HTML stays reusable (fresh diagram placeholders, resolved internal links).
  nodesStore.nodes.set(
    props.node.id!,
    {
      ...props.node,
      content: updatedMarkdown,
      content_compiled: compile(updatedMarkdown, id => nodesStore.getById(id)?.name),
    } as Node,
    true,
  );
  updateDocumentContent();
};

function runMermaidRender() {
  if (!rootElement.value) return;
  renderMermaidDiagrams(rootElement.value, { dark: colorMode.value === 'dark' });
}

// Inline PDFs (stock viewer iframes + view-only overlays): embeds always,
// plain links only in inline mode. Rebuilt when the HTML re-renders.
let inlinePdf: ReturnType<typeof createInlinePdf> | null = null;

function runPdfEmbeds() {
  if (!rootElement.value) return;
  inlinePdf?.destroy();
  inlinePdf = createInlinePdf(rootElement.value, {
    mode: () => pdfMode.value,
    openPanel: payload => {
      if (openPdfPanel) openPdfPanel(payload);
      else window.open(payload.fileUrl, '_blank', 'noopener');
    },
    listAnnotations: async resourceId => {
      const res = await makeRequest<PdfAnn[]>(`resources/${resourceId}/annotations`, 'GET', {});
      if (res.status !== 'success' || !res.result) throw new Error(res.message || 'annotations load failed');
      return res.result;
    },
    colorOf: i => PDF_ANNOT_COLORS[i] ?? PDF_ANNOT_COLORS[0]!,
    isPdfResource: id => isPdfNode(nodesStore.getById(id)),
    fileNameOf: id => nodesStore.getById(id)?.name || 'file.pdf',
  });
  inlinePdf.mountEmbeds();
  inlinePdf.ensureLinkMode();
  renderExtEmbeds(rootElement.value);
}

function runAbcRender() {
  if (!rootElement.value) return;
  renderAbcBlocks(rootElement.value);
}

function runDataviewRender() {
  if (!rootElement.value) return;
  const config = useRuntimeConfig();
  renderDataviewBlocks(rootElement.value, config.public.baseApi || '', props.node, id => nodesStore.getById(id));
}

function runCodeEmbeds() {
  if (!rootElement.value) return;
  renderCodeEmbeds(rootElement.value);
}

onMounted(() => {
  runMermaidRender();
  runAbcRender();
  runPdfEmbeds();
  runDataviewRender();
  runCodeEmbeds();
  const unsub = subscribeDrawioCacheInvalidated(() => {
    rerenderImages(rootElement.value!);
  });
  onUnmounted(() => {
    unsub();
    inlinePdf?.destroy();
    inlinePdf = null;
  });
});
watch(
  html,
  async () => {
    await nextTick();
    runMermaidRender();
    runAbcRender();
    runPdfEmbeds();
    runDataviewRender();
    runCodeEmbeds();
    if (rootElement.value) renderExtEmbeds(rootElement.value);
  },
);

watch(
  () => colorMode.value,
  async () => {
    await nextTick();
    runMermaidRender();
  },
);

// Per-note / global PDF mode flips re-render plain links (inline viewers
// mount or restore to links); embeds are mode-independent.
watch(pdfMode, () => {
  inlinePdf?.ensureLinkMode();
});
</script>

<style lang="scss" scoped>
article {
  max-width: 100%;
}

.doc-properties {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 12px;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  font-size: 13px;
  background: var(--bg-secondary);
}

.doc-property {
  display: flex;
  gap: 12px;
}

.doc-property-key {
  min-width: 110px;
  color: var(--text-secondary);
}

.doc-property-value {
  flex: 1;
  overflow-wrap: anywhere;
}
</style>
