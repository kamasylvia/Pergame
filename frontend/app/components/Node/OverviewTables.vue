<template>
  <div class="overview-tables">
    <NodeDirectoryTree :container-id="props.containerId" />
    <section>
      <h2>最近浏览</h2>
      <table v-if="recent.length" class="dataview-table">
        <thead>
          <tr>
            <th>名称</th>
            <th>修改时间</th>
            <th>最近浏览</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="row in recent" :key="row.id">
            <td><a href="#" @click.prevent="open(row)"><Icon :name="resolveIcon(row)" display="xsm" class="row-icon" style="display: inline-flex" />{{ row.name || '(untitled)' }}</a></td>
            <td>{{ dateCell(row.updated_timestamp) }}</td>
            <td>{{ dateCell(viewed(row)) }}</td>
          </tr>
        </tbody>
      </table>
      <span v-else class="dataview-empty">{{ loading ? t('common.status.loading') : 'No results' }}</span>
    </section>
  </div>
</template>

<script setup lang="ts">
// Folder/workspace overview: the interactive directory tree plus the
// recent-documents table (the old flat children table retired with it).
import { recentSource, runDqlQuery, dateCell, onlyViewed } from '~/helpers/dataview-render';
import { resolveNodeLink, resolveIcon } from '~/helpers/node';
import type { Node } from '~/stores';

const props = defineProps<{ containerId: string }>();

const router = useRouter();
const config = useRuntimeConfig();
const { t } = useI18nT();

const recent = ref<Node[]>([]);
const loading = ref(true);

// `viewed_timestamp` is merged into DQL result rows at query time, so it is
// not part of the Node type.
function viewed(row: Node): unknown {
  return (row as unknown as Record<string, unknown>).viewed_timestamp;
}

function open(row: Node) {
  router.push(resolveNodeLink(row));
}

onMounted(async () => {
  const apiBase = (config.public.baseApi as string) || '';
  try {
    const result = await runDqlQuery(apiBase, recentSource(props.containerId));
    recent.value = onlyViewed(result.rows);
  } catch {
    // Failed queries render as their empty state; a broken query must not
    // break the whole container page.
  }
  loading.value = false;
});
</script>

<style scoped lang="scss">
.overview-tables {
  display: flex;
  flex-direction: column;
  gap: 24px;
}

h2 {
  margin-bottom: 8px;
  font-size: 16px;
  font-weight: 600;
}

.row-icon {
  margin-right: 6px;
  vertical-align: -3px;
}
</style>
