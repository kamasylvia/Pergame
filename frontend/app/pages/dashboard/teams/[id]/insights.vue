<template>
  <div class="page-card">
    <Teleport to="#navbar-title"><Icon name="insights" display="lg" /> {{ t('teams.insights.title') }}</Teleport>
    <Teleport to="#navbar-bottom"><NodeTeamNavbar :team-id="teamId" /></Teleport>
    <h2>{{ t('teams.insights.trending') }}</h2>
    <Trending :stats="stats" />
    <h2>{{ t('teams.insights.stats') }}</h2>
    <NodeStats :parent-id="teamId" />
  </div>
</template>

<script setup lang="ts">
import type { MonthlyCount } from '~/stores';

definePageMeta({ breadcrumb: { i18n: 'teams.insights.title' } });

const route = useRoute();
const nodesStore = useNodesStore();
const { t } = useI18nT();

const teamId = route.params.id as string;

const stats = ref<MonthlyCount[]>([]);

onMounted(async () => {
  const days = await nodesStore.fetchActivity(3660, teamId, 'created').catch(() => []);
  const byMonth = new Map<string, number>();
  for (const d of days) {
    const month = d.day.slice(5, 7);
    byMonth.set(month, (byMonth.get(month) ?? 0) + d.count);
  }
  stats.value = [...byMonth.entries()].map(([month, count]) => ({ month, count })).sort((a, b) => a.month.localeCompare(b.month));
});
</script>
