<template>
  <div class="quick-search-tab">
    <SearchResultsList
      :items="flattenedItems"
      :selected-index="selectedIndex"
      :query="searchQuery"
      @update-selected-index="$emit('updateSelectedIndex', $event)"
    />
  </div>
</template>

<script setup lang="ts">
import { quickActions, availablePages, type SearchResult, type BaseCommand } from '~/helpers/navigation';
import { usePluginUi } from '~/services/plugins';
import SearchResultsList from './SearchResultsList.vue';

const props = defineProps<{ searchQuery: string; selectedIndex: number }>();
defineEmits<{ updateSelectedIndex: [index: number] }>();

const documentStore = useNodesStore();
const pluginUi = usePluginUi();

function tokenize(text: string) {
  return text.trim().toLowerCase().split(/\s+/).filter(Boolean);
}

function matchesTokens(haystack: string, tokens: string[]) {
  const value = haystack.toLowerCase();
  return tokens.every(t => value.includes(t));
}

function filterByTokens<T>(items: T[], getText: (item: T) => string): T[] {
  const tokens = tokenize(props.searchQuery);
  if (tokens.length === 0) return items;
  return items.filter(item => matchesTokens(getText(item), tokens));
}

const filteredActions = computed(() => filterByTokens(quickActions, a => `${a.title} ${a.description}`));

const docResults = ref<SearchResult[]>([]);
let docSearchId = 0;

// Server metadata search (the store no longer holds every node): debounced,
// out-of-order guarded, empty on blank query.
const runDocSearch = useDebounceFn(async (query: string) => {
  const id = ++docSearchId;
  if (!query.trim()) {
    docResults.value = [];
    return;
  }
  try {
    const rows = await documentStore.searchMeta(query, 3, 5);
    if (id !== docSearchId) return;
    docResults.value = rows.map(d => ({
      id: d.id,
      title: d.name,
      description: d.tags ? `#${String(d.tags)}` : 'Document',
      icon: 'files',
      path: `/dashboard/docs/${d.id}`,
      category: 'Documents',
    }));
  } catch {
    if (id === docSearchId) docResults.value = [];
  }
}, 200);

watch(
  () => props.searchQuery,
  query => runDocSearch(query),
  { immediate: true },
);

const filteredDocuments = computed<SearchResult[]>(() => docResults.value);

const filteredPages = computed(() => filterByTokens(availablePages, p => `${p.title} ${p.description} ${p.category}`));

const filteredPluginCommands = computed(() =>
  filterByTokens(pluginUi.commands, c => `${c.title} ${c.pluginId}`),
);

/** Palette row: navigable (`path`) or executable (`run`), never both. */
interface FlattenedItem extends Omit<BaseCommand, 'path'> {
  path?: string;
  run?: () => void;
  section: string;
  globalIndex: number;
}

const flattenedItems = computed(() => {
  const items: FlattenedItem[] = [];
  let globalIndex = 0;

  const pushWithIndex = (arr: Omit<FlattenedItem, 'section' | 'globalIndex'>[], section: string) => {
    arr.forEach(item => {
      items.push({ ...item, section, globalIndex });
      globalIndex++;
    });
  };

  pushWithIndex(
    filteredActions.value.map(a => ({
      id: `action-${a.id}`,
      icon: a.icon,
      title: a.title,
      description: a.description,
      path: a.path,
      shortcut: a.shortcut,
    })),
    'Pages & actions',
  );

  pushWithIndex(
    filteredPages.value.map(p => ({
      id: `page-${p.id}`,
      icon: p.icon,
      title: p.title,
      description: p.description,
      path: p.path,
    })),
    'Pages & actions',
  );

  pushWithIndex(
    filteredPluginCommands.value.map(c => ({
      id: `plugin-${c.id}`,
      icon: '🧩',
      title: c.title,
      description: `Plugin · ${c.pluginId}`,
      run: c.run,
    })),
    'Plugins',
  );

  pushWithIndex(
    filteredDocuments.value.map(d => ({
      id: `doc-${d.id}`,
      icon: d.icon,
      title: d.title,
      description: d.description,
      path: d.path,
    })),
    'Documents',
  );

  return items;
});

defineExpose({ flattenedItems });
</script>

<style scoped lang="scss">
.quick-search-tab {
  display: flex;
  flex-direction: column;
  height: 100%;
}
</style>
