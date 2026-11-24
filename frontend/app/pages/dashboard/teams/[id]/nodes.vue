<template>
  <div>
    <Teleport to="#navbar-bottom"><NodeTeamNavbar :team-id="teamId" /></Teleport>
    <NodeContainerView :parent="parent" :nodes="rows" :parent-id="teamId" :total="total">
      <template #title>
        <Icon name="nodes" display="lg" />
        {{ t('nodes.nodes') }}
      </template>
    </NodeContainerView>
  </div>
</template>

<script setup lang="ts">
import type { Node } from '~/stores';

definePageMeta({ breadcrumb: { i18n: 'nodes.nodes' } });

const nodesStore = useNodesStore();

const route = useRoute();
const { t } = useI18nT();

const teamId = route.params.id as string;
const parent = computed(() => nodesStore.getById(teamId));
const rows = ref<Node[]>([]);
const total = ref(0);

onMounted(async () => {
  if (!nodesStore.getById(teamId)) await nodesStore.fetch({ id: teamId }).catch(() => null);
  const page = await nodesStore.fetchFlatAll(3, teamId).catch(() => ({ rows: [], total: 0 }));
  rows.value = page.rows;
  total.value = page.total;
});
</script>
