<template>
  <div style="width: 100%; padding: 1rem 0" @contextmenu.prevent="openContextMenu">
    <div v-if="!error" style="display: flex; justify-content: space-between">
      <div :style="{ maxWidth: width, flexBasis: width }" class="doc-container">
        <NodeDocumentHeader :doc="node" style="margin-bottom: 20px" />

        <NodeDocumentContentCompiled v-if="node && !node.partial" ref="elementComponent" :node="node" />
        <NodeDocumentSkeleton v-else />
        <NodeDocumentFooter :document="node" :next="next" :previous="previous" />
      </div>

      <div v-if="!devise.isTablet.value && !hideTOC && !sidePanel" class="toc">
        <NodeTOC :doc="node" :element="element" style="width: 320px; margin-left: 20px" />
      </div>
      <div v-if="sidePanel" class="doc-side">
        <NodeResourcePdfSidePanel
          v-if="sidePanel.kind === 'pdf'"
          :resource-id="sidePanel.resourceId"
          :file-url="sidePanel.fileUrl"
          :title="sidePanel.title"
          :page="sidePanel.page"
          :annotation-id="sidePanel.annotationId"
          @close="sidePanel = null"
        />
        <NodeResourceCodeSidePanel
          v-else-if="sidePanel.kind === 'code'"
          :file-url="sidePanel.fileUrl"
          :title="sidePanel.title"
          @close="sidePanel = null"
        />
        <NodeDocumentMetadataPanel v-else-if="node" :doc="node" @close="sidePanel = null" />
      </div>
    </div>
    <Error v-else :error="error" />
  </div>
</template>
<script setup lang="ts">
import NodeContextMenu from '~/components/Node/Action/ContextMenu.vue';
import NodeDocumentContentCompiled from '~/components/Node/Document/ContentCompiled.vue';
import DeleteNodeModal from '~/components/Node/Modals/Delete.vue';
import { resolveDocSize } from '~/helpers/node';

import type { RouteLocationNormalizedLoaded } from 'vue-router';
import type { Node } from '~/stores';

definePageMeta({
  breadcrumb: (route: RouteLocationNormalizedLoaded) => {
    return useBreadcrumbs().generateBreadcrumbsById(route.params.id as string);
  },
});

const documentsStore = useNodesStore();
const preferencesStore = usePreferencesStore();

const devise = useDevice();
const contextMenu = useContextMenu();
const modals = useModal();
const route = useRoute();
const router = useRouter();

const hideTOC = preferencesStore.get('hideTOC');
const docSize = preferencesStore.get('docSize');

const elementComponent = ref<InstanceType<typeof NodeDocumentContentCompiled>>();
const element = computed(() => elementComponent.value?.rootElement as HTMLElement | undefined);

const error = ref<null | PergameError>(null);

const documentId = computed(() => route.params.id as string);

// Prev/next doc navigation (server neighbors under the tree ordering —
// memory siblings go blind under lazy loading). Re-fetch on id/sort moves.
const sortKey = preferencesStore.get('sortKey');
const sortOrder = preferencesStore.get('sortOrder');
const neighborIds = ref<{ prev: string | null; next: string | null }>({ prev: null, next: null });
watch([documentId, sortKey, sortOrder], async ([id]) => {
  neighborIds.value = { prev: null, next: null };
  if (!id) return;
  try {
    const { prev, next } = await documentsStore.fetchNeighbors(id, sortKey.value, sortOrder.value, 3);
    if (documentId.value !== id) return;
    neighborIds.value = { prev: prev?.id ?? null, next: next?.id ?? null };
  } catch {
    // Footer hides; same as missing neighbors.
  }
}, { immediate: true });
const next = computed(() => (neighborIds.value.next ? documentsStore.getById(neighborIds.value.next) : undefined) ?? undefined);
const previous = computed(() => (neighborIds.value.prev ? documentsStore.getById(neighborIds.value.prev) : undefined) ?? undefined);

const node = computed<Node | undefined>(() => {
  if (!documentId.value) return undefined;
  return documentsStore.getById(documentId.value);
});

const width = computed(() => {
  const size = resolveDocSize(node.value, docSize.value ?? 1);
  if (size == 2) return '980px';
  if (size == 1) return '800px';
  return '700px';
});

