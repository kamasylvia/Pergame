<template>
  <div ref="root">
    <div aria-hidden="true" :style="{ height: `${win.before}px` }" />
    <div
      v-for="(row, i) in windowRows"
      :key="row.key"
      class="vt-row"
      :class="{ 'vt-enter': isEntering(win.start + i) }"
      :data-row-key="row.key"
      :style="{ paddingLeft: `${row.level * 15}px` }"
      @click.capture="onRowClick($event, row.key, win.start + i)"
    >
      <div v-if="row.loading" class="vt-loading" aria-hidden="true"><span /></div>
      <SidebarItem
        v-else
        :level="row.level"
        :item="row.item"
        :class="{ 'collapse-header doc': row.hasChildren, selected: selected.has(row.key) }"
      >
        <svg
          v-if="row.hasChildren"
          :class="{ rotated: !row.expanded }"
          xmlns="http://www.w3.org/2000/svg"
          height="24"
          viewBox="0 -960 960 960"
          width="24"
          @click="toggle(row.item.id)"
        >
          <path d="M480-345 240-585l56-56 184 184 184-184 56 56-240 240Z" />
        </svg>
      </SidebarItem>
    </div>
    <div aria-hidden="true" :style="{ height: `${win.after}px` }" />
  </div>
</template>

<script setup lang="ts">
/**
 * Virtualized file tree, after SiYuan's DocTreeController: fixed measured
 * row height + expansion-aware flat rows + a scrolled window with
 * before/after spacers. Only the visible slice (+overscan) mounts, so a
 * fully expanded library no longer renders thousands of rows.
 *
 * Also copied: expand enter animation (fade-only; exit snapshots don't fit
 * windowing) and ctrl/shift multi-select with batch delete. Deliberately
 * NOT copied: async batched directory loading (SiYuan pages children from
 * the kernel; our nodes already sit in the store, so there is nothing to
 * batch) and keyboard navigation.
 */
import type { TreeItem } from '~/helpers/TreeBuilder';
import type { Node } from '~/stores';
import SidebarItem from './SidebarItem.vue';
import NodeDeleteModal from '~/components/Node/Modals/Delete.vue';
import { getItemChildren, type SidebarItem as SidebarItemType } from './helpers';

interface DataRow {
  loading?: false;
  key: string;
  item: TreeItem<Node>;
  level: number;
  /** Directory this row belongs to (link rows: the link parent). */
  parentKey: string | null;
  hasChildren: boolean;
  expanded: boolean;
}

interface LoadingRow {
  loading: true;
  key: string;
  level: number;
  parentKey: string | null;
}

type FlatRow = DataRow | LoadingRow;

const props = defineProps<{ items: TreeItem<Node>[]; scroller: HTMLElement | null }>();

const nodesStore = useNodesStore();
const nodesTree = useNodesTree();
const preferences = usePreferencesStore();
const compactMode = preferences.get('compactMode');
const sortKey = preferences.get('sortKey');
const sortDir = preferences.get('sortOrder');
const route = useRoute();

const OVERSCAN = 8;
const FALLBACK_ROW_HEIGHT = 28;

const root = ref<HTMLElement | null>(null);
const rowHeight = ref(FALLBACK_ROW_HEIGHT);
/** Tree-content position at the scroller viewport top (SiYuan's scrollTop). */
const viewTop = ref(0);
const viewportH = ref(600);

const flatRows = computed<FlatRow[]>(() => {
  const rows: FlatRow[] = [];
  const walk = (items: TreeItem<Node>[], level: number, parentKey: string | null) => {
    for (const item of items) {
      const kids = getItemChildren(item as SidebarItemType) ?? [];
      const expanded = nodesTree.isExpanded(item.id);
      // Unloaded-but-nonempty dirs (server has_children) keep their chevron.
      const hasKids = kids.length > 0 || nodesStore.hasChildren.get(item.id) === true;
      rows.push({ key: item.linkId ?? item.id, item, level, parentKey, hasChildren: hasKids, expanded });
      if (!kids.length || !expanded) {
        // Skeleton while the first page lands (never shown once complete).
        if (expanded && hasKids && !nodesStore.isDirComplete(item.id)) {
          rows.push({ loading: true, key: `loading:${item.id}`, level: level + 1, parentKey: item.id });
        }
        continue;
      }
      walk(kids as TreeItem<Node>[], level + 1, item.id);
    }
  };
  walk(props.items, 0, null);
  return rows;
});

/** Last flat index per directory (scroll-proximity pump needs it). */
const dirLastIndex = computed(() => {
  const m = new Map<string | null, number>();
  flatRows.value.forEach((r, i) => {
    if (!r.loading) m.set(r.parentKey, i);
  });
  return m;
});

