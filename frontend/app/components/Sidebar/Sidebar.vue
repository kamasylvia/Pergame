<template>
  <div :class="{ 'sidebar-mask': isMobile && isOpened }" />
  <Resizable>
    <Dock v-if="!isMobile" />
    <div ref="sidebarRoot" class="sidebar" :class="{ compact: compactMode }">
      <div class="sidebar-fixed">
      <section class="header">
        <span class="name">
          <IconApp />
          <NuxtLink class="home-link" to="/dashboard">Pergame</NuxtLink>
        </span>
        <IconClose class="btn" />
      </section>
      <input v-model="filter" type="text" :placeholder="t('components.sidebar.searchPlaceholder')" />

      <div v-if="userStore.user" class="user">
        <UserAvatar :user="userStore.user" />
        <div class="details">
          <NuxtLink to="/dashboard/settings?p=profile">
            <div>{{ userStore.user.username }}</div>
            <div class="email">{{ userStore.user.email }}</div>
          </NuxtLink>
          <div class="icons">
            <NuxtLink to="/dashboard/docs/new" class="nav-item" @click="onClick">
              <Icon name="add_file" />
              <p class="hint-tooltip">{{ t('components.sidebar.newDoc') }}</p>
            </NuxtLink>
            <NuxtLink class="nav-item" @click="newCategory">
              <Icon name="add_folder" />
              <p class="hint-tooltip">{{ t('components.sidebar.newCategory') }}</p>
            </NuxtLink>
            <NuxtLink v-if="!nodesTree.isAllCollapsed()" class="nav-item" @click.stop="nodesTree.collapseAll">
              <Icon name="collapse" />
              <p class="hint-tooltip">{{ t('components.sidebar.closeAll') }}</p>
            </NuxtLink>
            <NuxtLink v-else class="nav-item" @click.stop="nodesTree.expandAll">
              <Icon name="arrow-expand" />
              <p class="hint-tooltip">{{ t('components.sidebar.openAll') }}</p>
            </NuxtLink>
          </div>
        </div>
      </div>
      <SidebarWorkspaces :workspaces="workspaces" :teams="teams" />
      <CollapseItem v-for="item in navigationItems(sidebarItemsPrefs)" :key="item.id" :item="item" :root="true" />
      <hr style="width: 100%; margin: 5px 0" />
      </div>
      <div ref="scroller" class="sidebar-tree">
      <VirtualTree v-if="tree.length" ref="virtualTree" :items="tree" :scroller="treeScroller" />
      <SidebarSkeleton v-else :is-loading="isLoading" />
      </div>
    </div>
  </Resizable>
</template>

<script setup lang="ts">
import { navigationItems, type Workspace } from './helpers';
import type { TreeItem } from '~/helpers/TreeBuilder';
import type { Node } from '~/stores';
import { resolveIcon, resolveNodeLink } from '~/helpers/node';
import NewCategoryModal from '~/components/Node/Modals/CreateCategory.vue';
import SidebarSkeleton from './SidebarSkeleton.vue';
import Dock from './Dock.vue';
import CollapseItem from './CollapseItem.vue';
import VirtualTree from './VirtualTree.vue';
import SidebarWorkspaces from './SidebarWorkspaces.vue';
import Resizable from './Resizable.vue';
import IconClose from './IconClose.vue';

const nodesStore = useNodesStore();
const preferences = usePreferencesStore();
const userStore = useUserStore();
const { t } = useI18nT();

const nodesTree = useNodesTree();
const { isOpened, hasSidebar, filtered } = useSidebar();
const { isMobile } = useDevice();
const modals = useModal();

const compactMode = preferences.get('compactMode');
const sidebarItemsPrefs = preferences.get('sidebarItems');
const route = useRoute();
const scroller = ref<HTMLElement | null>(null);
const sidebarRoot = ref<HTMLElement | null>(null);
// Mobile keeps the legacy whole-sidebar scroll, so the virtual list tracks the
// outer box there and the inner tree scroller on desktop.
const treeScroller = computed(() => (isMobile.value ? sidebarRoot.value : scroller.value));
const virtualTree = ref<{
  ensureVisible: (id: string) => Promise<boolean>;
  isRowInViewport: (id: string) => boolean;
  flatLength: ComputedRef<number>;
} | null>(null);

const filter = ref<string>('');

const teamRows = ref<Node[]>([]);
const workspaceRows = ref<Node[]>([]);

