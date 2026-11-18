<template>
  <div class="overview-tables">
    <section>
      <h2>目录</h2>
      <table v-if="children.length" class="dataview-table">
        <thead>
          <tr>
            <th>名称</th>
            <th>创建时间</th>
            <th>修改时间</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="row in children" :key="row.id">
            <td><a href="#" @click.prevent="open(row)"><Icon :name="resolveIcon(row)" display="xsm" class="row-icon" style="display: inline-flex" />{{ indexRowLabel(row) }}</a></td>
            <td>{{ dateCell(row.created_timestamp) }}</td>
            <td>{{ dateCell(row.updated_timestamp) }}</td>
          </tr>
        </tbody>
      </table>
      <span v-else class="dataview-empty">No results</span>
    </section>
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
      <span v-else class="dataview-empty">No results</span>
    </section>
  </div>
</template>

<script setup lang="ts">
// Folder/workspace overview (index template v2 inlined): the same two
// preset tables the folder index files rendered, resolved against the
// container node directly instead of a stored markdown document.
import { childrenSource, recentSource, runDqlQuery, indexRowLabel, dateCell, sortFoldersFirst, onlyViewed } from '~/helpers/dataview-render';
import { resolveNodeLink, resolveIcon } from '~/helpers/node';
import type { Node } from '~/stores';

const props = defineProps<{ containerId: string }>();

const router = useRouter();
const config = useRuntimeConfig();

const children = ref<Node[]>([]);
const recent = ref<Node[]>([]);

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
  const results = await Promise.allSettled([
    runDqlQuery(apiBase, childrenSource(props.containerId)),
    runDqlQuery(apiBase, recentSource(props.containerId)),
  ]);
  if (results[0].status === 'fulfilled') children.value = sortFoldersFirst(results[0].value.rows);
  if (results[1].status === 'fulfilled') recent.value = onlyViewed(results[1].value.rows);
  // Failed queries render as their empty state; a broken query must not
  // break the whole container page.
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
