<template>
  <div class="modal">
    <h3>{{ titleKey }} • {{ targets.length > 1 ? targets.length : targets[0]?.name }}</h3>
    <label for="clone-target">{{ pickKey }}</label>
    <input
      v-model="query"
      type="text"
      class="picker-search"
      :placeholder="t('common.actions.search')"
    />
    <!-- Flat fuzzy results while searching; the browse tree otherwise. -->
    <div v-if="query" class="tree-picker">
      <button
        v-for="row in matches"
        :key="row.id"
        class="result-row"
        :disabled="row.disabled"
        @click="pick(row.node)"
      >
        <Icon name="folder" />{{ row.path }}
      </button>
      <p v-if="!matches.length" class="description">{{ t('common.actions.search') }}: —</p>
    </div>
    <div v-else class="tree-picker">
      <AppSelectNode
        v-for="root in pickRoots"
        :key="root.id"
        :level="0"
        :node="root"
        :disabled="isDisabled"
        @select="pick"
      />
    </div>
    <p v-if="!query && !pickRoots.length" class="description">{{ pickKey }}: —</p>

    <div class="footer">
      <AppButton type="secondary" @click="emit('close')">{{ t('common.actions.cancel') }}</AppButton>
    </div>
  </div>
</template>

<script setup lang="ts">
import AppSelectNode from '~/components/AppSelectNode.vue';
import { nodeFullPath } from '~/helpers/node';
import type { Node } from '~/stores';
import type { TreeItem } from '~/helpers/TreeBuilder';

/** `move` re-parents the node(s); the default `clone` creates second positions. Bulk callers pass `nodes` (already topmost-reduced); single callers pass `node`. */
const props = withDefaults(defineProps<{ node?: Node; nodes?: Node[]; mode?: 'clone' | 'move' }>(), { node: undefined, nodes: undefined, mode: 'clone' });
const emit = defineEmits(['close']);

const store = useNodesStore();
const notifications = useNotifications();
const { t } = useI18nT();

const targets = computed(() => (props.nodes?.length ? props.nodes : props.node ? [props.node] : []));

const titleKey = computed(() => (props.mode === 'move' ? t('nodes.modals.delete.moveTitle') : t('nodes.modals.delete.cloneTitle')));
const pickKey = computed(() => (props.mode === 'move' ? t('nodes.modals.delete.movePick') : t('nodes.modals.delete.clonePick')));

/** Folders only (workspaces + categories); documents/resources are not parents. */
const folderTree = ref<TreeItem<Node>[]>([]);
const pickRoots = computed(() => folderTree.value.map(item => ({ ...item }) as ANode));

/** Every subject itself plus its subtree cannot be a target (server re-guards past the cap). */
const descendantIds = ref<Set<string>>(new Set());
const selfIds = computed(() => new Set(targets.value.map(n => n.id)));
const isDisabled = (i: ANode) => selfIds.value.has(String(i.id)) || descendantIds.value.has(String(i.id));

onMounted(async () => {
  folderTree.value = await store.fetchFolderTree(2).catch(() => []);
  const subs = await Promise.all(targets.value.map(n => store.fetchFlatAll(undefined, n.id).catch(() => ({ rows: [] as Node[] }))));
  descendantIds.value = new Set(subs.flatMap(s => s.rows.map(n => n.id)));
});

/** Flat "workspace / … / folder" index of every selectable folder. */
const flatFolders = computed(() => {
  const out: Array<{ node: ANode; name: string; path: string }> = [];
  const walk = (items: ANode[]) => {
    for (const item of items) {
      // Tree items carry `label`/`parentId`, not the node's own fields.
      const name = item.label ?? item.data?.name ?? '';
      out.push({
        node: item,
        name,
        path: nodeFullPath({ name, parent_id: item.parentId ?? item.data?.parent_id }, id => store.getById(id)),
      });
      if (item.children?.length) walk(item.children as ANode[]);
    }
  };
  walk(pickRoots.value);
  return out;
});

const query = ref('');

/** Name-substring matches (case-insensitive); disabled rows stay visible but greyed. */
const matches = computed(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return [];
  return flatFolders.value
    .filter(({ name }) => name.toLowerCase().includes(q))
    .map(({ node, name, path }) => ({ id: node.id, node, name, path, disabled: isDisabled(node) }));
});

const pick = async (target: ANode) => {
  const failed: string[] = [];
  for (const subject of targets.value) {
    try {
      if (props.mode === 'move') {
        await store.update({ ...subject, parent_id: String(target.id) });
      } else {
        await store.cloneTo(subject.id, String(target.id));
      }
    } catch (e) {
      failed.push(`${subject.name}: ${String(e)}`);
    }
  }
  emit('close');
  if (failed.length) {
    notifications.add({
      type: 'error',
      title: props.mode === 'move' ? 'Move' : 'Clone',
      message: failed.join('\n'),
    });
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

.picker-search {
  padding: 6px 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  font: inherit;
  font-size: 13px;
  color: var(--text-body);
  background: var(--surface-raised);

  &:focus {
    outline: none;
    border-color: var(--primary);
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

.result-row {
  display: flex;
  gap: 8px;
  align-items: center;
  width: 100%;
  padding: 5px 8px;
  border: none;
  border-radius: var(--radius-sm);
  font: inherit;
  font-size: 13px;
  color: var(--text-body);
  text-align: left;
  background: none;
  cursor: pointer;
  transition: background 0.1s;

  &:hover:not(:disabled) {
    background: var(--surface-raised);
  }

  &:disabled {
    color: var(--text-secondary);
    cursor: not-allowed;
    opacity: 0.55;
  }
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
