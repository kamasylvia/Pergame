<template>
  <li class="dir-row" :style="{ '--depth': depth }">
    <div
      class="row-main"
      :class="{ 'drop-before': dropZone === 'before', 'drop-after': dropZone === 'after', 'drop-into': dropZone === 'into' }"
      draggable="true"
      @dragstart="onDragStart"
      @dragover.prevent="onDragOver"
      @dragleave="onDragLeave"
      @drop.prevent="onDrop"
    >
      <slot name="leading" :node="node" />
      <button v-if="hasKids" class="chevron" :class="{ open: expanded }" @click="toggle">
        <Icon name="chevron_right" display="sm" />
      </button>
      <span v-else class="chevron-placeholder" />
      <a href="#" class="row-title" :title="titleTip" @click.prevent="open">
        <Icon :name="node.icon || resolveIcon(node)" display="xsm" class="row-icon" />
        <span class="row-name">{{ node.name || '(untitled)' }}</span>
      </a>
      <span v-if="total >= 0 && hasKids" class="child-count">{{ total }}</span>
      <span v-if="singleTag" class="tag tag-single">{{ singleTag }}</span>
      <span class="row-time">{{ dateCell(node.updated_timestamp) }}</span>
      <button
        class="pin-btn"
        :class="{ pinned: node.order === -1 }"
        :title="node.order === -1 ? t('common.actions.unpin') : t('common.actions.pin')"
        @click="togglePin"
      >
        <Icon :name="node.order === -1 ? 'pin_off' : 'pin'" display="xsm" />
      </button>
    </div>
    <div v-if="multiTags.length" class="row-tags">
      <span v-for="tag in multiTags" :key="tag" class="tag">{{ tag }}</span>
    </div>
    <slot name="below" :node="node" :depth="depth" />
    <ul v-if="expanded && hasKids" class="row-children">
      <NodeDirectoryTreeRow v-for="child in children" :key="child.id" :node="child" :depth="depth + 1" :sort="sort">
        <template #leading="p">
          <slot name="leading" v-bind="p" />
        </template>
        <template #below="p">
          <slot name="below" v-bind="p" />
        </template>
      </NodeDirectoryTreeRow>
      <li v-if="!complete" class="load-more-row">
        <AppButton type="link" :disabled="loading" @click="loadMore">
          {{ loading ? t('common.status.loading') : t('nodes.directory.loadMore') }}
        </AppButton>
      </li>
    </ul>
  </li>
</template>

<script setup lang="ts">
import DirectoryMoveConfirm from '~/components/Node/Modals/DirectoryMoveConfirm.vue';
import { resolveIcon, resolveNodeLink, parseTags } from '~/helpers/node';
import { dateCell } from '~/helpers/dataview-render';
import { moveDirectoryNode, type DirectorySort, type DropZone } from '~/composables/useDirectoryLevel';
import type { Node } from '~/stores';

const props = defineProps<{ node: Node; depth: number; sort: DirectorySort }>();

defineSlots<{
  leading(props: { node: Node }): unknown;
  below(props: { node: Node; depth: number }): unknown;
}>();

const nodesStore = useNodesStore();
const preferences = usePreferencesStore();
const router = useRouter();
const modals = useModal();
const notifications = useNotifications();
const { t } = useI18nT();

const resetSort = inject<() => void>('directoryResetSort', () => {});
const registerRow = inject<(id: string) => void>('directoryRegister', () => {});
const unregisterRow = inject<(id: string) => void>('directoryUnregister', () => {});

const expanded = ref(false);

onMounted(() => registerRow(props.node.id));
onUnmounted(() => unregisterRow(props.node.id));

const { children, total, complete, loading, ensure, loadMore } = useDirectoryLevel(
  () => props.node.id,
  () => props.sort,
);

const hasKids = computed(() => children.value.length > 0 || nodesStore.hasChildren.get(props.node.id) === true);

const tags = computed(() => (props.node.tags ? parseTags(props.node.tags) : []));
const singleTag = computed(() => (tags.value.length === 1 ? tags.value[0] : ''));
const multiTags = computed(() => (tags.value.length > 1 ? tags.value : []));

const titleTip = computed(() => `${t('components.filter.created')}: ${dateCell(props.node.created_timestamp)}`);

const open = () => router.push(resolveNodeLink(props.node));

const toggle = () => {
  expanded.value = !expanded.value;
  if (expanded.value) ensure();
};

const togglePin = async () => {
  try {
    await nodesStore.update({ ...props.node, order: props.node.order === -1 ? 0 : -1 });
  } catch {
    // Pinned state re-renders from the store; a failed write simply keeps it.
  }
};