onMounted(async () => {
  const [t, w] = await Promise.all([
    nodesStore.fetchFlatAll(0).catch(() => ({ rows: [] as Node[] })),
    nodesStore.fetchFlatAll(1).catch(() => ({ rows: [] as Node[] })),
  ]);
  teamRows.value = t.rows;
  workspaceRows.value = w.rows;
});

const teams: ComputedRef<Workspace[]> = computed(() => [...teamRows.value.map(c => ({ text: c.name, value: c.id, meta: c, children: [] }))]);
const workspaces = computed(() => {
  const items = [];
  for (const workspace of workspaceRows.value) {
    const parentTeam = teams.value.find(t => t.value === workspace.parent_id);
    if (parentTeam) {
      parentTeam.children!.push({ text: workspace.name, value: workspace.id, meta: workspace });
      continue;
    }
    items.push({ text: workspace.name, value: workspace.id, meta: workspace });
  }
  return items;
});
const isLoading = computed(() => nodesStore.isFetching);

// Server-backed filter: substring matches as a flat list (the loaded tree
// is partial under lazy loading, so client-side tree filtering would lie).
const filterResults = ref<TreeItem<Node>[]>([]);
let filterSearchId = 0;
const runFilterSearch = useDebounceFn(async (query: string) => {
  const id = ++filterSearchId;
  const q = query.trim();
  if (!q) {
    filterResults.value = [];
    return;
  }
  try {
    const rows = await nodesStore.searchMeta(q, undefined, 50);
    if (id !== filterSearchId) return;
    filterResults.value = rows.map(node => ({
      id: node.id,
      label: node.name,
      route: resolveNodeLink(node),
      icon: resolveIcon(node),
      data: node,
    }));
  } catch {
    if (id === filterSearchId) filterResults.value = [];
  }
}, 250);

watch(filter, query => {
  runFilterSearch(query);
  // Filter cleared: reveal the open note in the restored tree.
  if (!query.trim() && routeNodeId.value) {
    revealedFor = null;
    revealToken++;
    lastNavAt = Date.now();
    void revealWithRetry(routeNodeId.value, revealToken);
  }
});

const tree = computed(() => {
  if (!filter.value.trim()) return filtered.value;
  return filterResults.value;
});

/**
 * Reveal the opened note in the tree: notes are often opened outside the
 * tree (search, links, tabs), leaving it collapsed elsewhere. Expands the
 * ancestor chain and scrolls the active row into view. Runs on route change
 * and retries as the tree populates (initial load fetches async); skips
 * silently when the node is not in the current workspace view.
 */
const routeNodeId = computed((): string | null => {
  const id = route.params.id;
  if (typeof id !== 'string') return null;
  if (route.path.startsWith('/dashboard/docs/') || route.path.startsWith('/dashboard/categories/')) return id;
  return null;
});
let revealedFor: string | null = null;
let revealToken = 0;
let lastNavAt = 0;
/**
 * Retry until revealed (bounded): the tree populates async and the virtual
 * list mounts a flush behind it, so a single attempt routinely lands in the
 * gap (empty tree, or list not yet mounted). Aborts when the route moves on.
 * Success means mounted AND in the viewport — rows merged later can push an
 * early success out of view again (lazy cascade), which the length watcher
 * below repairs within a bounded settle window.
 */
const revealWithRetry = async (nodeId: string, token: number) => {
  // Lazy bootstrap: the chain and every directory along it must exist in
  // the store before expansion can mount the row (no-ops when loaded).
  await nodesStore.ensureAncestors(nodeId).catch(() => {});
  for (let i = 0; i < 30 && token === revealToken; i++) {
    if (nodeId === revealedFor) return;
    // getAncestors returns [self … root]; the node itself needs no expansion.
    const chain = nodesTree.getAncestors(nodeId);
    const vt = virtualTree.value;
    if (chain.length && vt) {
      await Promise.all(chain.slice(1).map(item => nodesStore.ensureChildren(item.id).catch(() => {})));
      for (const item of chain.slice(1)) nodesTree.setExpanded(item.id, true);
      // The tree is virtualized: rows outside the window are not mounted, so
      // the list itself scrolls by index first, then polishes with a
      // nearest-scroll.
      if ((await vt.ensureVisible(nodeId)) && vt.isRowInViewport(nodeId)) {
        revealedFor = nodeId;
        return;
      }
    }
    await new Promise(resolve => setTimeout(resolve, 200));
  }
};
watch(routeNodeId, id => {
  revealedFor = null;
  revealToken++;
  lastNavAt = Date.now();
  // Filtered view shows search hits, not the tree: revealing would fight it.
  if (id && !filter.value.trim()) void revealWithRetry(id, revealToken);
}, { immediate: true });
// Post-success repair: late-merging rows can push a revealed row back out
// of view. Only within 10s of navigation and only when actually outside —
// never yanks a settled view.
watch(
  // NOTE: ref() unwraps the nested computed, so this is already a number.
  () => virtualTree.value?.flatLength,
  () => {
    const id = routeNodeId.value;
    const vt = virtualTree.value;
    if (!id || revealedFor !== id || !vt || Date.now() - lastNavAt > 10000) return;
    if (!vt.isRowInViewport(id)) void vt.ensureVisible(id);
  },
);

