<template>
  <div class="context-menu" :class="{ 'is-context-menu': props.contextMenu }">
    <div class="menu-header">
      <UserAvatar :user="user" square />
      <div class="header-info">
        <span class="header-name">{{ node.name }}</span>
        <span class="header-meta">{{ user?.username }} · {{ shortDate(node.updated_timestamp) }}</span>
      </div>
    </div>

    <div class="menu-group">
      <button class="menu-item" @click="action('open')"><Icon name="file_open" />{{ t('common.actions.open') }}<kbd>Enter</kbd></button>
      <button class="menu-item" @click="action('edit')"><Icon name="edit_page" />{{ t('common.actions.edit') }}<kbd>E</kbd></button>
      <button class="menu-item" @click="action('new')"><Icon name="add_file" />{{ t('common.actions.new') }}</button>
    </div>

    <div class="menu-group">
      <button class="menu-item" @click="action('cloneTo')"><Icon name="duplicate" />{{ t('common.actions.cloneTo') }}</button>
      <button v-if="props.linkId" class="menu-item" @click="action('removeLink')"><Icon name="link_off" />{{ t('common.actions.removeLink') }}</button>
      <button class="menu-item" @click="action('duplicate')"><Icon name="duplicate" />{{ t('common.actions.duplicate') }}<kbd>Ctrl+D</kbd></button>
      <div class="menu-item has-submenu" @mouseenter="positionSubmenu">
        <Icon name="copy" />{{ t('common.actions.copy') }}
        <Icon class="submenu-arrow" name="chevron_right" />
        <div class="submenu">
          <button class="menu-item" @click="action('copyLink')"><Icon name="link" />{{ t('common.actions.copyLink') }}<kbd>Ctrl+L</kbd></button>
          <button class="menu-item" @click="action('copyPath')"><Icon name="location" />{{ t('common.actions.copyPath') }}</button>
          <button class="menu-item" @click="action('copyId')"><Icon name="snippets" />{{ t('common.actions.copy') }} ID</button>
        </div>
      </div>
      <button class="menu-item" @click="action('pin')">
        <Icon :name="node.order === -1 ? 'pin_off' : 'pin'" />{{ node.order === -1 ? t('common.actions.unpin') : t('common.actions.pin') }}<kbd>P</kbd>
      </button>
    </div>

    <div v-if="nodesPermissionsStore.hasPermissions(node, 4)" class="menu-group">
      <button class="menu-item" @click="action('manageAccess')"><Icon name="manage_access" />{{ t('nodes.actions.managePermissions') }}</button>
    </div>

    <div class="menu-group">
      <button class="menu-item delete" @click="action('delete')"><Icon name="delete" />{{ t('common.actions.delete') }}<kbd>Del</kbd></button>
    </div>

  </div>
</template>

<script setup lang="ts">
import NodePermissions from '../Modals/Permissions.vue';
import NodeDeleteModal from '../Modals/Delete.vue';
import NodeCloneModal from '../Modals/CloneTo.vue';
import type { Node } from '~/stores';
import { nodeFullPath, resolveNodeLink } from '~/helpers/node';

const props = defineProps<{ node: Node; contextMenu?: boolean; linkId?: string }>();
const emit = defineEmits(['close']);

const nodeStore = useNodesStore();
const nodesPermissionsStore = useNodesPermissionsStore();
const userStore = useUserStore();
const notifications = useNotifications();

const { t } = useI18nT();
const { shortDate } = useDateFormatters();
const router = useRouter();
const route = useRoute();
const modals = useModal();

userStore.fetchPublicUser(props.node.user_id);

const user = computed(() => userStore.getById(props.node.user_id || ''));
/** The submenu opens to the right; when that would leave the viewport
 * (menu opened near the right edge), flip it to the parent's left. */
function positionSubmenu(event: MouseEvent) {
  const row = event.currentTarget as HTMLElement;
  const sub = row.querySelector<HTMLElement>('.submenu');
  if (!sub) return;
  const rect = sub.getBoundingClientRect();
  sub.classList.toggle('submenu-left', rect.right > window.innerWidth - 8);
}