// Drag and drop (plugin parity): edge quarters reorder siblings, the
// middle moves into the target. Same mime as the sidebar so rows can be
// dragged across both trees.
const dropZone = ref<DropZone | ''>('');

const onDragStart = (event: DragEvent) => {
  event.dataTransfer?.setData('text/plain', props.node.id);
  if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
};

const zoneAt = (event: DragEvent): DropZone => {
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  const ratio = (event.clientY - rect.top) / Math.max(1, rect.height);
  if (ratio < 0.25) return 'before';
  if (ratio > 0.75) return 'after';
  return 'into';
};

const onDragOver = (event: DragEvent) => {
  if (!event.dataTransfer?.types.includes('text/plain')) return;
  dropZone.value = zoneAt(event);
};

const onDragLeave = (event: DragEvent) => {
  // Dragleave fires on child transitions too; only clear when truly outside.
  const current = event.currentTarget as HTMLElement;
  const to = event.relatedTarget as unknown as HTMLElement | null;
  if (to && current.contains(to)) return;
  dropZone.value = '';
};

const onDrop = async (event: DragEvent) => {
  const zone = dropZone.value || zoneAt(event);
  dropZone.value = '';
  const draggedId = event.dataTransfer?.getData('text/plain') || '';
  const dragged = draggedId ? nodesStore.getById(draggedId) : undefined;
  if (!dragged || dragged.id === props.node.id) return;
  // Guards mirror the sidebar (plus the server cycle guard as backstop).
  if (zone === 'into') {
    if ((dragged.role ?? 99) <= 2 && (props.node.role ?? 99) > 2) return;
    if (nodesStore.isDescendant(dragged, props.node.id)) return;
    if (preferences.get('directoryConfirmMove').value) {
      modals.add(
        new Modal(shallowRef(DirectoryMoveConfirm), {
          props: {
            nodeName: dragged.name,
            targetName: props.node.name,
            onConfirm: () => void doMove(dragged.id, 'into'),
          },
          size: 'small',
        }),
      );
      return;
    }
  }
  await doMove(dragged.id, zone);
};

const doMove = async (draggedId: string, zone: DropZone) => {
  // Reordering under a temp sort would be invisible; restore tree order.
  if (zone !== 'into') resetSort();
  try {
    await moveDirectoryNode(draggedId, props.node.id, zone);
  } catch (e) {
    notifications.add({ type: 'error', title: t('common.status.error'), message: String(e) });
  }
};
</script>

<style scoped lang="scss">
.dir-row {
  list-style: none;
}

.row-main {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 3px 4px 3px calc(var(--depth, 0) * 18px + 4px);
  border-radius: var(--radius-sm);
  border-top: 2px solid transparent;
  border-bottom: 2px solid transparent;

  &:hover {
    background: var(--surface-hover);
  }

  &.drop-before {
    border-top-color: var(--primary);
  }

  &.drop-after {
    border-bottom-color: var(--primary);
  }

  &.drop-into {
    background: var(--accent-bg);
  }
}

.chevron {
  display: inline-flex;
  padding: 2px;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition: transform $transition-fast ease;

  &.open {
    transform: rotate(90deg);
  }
}

.chevron-placeholder {
  width: 22px;
  flex-shrink: 0;
}

.row-title {
  display: inline-flex;
  align-items: center;
  min-width: 0;
  flex: 1;
  color: var(--text-primary);
  text-decoration: none;
  cursor: pointer;
}

.row-icon {
  margin-right: 6px;
  flex-shrink: 0;
}

.row-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.child-count {
  font-size: 0.75rem;
  color: var(--text-secondary);
  background: var(--surface-raised);
  border-radius: var(--radius-sm);
  padding: 0 6px;
  flex-shrink: 0;
}

.tag {
  font-size: 0.75rem;
  color: var(--primary);
  background: var(--primary-bg);
  border-radius: var(--radius-sm);
  padding: 0 6px;
  white-space: nowrap;
  flex-shrink: 0;
}

.row-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  padding: 0 4px 2px calc(var(--depth, 0) * 18px + 30px);
}

.row-time {
  font-size: 0.78rem;
  color: var(--text-secondary);
  white-space: nowrap;
  flex-shrink: 0;
}

.pin-btn {
  display: inline-flex;
  padding: 2px;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  opacity: 0;
  flex-shrink: 0;

  .row-main:hover &,
  &.pinned {
    opacity: 1;
  }

  &.pinned {
    color: var(--primary);
  }
}

.row-children {
  margin: 0;
  padding: 0;
}

.load-more-row {
  list-style: none;
  padding-left: calc(var(--depth, 0) * 18px + 30px);
}
</style>
