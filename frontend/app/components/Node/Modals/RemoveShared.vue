<template>
  <div class="modal">
    <h3>{{ t('nodes.modals.removeShared.title') }}</h3>
    <p>{{ t('nodes.modals.removeShared.confirm') }}</p>
    <p style="opacity: 0.7">{{ t('nodes.modals.removeShared.noAccess') }}</p>
    <p v-if="childCount > 0" class="warn">{{ t('nodes.modals.removeShared.childWarning', { count: childCount }) }}</p>
    <div class="footer">
      <AppButton type="secondary" @click="emit('close')">{{ t('common.actions.cancel') }}</AppButton>
      <AppButton type="danger" @click="removeDoc">{{ t('common.actions.confirm') }}</AppButton>
    </div>
  </div>
</template>

<script setup lang="ts">
const props = defineProps<{ nodeId: string }>();
const emit = defineEmits(['close']);

const nodesStore = useNodesStore();
const nodesPermissionsStore = useNodesPermissionsStore();
const userStore = useUserStore();

const { t } = useI18nT();
const notifications = useNotifications();
const childCount = ref(0);

onMounted(async () => {
  childCount.value = (await nodesStore.fetchStats(props.nodeId).catch(() => null))?.total ?? 0;
});

const removeDoc = () => {
  const node = nodesStore.getById(props.nodeId);
  const perm = node?.permissions.find(p => p.user_id === userStore.user?.id);
  if (!perm) return notifications.add({ type: 'error', title: t('common.errors.generic') });
  nodesPermissionsStore
    .removePermission(perm)
    .then(() => {
      notifications.add({ type: 'success', title: t('nodes.modals.removeShared.success') });
      emit('close');
      useRouter().push('/dashboard');
    })
    .catch(e => notifications.add({ type: 'error', title: t('common.errors.generic'), message: e }));
};
</script>

<style scoped lang="scss">
.warn {
  color: var(--red)-dark;
  opacity: 0.7;
}
</style>