// Actions

const handleClickOutside = (e: MouseEvent) => {
  if (isOpened.value && e.target && !(e.target as Element).closest('.sidebar') && !(e.target as Element).closest('.open-sidebar')) isOpened.value = false;
};
const newCategory = () => modals.add(new Modal(shallowRef(NewCategoryModal), { props: { role: 2 } }));

const onClick = () => {
  if (isMobile.value) isOpened.value = false;
};

onMounted(() => {
  hasSidebar.value = true;
  if (isMobile.value) return document.addEventListener('click', handleClickOutside);
  // ELSE: Desktop
  isOpened.value = true;
});
onBeforeUnmount(() => {
  hasSidebar.value = false;
  document.removeEventListener('click', handleClickOutside);
});
</script>

<style scoped lang="scss">
.sidebar {
  display: flex;
  flex-direction: column;
  width: 100%;
  max-height: 100%;
  padding: 0.5rem 0.2rem 0.5rem 0.5rem;
  font-family: $font-ui;
  background: var(--surface-base);
  backdrop-filter: var(--backdrop-blur) var(--backdrop-saturate);
  overflow: hidden;
}

.sidebar-fixed {
  flex-shrink: 0;
}

.sidebar-tree {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  scrollbar-gutter: stable;

  &::-webkit-scrollbar-thumb {
    background: transparent;
  }
}

.home-link {
  font-family: Arial;
  font-size: 19px;
  font-weight: 600;
}

.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  height: 50px;
  margin-bottom: 8px;

  .btn {
    width: 30px;
    cursor: pointer;
    fill: var(--text-body);
  }

  .name {
    display: flex;
    align-items: baseline;
    gap: 6px;
    font-size: 1rem;
    font-weight: 400;
  }
}

.nav-item:hover .hint-tooltip {
  opacity: 1;
  visibility: visible;
}

.icons {
  position: relative;
  display: flex;
  height: fit-content;

  a {
    padding: 2px;
    border-radius: 25%;

    &:hover {
      background: var(--selection-color);
    }
  }
}

input {
  height: 30px;
  margin: 3px 0;
  padding-left: 30px;
  border: none;
  background-color: var(--surface-raised);
  background-image: url('data:image/svg+xml,<svg xmlns="http://www.w3.org/2000/svg" fill="%235b5967" height="24" viewBox="0 -960 960 960" width="22"><path d="M784-120 532-372q-30 24-69 38t-83 14q-109 0-184.5-75.5T120-580q0-109 75.5-184.5T380-840q109 0 184.5 75.5T640-580q0 44-14 83t-38 69l252 252-56 56ZM380-400q75 0 127.5-52.5T560-580q0-75-52.5-127.5T380-760q-75 0-127.5 52.5T200-580q0 75 52.5 127.5T380-400Z"/></svg>');
  background-repeat: no-repeat;
  background-position: 5px;
  outline: none;
  transition: background-color $transition-base;
}

.user {
  display: flex;
  align-items: center;
  margin: 4px 0 0 5px;

  .details {
    display: flex;
    justify-content: space-between;
    width: 100%;
    margin-left: 5px;
    font-size: 0.85rem;
  }

  .email {
    font-size: 0.7rem;
    color: var(--text-secondary);
  }
}

.compact:deep(.item) {
  margin: 0;
  padding: 0.5px 8px;
  font-size: 14px;
}

.sidebar-mask {
  position: fixed;
  top: 0;
  left: 0;
  z-index: 50;
  display: flex;
  width: 100vw;
  height: 100vh;
  background-color: var(--overlay-backdrop);
}

@media (width < 768px) {
  .sidebar {
    display: block;
    overflow-y: auto;
    scrollbar-gutter: stable;
  }

  .sidebar::-webkit-scrollbar-thumb {
    background: transparent;
  }

  .sidebar-tree {
    overflow: visible;
  }
}

</style>
