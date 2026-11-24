<template>
  <section>
    <h2>Teams</h2>
    <NodeCardTeam v-for="team in teams" :key="team.id" :team="team" @click="emit('close')" />

    <NoContent v-if="!teams.length" :title="t('teams.actions.noTeams')" :description="t('teams.actions.createDescription')">
      <AppButton type="primary" @click="openCreateTeam">{{ t('teams.actions.create') }}</AppButton>
    </NoContent>
  </section>
</template>

<script setup lang="ts">
import CreateCategoryModal from '~/components/Node/Modals/CreateCategory.vue';
import type { Node } from '~/stores';

const emit = defineEmits<{ (e: 'close'): void }>();

const store = useNodesStore();
const teams = ref<Node[]>([]);

onMounted(async () => {
  teams.value = (await store.fetchFlatAll(0).catch(() => ({ rows: [] as Node[] }))).rows;
});

const { t } = useI18nT();
const modals = useModal();

const openCreateTeam = () => modals.add(new Modal(shallowRef(CreateCategoryModal), { props: { role: 0, parentId: undefined }, size: 'small' }));
</script>

<style scoped lang="scss">
section {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}
</style>