/** Own-row flat index per directory id. */
const dirOwnIndex = computed(() => {
  const m = new Map<string, number>();
  flatRows.value.forEach((r, i) => {
    if (!r.loading) m.set(r.item.id, i);
  });
  return m;
});

/** SiYuan's getDocTreeWindow: visible slice plus overscan, spacer sizes. */
const win = computed(() => {
  const count = flatRows.value.length;
  const size = Math.max(1, rowHeight.value);
  const top = Math.max(0, viewTop.value);
  const height = Math.max(size, viewportH.value);
  const start = Math.max(0, Math.min(count, Math.floor(top / size) - OVERSCAN));
  const end = Math.max(start, Math.min(count, Math.ceil((top + height) / size) + OVERSCAN));
  return { start, end, before: start * size, after: (count - end) * size };
});
const windowRows = computed(() => flatRows.value.slice(win.value.start, win.value.end));

/** Descendant flat-range [start, end) for the row at `index`. */
function descendantRange(index: number): [number, number] {
  const rows = flatRows.value;
  const base = rows[index]?.level ?? 0;
  let end = index + 1;
  while (end < rows.length && rows[end]!.level > base) end++;
  return [index + 1, end];
}

// Expand enter animation: rows revealed by a toggle fade in once (transform
// + opacity only, so row-height math is untouched). Cleared shortly after
// so scroll-mounted rows don't replay it.
const enterRange = ref<{ start: number; end: number } | null>(null);
let enterTimer = 0;
const isEntering = (flatIndex: number) => {
  const range = enterRange.value;
  return !!range && flatIndex >= range.start && flatIndex < range.end;
};

const toggle = (id: string) => {
  const expanding = !nodesTree.isExpanded(id);
  nodesTree.toggleExpand(id);
  if (!expanding) return;
  // Lazy: first page on expand (no-op once the directory is complete).
  void nodesStore.ensureChildren(id, sortKey.value, sortDir.value);
  void nextTick().then(() => {
    const index = flatRows.value.findIndex(r => !r.loading && r.item.id === id);
    if (index < 0) return;
    const [start, end] = descendantRange(index);
    window.clearTimeout(enterTimer);
    enterRange.value = { start, end };
    enterTimer = window.setTimeout(() => {
      enterRange.value = null;
    }, 400);
  });
};

// Multi-select (position keys, SiYuan's occurrenceKey equivalent): plain
// click selects one and navigates as before; ctrl/meta toggles;
// shift extends from the anchor. Selection prunes itself when rows leave
// the flat list (collapse, delete, refetch).
const selected = ref(new Set<string>());
const anchorKey = ref<string | null>(null);

// Right-click bridge (injected by SidebarItem rows): the context menu acts on
// the whole multi-selection when the right-clicked row belongs to it, and
// collapses the selection to that row otherwise (Finder/VSCode behavior).
// Returns the selected nodes, or [] when collapsed to a single row.
provide('sidebarTreeSelection', {
  contextNodes(id: string): Node[] {
    const rows = flatRows.value.filter((r): r is DataRow => !r.loading && selected.value.has(r.key));
    if (rows.some(r => r.item.id === id)) {
      const seen = new Set<string>();
      const out: Node[] = [];
      for (const r of rows) {
        if (seen.has(r.item.data.id)) continue;
        seen.add(r.item.data.id);
        out.push(r.item.data);
      }
      return out;
    }
    const row = flatRows.value.find((r): r is DataRow => !r.loading && r.item.id === id);
    if (row) {
      selected.value = new Set([row.key]);
      anchorKey.value = row.key;
    }
    return [];
  },
});

function onRowClick(event: MouseEvent, key: string, flatIndex: number) {
  const row = flatRows.value[flatIndex];
  if (!row || row.loading || row.key !== key) return;
  if (event.ctrlKey || event.metaKey) {
    event.preventDefault();
    event.stopPropagation();
    if (selected.value.has(key)) selected.value.delete(key);
    else selected.value.add(key);
    anchorKey.value = key;
    return;
  }
  if (event.shiftKey && anchorKey.value) {
    event.preventDefault();
    event.stopPropagation();
    const rows = flatRows.value;
    const anchor = rows.findIndex(r => r.key === anchorKey.value);
    const [from, to] = anchor < 0 ? [flatIndex, flatIndex] : [Math.min(anchor, flatIndex), Math.max(anchor, flatIndex)];
    selected.value = new Set(
      rows.slice(from, to + 1).flatMap(r => (r.loading ? [] : [r.key])),
    );
    return;
  }
  selected.value = new Set([key]);
  anchorKey.value = key;
}

