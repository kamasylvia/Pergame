<template>
  <NodeContainerView :parent="parent" :nodes="rows" :total="total" :loading="loading" />
</template>

<script setup lang="ts">
import NodeDeleteModal from '~/components/Node/Modals/Delete.vue';
import type { RouteLocationNormalizedLoaded } from 'vue-router';
import type { Node } from '~/stores';

definePageMeta({
  breadcrumb: (route: RouteLocationNormalizedLoaded) => {
    return useBreadcrumbs().generateBreadcrumbsById(route.params.id as string);
  },
});

const nodesStore = useNodesStore();
const nodesPermissionsStore = useNodesPermissionsStore();

const modals = useModal();
const route = useRoute();

const parentId = route.params.id as string;
const parent = computed(() => nodesStore.getById(parentId));
const rows = ref<Node[]>([]);
const total = ref(0);
const loading = ref(true);

onMounted(async () => {
  try {
    if (!nodesStore.getById(parentId)) {
      try {
        await nodesStore.fetch({ id: parentId });
      } catch {
        /* parent stays undefined; container renders the empty state */
      }
    }
    const page = await nodesStore.fetchFlatAll(3, parentId).catch(() => ({ rows: [], total: 0 }));
    rows.value = page.rows;
    total.value = page.total;
  } finally {
    loading.value = false;
  }
});

const openDeleteModal = () => {
  if (!parent.value || !nodesPermissionsStore.hasPermissions(parent.value, 4)) return;
  modals.add(new Modal(shallowRef(NodeDeleteModal), { size: 'small', props: { node: parent.value, redirect: '/dashboard' } }));
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