async function loadDocument(id: string) {
  error.value = null;

  if (documentsStore.isFetching) return;

  let cached = documentsStore.getById(id);

  // Lazy loading: a deep link can arrive before any directory load, so a
  // cache miss means "fetch first", not 404.
  if (!cached) {
    try {
      cached = await documentsStore.fetch({ id });
    } catch {
      cached = undefined;
    }
  }
  if (!cached) {
    error.value = {
      code: 404,
      message: 'Document not found',
    };
    return;
  }

  if (cached.partial) {
    try {
      await documentsStore.fetch({ id });
    } catch (err: unknown) {
      error.value = {
        code: 500,
        message: (err as Error).message || 'An error occurred while fetching the document',
      };
    }
  }

  if (node.value) useHead({ title: node.value.name });
}

watch(
  [documentId, () => documentsStore.isFetching],
  ([newId, _]) => {
    if (newId) {
      loadDocument(newId);
    }
  },
  { immediate: true },
);

// Actions
const openContextMenu = (event: MouseEvent) => {
  if (!node.value) return;
  contextMenu.open(shallowRef(NodeContextMenu), event, {
    props: { contextMenu: true, node: node.value },
  });
};

const openDeleteModal = () => {
  if (!node.value) return;
  modals.add(
    new Modal(shallowRef(DeleteNodeModal), {
      props: { node: node.value, redirectTo: '/dashboard' },
      size: 'small',
    }),
  );
};

// Shortcuts
function handleDocumentKeydown(e: KeyboardEvent) {
  if (e.ctrlKey && e.key === 'e') {
    // Go to the edit page of the current document
    if (!node.value) return;
    e.preventDefault();
    router.push(`/dashboard/docs/edit/${node.value.id}`);
  }
  if (e.key === 'ArrowRight') {
    // Go to the next document page
    e.preventDefault();
    if (!next.value?.id) return;
    e.preventDefault();
    router.push(`/dashboard/docs/${next.value.id}`);
  }
  if (e.key === 'ArrowLeft') {
    if (!previous.value?.id) return;
    e.preventDefault();
    router.push(`/dashboard/docs/${previous.value.id}`);
  }
  if (e.key === 'Escape') {
    if (sidePanel.value) {
      sidePanel.value = null;
      return;
    }
    // Close context menu on Escape
    contextMenu.close();
  }
  if (e.key === 'Delete') {
    // Open delete modal on Delete
    e.preventDefault();
    openDeleteModal();
  }
}

onMounted(() => {
  document.addEventListener('keydown', handleDocumentKeydown);
});

onUnmounted(() => {
  document.removeEventListener('keydown', handleDocumentKeydown);
});

// Right side panel: PDF / code viewer xor doc settings (one slot, TOC yields).
// Plain PDF / code / annotation link clicks land here via ContentCompiled;
// the header Metadata action lands here too. Closed on navigation.
interface PdfPanelState {
  kind: 'pdf';
  resourceId: string;
  fileUrl: string;
  title: string;
  page?: number;
  annotationId?: string | null;
}
interface CodePanelState {
  kind: 'code';
  fileUrl: string;
  title: string;
}
type SidePanelState = PdfPanelState | CodePanelState | { kind: 'meta' };
const sidePanel = ref<SidePanelState | null>(null);
provide('openPdfPanel', (p: Omit<PdfPanelState, 'kind'>) => {
  sidePanel.value = { ...p, kind: 'pdf' };
});
provide('openCodePanel', (p: Omit<CodePanelState, 'kind'>) => {
  sidePanel.value = { ...p, kind: 'code' };
});
provide('openMetadataPanel', () => {
  sidePanel.value = { kind: 'meta' };
});
watch(documentId, () => {
  sidePanel.value = null;
});
</script>

<style scoped lang="scss">
.doc-container {
  // Pinned size: the side panel (flex-grow) absorbs all leftover instead.
  flex: 0 1 auto;
  width: 100%;
  margin: 0 auto;
}

@media screen and (width >= 810px) {
  .doc-container {
    padding: 0 2rem;
  }
}

.toc {
  position: relative;
}

@media screen and (width <= 1280px) {
  .toc {
    display: none;
  }
}

.doc-side {
  position: sticky;
  top: 70px;
  // Grow into freed space (hidden sidebar, wide screens); never squeeze
  // the doc below its basis, never collapse below usable width.
  flex: 1 1 560px;
  min-width: 380px;
  height: calc(100vh - 140px);
  margin-left: 20px;
}
</style>