async function action(name: string) {
  switch (name) {
    case 'open':
      router.push(resolveNodeLink(props.node));
      break;
    case 'edit':
      // Workspaces and folders edit on the categories page, documents on the doc editor.
      router.push(props.node.role <= 2 ? `/dashboard/categories/${props.node.id}/edit` : `/dashboard/docs/edit/${props.node.id}`);
      break;
    case 'new':
      router.push(`/dashboard/docs/new?parent_id=${props.node.id}`);
      break;
    case 'cloneTo':
      modals.add(new Modal(shallowRef(NodeCloneModal), { props: { node: props.node }, size: 'small' }));
      break;
    case 'removeLink':
      if (props.linkId) {
        nodeStore
          .removeLink(props.node.id, props.linkId)
          .catch(e => notifications.add({ type: 'error', title: 'Clone', message: String(e) }));
      }
      break;
    case 'duplicate':
      nodeStore.duplicate(props.node);
      break;
    case 'delete':
      modals.add(
        new Modal(shallowRef(NodeDeleteModal), {
          props: { node: props.node, redirectTo: route.params?.id === props.node.id ? '/dashboard' : undefined },
          size: 'small',
        }),
      );
      break;
    case 'copyLink':
      navigator.clipboard.writeText(`${window.location.origin}${resolveNodeLink(props.node)}`);
      break;
    case 'copyPath':
      navigator.clipboard.writeText(nodeFullPath(props.node, id => nodeStore.getById(id)));
      break;
    case 'pin':
      await nodeStore.update({ ...props.node, order: props.node.order === -1 ? 0 : -1 });
      break;
    case 'manageAccess':
      modals.add(new Modal(shallowRef(NodePermissions), { props: { node: props.node }, size: 'small' }));
      break;
    case 'copyId':
      navigator.clipboard.writeText(props.node.id);
      break;
  }
  emit('close');
}
</script>

<style scoped lang="scss">
.context-menu {
  padding: 6px;
}

.menu-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 6px;
}

.header-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.header-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
  text-overflow: ellipsis;
  white-space: nowrap;
  overflow: hidden;
}

.header-meta {
  font-size: 11px;
  color: var(--text-secondary);
}

.menu-group {
  padding: 2px 0;

  & + & {
    margin-top: 2px;
    padding-top: 4px;
    border-top: 1px solid var(--border);
  }
}

.menu-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 4px 10px;
  border: none;
  border-radius: var(--radius-sm);
  font: inherit;
  font-size: 13px;
  color: var(--text-body);
  text-align: left;
  background: none;
  cursor: pointer;
  transition: background 0.1s;

  &:hover {
    background: var(--surface-raised);
  }

  kbd {
    margin-left: auto;
    padding: 2px 5px;
    font-size: 10px;
    color: var(--text-secondary);
    background: var(--surface-raised);
  }

  .submenu-arrow {
    margin-left: auto;
  }

  &.has-submenu {
    position: relative;

    &:hover > .submenu {
      display: block;
    }
  }

  .submenu {
    display: none;
    position: absolute;
    top: -6px;
    // Flush with the parent edge: a gap here breaks the :hover chain and
    // collapses the submenu before the pointer reaches it.
    left: 100%;
    min-width: 200px;
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    // Same surface as the parent menu so .menu-item :hover highlight reads.
    background: var(--surface-base);
    box-shadow: var(--shadow-md);

    &.submenu-left {
      left: auto;
      right: 100%;
    }
  }

  &.delete {
    color: var(--red);

    :deep(svg) {
      fill: var(--red);
    }

    kbd {
      color: var(--red);
      background: var(--red-bg);
    }
  }
}

@media screen and (width <= 768px) {
  .context-menu.is-context-menu {
    width: 100%;
    border: none;
    box-shadow: none;

    .menu-item {
      padding: 12px 16px;
      font-size: 16px;
    }
  }
}
</style>