const modals = useModal();

function deleteSelected() {
  const rows = flatRows.value.filter((r): r is DataRow => !r.loading && selected.value.has(r.key));
  if (!rows.length) return;
  const deletingOpen = typeof route.params.id === 'string' && rows.some(r => r.item.id === route.params.id);
  modals.add(
    new Modal(shallowRef(NodeDeleteModal), {
      props: { nodes: rows.map(r => r.item.data), redirectTo: deletingOpen ? '/dashboard' : undefined },
      size: 'small',
    }),
  );
}

function onKeydown(event: KeyboardEvent) {
  if (!selected.value.size || modals.modals.value.length) return;
  const target = event.target as HTMLElement | null;
  // Never hijack typing: inputs, editors and editable surfaces keep Delete/Escape.
  if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable || target.closest('.cm-editor'))) return;
  if (event.key === 'Delete' || event.key === 'Backspace') {
    event.preventDefault();
    deleteSelected();
  } else if (event.key === 'Escape') {
    selected.value = new Set();
    anchorKey.value = null;
  }
}

/** Tree content top in scroller coordinates (the scroller also holds the header above us). */
function treeTopInScroller(): number | null {
  const sc = props.scroller;
  const el = root.value;
  if (!sc || !el) return null;
  return el.getBoundingClientRect().top - sc.getBoundingClientRect().top + sc.scrollTop;
}

function syncWindow() {
  const sc = props.scroller;
  const top = treeTopInScroller();
  if (!sc || top === null) return;
  viewTop.value = sc.scrollTop - top;
  viewportH.value = sc.clientHeight;
}

/** Stride between two rendered rows (margins included, collapsing honored). */
async function measure() {
  await nextTick();
  const rows = root.value?.querySelectorAll<HTMLElement>(':scope > .vt-row');
  if (rows && rows.length >= 2) {
    const stride = rows[1]!.offsetTop - rows[0]!.offsetTop;
    if (stride > 0 && Math.abs(stride - rowHeight.value) > 0.1) rowHeight.value = stride;
  }
}

let frame = 0;
function onScroll() {
  if (frame) return;
  frame = requestAnimationFrame(() => {
    frame = 0;
    syncWindow();
  });
}

/**
 * Mount the row for `id` and bring it into view (SiYuan's ensureRendered
 * minus the async loading): position math first, then a nearest-scroll
 * polish pass to absorb measurement drift.
 */
async function ensureVisible(id: string): Promise<boolean> {
  const index = flatRows.value.findIndex(r => !r.loading && r.item.id === id);
  if (index < 0) return false;
  const sc = props.scroller;
  if (sc) {
    syncWindow();
    const size = Math.max(1, rowHeight.value);
    const top = (treeTopInScroller() ?? 0) + index * size;
    if (top < sc.scrollTop) sc.scrollTop = top;
    else if (top + size > sc.scrollTop + sc.clientHeight) sc.scrollTop = Math.max(0, top + size - sc.clientHeight);
    syncWindow();
    await nextTick();
  }
  const key = flatRows.value[index]!.key;
  const rowEl =
    root.value?.querySelector(`[data-row-key="${key}"] .router-link-exact-active:not(.nav)`) ??
    root.value?.querySelector(`[data-row-key="${key}"] .router-link-active:not(.nav)`);
  rowEl?.scrollIntoView({ block: 'nearest' });
  return !!rowEl;
}

/** Mounted and inside the scroller viewport (reveal success criterion). */
function isRowInViewport(id: string): boolean {
  const sc = props.scroller;
  const row = flatRows.value.find(r => !r.loading && r.item.id === id);
  if (!sc || !row) return false;
  const el = root.value?.querySelector(`[data-row-key="${row.key}"]`);
  if (!el) return false;
  const r = el.getBoundingClientRect();
  const s = sc.getBoundingClientRect();
  return r.top >= s.top - 1 && r.bottom <= s.bottom + 1;
}

const flatLength = computed(() => flatRows.value.length);

defineExpose({ ensureVisible, isRowInViewport, flatLength });

let resizeObserver: ResizeObserver | null = null;

function attachScroller(sc: HTMLElement | null) {
  if (!sc) return;
  sc.addEventListener('scroll', onScroll, { passive: true });
  if (typeof ResizeObserver !== 'undefined') {
    resizeObserver = new ResizeObserver(() => syncWindow());
    resizeObserver.observe(sc);
  }
  syncWindow();
}

