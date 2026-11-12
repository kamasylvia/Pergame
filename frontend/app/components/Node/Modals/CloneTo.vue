<template>
  <div class="modal">
    <h3>{{ t('nodes.modals.delete.cloneTitle') }} • {{ props.node.name }}</h3>
    <label for="clone-target">{{ t('nodes.modals.delete.clonePick') }}</label>
    <div class="tree-picker">
      <AppSelectNode
        v-for="root in pickRoots"
        :key="root.id"
        :level="0"
        :node="root"
        :disabled="isDisabled"
        @select="pick"
      />
    </div>
    <p v-if="!pickRoots.length" class="description">{{ t('nodes.modals.delete.clonePick') }}: —</p>

    <div class="footer">
      <AppButton type="secondary" @click="emit('close')">{{ t('common.actions.cancel') }}</AppButton>
    </div>
  </div>
</template>

<script setup lang="ts">
import AppSelectNode from '~/components/AppSelectNode.vue';
import type { Node } from '~/stores';

const props = defineProps<{ node: Node }>();
const emit = defineEmits(['close']);

const store = useNodesStore();
const nodesTree = useNodesTree();
const notifications = useNotifications();
const { t } = useI18nT();

/** Folders only (workspaces + categories); documents/resources are not parents. */
const pickRoots = computed(() =>
  nodesTree.getTreeUpToRole(2).value.map(
    item => ({ ...item }) as ANode,
  ),
);

const isDisabled = (i: ANode) => {
  if (i.id === props.node.id) return true;
  return nodesTree.getSubtreeAsArray(props.node.id).some(n => n.id === i.id);
};

const pick = async (target: ANode) => {
  try {
    await store.cloneTo(props.node.id, String(target.id));
    emit('close');
  } catch (e) {
    notifications.add({ type: 'error', title: 'Clone', message: String(e) });
  }
};
</script>

<style scoped lang="scss">
.modal {
  display: flex;
  flex-direction: column;
  gap: 10px;

  h3 {
    margin: 0;
    font-size: 15px;
    color: var(--text-primary);
  }

  label {
    font-size: 12px;
    color: var(--text-secondary);
  }
}

.tree-picker {
  display: flex;
  flex-direction: column;
  max-height: 320px;
  padding: 4px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  overflow: auto;
}

.description {
  font-size: 12px;
  color: var(--text-secondary);
}

.footer {
  display: flex;
  justify-content: flex-end;
}
</style>