function detachScroller(sc: HTMLElement | null) {
  sc?.removeEventListener('scroll', onScroll);
  resizeObserver?.disconnect();
  resizeObserver = null;
}

onMounted(() => {
  attachScroller(props.scroller);
  void measure();
  window.addEventListener('keydown', onKeydown);
});

onBeforeUnmount(() => {
  if (frame) cancelAnimationFrame(frame);
  window.clearTimeout(enterTimer);
  detachScroller(props.scroller);
  window.removeEventListener('keydown', onKeydown);
});

// The parent template ref can land after our mount; (re)attach then.
watch(
  () => props.scroller,
  (sc, old) => {
    if (sc === old) return;
    detachScroller(old);
    attachScroller(sc);
    void measure();
  },
);

watch(
  () => flatRows.value.length,
  () => {
    syncWindow();
    void measure();
  },
);

// Scroll-proximity pump (SiYuan viewport loading): every expanded,
// incomplete directory whose span touches the prefetch band fetches
// forward — including dirs scrolled out of view (their own row carries
// them even when no child row is visible). No-op for complete dirs.
const PREFETCH_BAND = OVERSCAN * 8;
const linkPrefetch = new Set<string>();
function pumpVisibleDirs() {
  if (import.meta.server) return;
  const w = win.value;
  const rows = flatRows.value;
  const lo = w.start - PREFETCH_BAND;
  const hi = w.end + PREFETCH_BAND;
  const dirs = new Set<string | null>([null]);
  for (const [d, own] of dirOwnIndex.value) {
    const row = rows[own]!;
    if (row.loading || !row.expanded || !row.hasChildren) continue;
    const last = dirLastIndex.value.get(d) ?? -1;
    const spanLo = own;
    const spanHi = Math.max(own, last);
    if (spanLo <= hi && spanHi >= lo) dirs.add(d);
  }
  for (const d of dirs) {
    if (nodesStore.isDirComplete(d)) continue;
    const last = dirLastIndex.value.get(d) ?? -1;
    if (last < 0 || last <= w.end + OVERSCAN) {
      void nodesStore.loadMoreChildren(d, sortKey.value, sortDir.value);
    }
    // Clone positions whose target row never loaded: fetch it so the
    // position can render (TreeBuilder drops targetless positions).
    if (d !== null) {
      for (const link of nodesStore.linksByParent.get(d) ?? []) {
        if (!nodesStore.getById(link.node_id) && !linkPrefetch.has(link.node_id)) {
          linkPrefetch.add(link.node_id);
          void nodesStore.fetch({ id: link.node_id }).finally(() => linkPrefetch.delete(link.node_id));
        }
      }
    }
  }
}
watch(win, () => pumpVisibleDirs(), { immediate: true });
watch([sortKey, sortDir], () => {
  // Server order changed: drop page markers (merged rows stay; refetch
  // converges on the new order) and reload what is visible.
  nodesStore.invalidateAllDirs();
  pumpVisibleDirs();
});
watch(flatRows, rows => {
  // Prune selections that left the flat list (collapse, delete, refetch).
  const alive = new Set(rows.map(r => r.key));
  if (anchorKey.value && !alive.has(anchorKey.value)) anchorKey.value = null;
  if ([...selected.value].some(k => !alive.has(k))) selected.value = new Set([...selected.value].filter(k => alive.has(k)));
});
watch(compactMode, () => {
  rowHeight.value = FALLBACK_ROW_HEIGHT;
  void measure();
});
</script>

<style scoped lang="scss">
/* Chevron treatment mirrored from CollapseItem (scoped styles don't cross components). */
.collapse-header {
  svg {
    fill: var(--text-body);

    &.rotated {
      transform: rotate(-90deg);
    }
  }
}

.doc {
  svg {
    opacity: 0;
  }

  &:hover {
    svg {
      opacity: 1;
    }
  }
}

.vt-enter {
  animation: vt-fade-in 0.25s ease-out;
}

@keyframes vt-fade-in {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
}

@media (prefers-reduced-motion: reduce) {
  .vt-enter {
    animation: none;
  }
}

.selected {
  background: var(--selection-color);
}

.vt-loading {
  display: flex;
  align-items: center;
  height: 28px;
  padding: 0 8px;

  span {
    width: 60%;
    height: 12px;
    border-radius: var(--radius-xs);
    background: var(--surface-raised);
    animation: vt-pulse 1.2s ease-in-out infinite;
  }
}

@keyframes vt-pulse {
  0%,
  100% {
    opacity: 0.5;
  }

  50% {
    opacity: 1;
  }
}

@media (prefers-reduced-motion: reduce) {
  .vt-loading span {
    animation: none;
  }
}
</style>
