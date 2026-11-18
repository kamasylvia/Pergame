import type { Node } from '~/stores';
import { resolveIcon } from '~/helpers/node';

interface QueryResult {
  columns: string[] | null;
  rows: Node[];
}

function esc(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

const COLUMN_LABELS: Record<string, string> = {
  name: '名称',
  created_timestamp: '创建时间',
  updated_timestamp: '修改时间',
  viewed_timestamp: '最近浏览',
  tags: '标签',
  description: '描述',
  role: '类型',
};

export function columnLabel(col: string): string {
  return COLUMN_LABELS[col] ?? col.replace(/^metadata\./, '');
}

function cellText(node: Node, col: string): string {
  if (col.startsWith('metadata.')) {
    const v = (node.metadata as Record<string, unknown> | undefined)?.[col.slice(9)];
    return v == null ? '' : String(v);
  }
  const v = (node as unknown as Record<string, unknown>)[col];
  if (col === 'created_timestamp' || col === 'updated_timestamp' || col === 'viewed_timestamp') {
    return typeof v === 'number' ? new Date(v).toLocaleDateString() : '';
  }
  return v == null ? '' : String(v);
}

export function dateCell(v: unknown): string {
  return typeof v === 'number' ? new Date(v).toLocaleDateString() : '';
}

/** Icon rendered before dataview row labels: the same sprite icon the
 * sidebar tree shows for the node, replacing the old OS-dependent emoji. */
export function iconSvg(n: Node): string {
  return `<svg class="icon" style="width:15px;height:15px;vertical-align:-3px;margin-right:4px" aria-hidden="true"><use href="#icon-${resolveIcon(n)}"></use></svg>`;
}

export function indexRowLabel(n: Node): string {
  return n.name || '(untitled)';
}

export function isFolder(n: { role?: number }): boolean {
  return n.role === 1 || n.role === 2;
}

/** Recent rows keep only docs with view history: an empty table means
 * nothing has been viewed yet (imported/migrated docs carry no history). */
export function onlyViewed(rows: Node[]): Node[] {
  return rows.filter(n => (n as unknown as Record<string, unknown>).viewed_timestamp != null);
}

export function sortFoldersFirst(rows: Node[]): Node[] {
  return [...rows].sort((a, b) => {
    const aFolder = isFolder(a) ? 0 : 1;
    const bFolder = isFolder(b) ? 0 : 1;
    if (aFolder !== bFolder) return aFolder - bFolder;
    const orderA = a.order ?? 0;
    const orderB = b.order ?? 0;
    if (orderA !== orderB) return orderA - orderB;
    return (a.name || '').localeCompare(b.name || '');
  });
}

function fullRowPath(row: Node, getById?: (id: string) => Partial<Node> | undefined): string {
  const segs: string[] = [row.name || '(untitled)'];
  let cur = row.parent_id && getById ? getById(row.parent_id) : undefined;
  let guard = 0;
  while (cur && guard++ < 32) {
    if (cur.name) segs.unshift(cur.name);
    if (!cur.parent_id) break;
    cur = getById!(cur.parent_id);
  }
  return segs.join(' / ');
}

/** Preset DQL for the folder/workspace overview: direct children table. */
export function childrenSource(containerId: string): string {
  return `TABLE name, created_timestamp, updated_timestamp FROM "id:${containerId}" DIRECT`;
}

/** Preset DQL for the folder/workspace overview: top 10 recently viewed docs. */
export function recentSource(containerId: string): string {
  return `TABLE name, updated_timestamp, viewed_timestamp FROM "id:${containerId}" WHERE role = 3 SORT viewed_timestamp DESC LIMIT 10`;
}

/** Run a DQL query against the nodes/query endpoint. */
export async function runDqlQuery(apiBase: string, source: string): Promise<QueryResult> {
  const resp = await fetch(`${apiBase}/api/nodes/query`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    credentials: 'include',
    body: JSON.stringify({ query: source }),
  });
  const body = (await resp.json()) as { status: string; result?: QueryResult; message?: string };
  if (body.status !== 'success' || !body.result) {
    throw new Error(body.message || 'Query failed');
  }
  return body.result;
}

function renderRows(el: HTMLElement, result: QueryResult, getById?: (id: string) => Partial<Node> | undefined) {
  const rows = sortFoldersFirst(result.rows);
  if (!rows.length) {
    el.innerHTML = '<span class="dataview-empty">No results</span>';
    return;
  }
  if (!result.columns) {
    // LIST: one clickable row per node.
    el.innerHTML = `<ul class="dataview-list">${rows
      .map(n => `<li><a data-internal-link="${esc(n.id)}" href="#" title="${esc(fullRowPath(n, getById))}">${iconSvg(n)}${esc(indexRowLabel(n))}</a></li>`)
      .join('')}</ul>`;
    return;
  }
  // TABLE: first column links to the node.
  const cols = result.columns;
  el.innerHTML = `<table class="dataview-table"><thead><tr>${cols
    .map(c => `<th>${esc(columnLabel(c))}</th>`)
    .join('')}</tr></thead><tbody>${rows
    .map(
      n =>
        `<tr>${cols
          .map(
            (c, i) =>
              i === 0
                ? `<td><a data-internal-link="${esc(n.id)}" href="#" title="${esc(fullRowPath(n, getById))}">${esc(cellText(n, c) || n.name || '(untitled)')}</a></td>`
                : `<td>${esc(cellText(n, c))}</td>`,
          )
          .join('')}</tr>`,
    )
    .join('')}</tbody></table>`;
}

export async function renderDataviewBlocks(
  root: HTMLElement,
  apiBase: string,
  currentNode: Partial<Node> | undefined,
  getById: (id: string) => Partial<Node> | undefined,
): Promise<void> {
  const elements = Array.from(root.querySelectorAll<HTMLElement>('div.dataview-block:not([data-dataview-done])'));
  if (!elements.length) return;

  for (const el of elements) {
    el.dataset.dataviewDone = '1';
    const kind = el.dataset.dataviewKind || 'dataview';
    let source: string;
    try {
      source = decodeURIComponent(el.dataset.dataviewSource || '');
    } catch {
      source = '';
    }
    if (kind === 'children' || kind === 'recent') {
      // Preset blocks (folder/workspace overview tables): list the node
      // itself — works for folders AND doc-parents and never goes stale on
      // rename/move (stable id, no path).
      const containerId = currentNode?.id;
      if (!containerId) {
        el.innerHTML = '<span class="dataview-error">No container node</span>';
        continue;
      }
      source = kind === 'children' ? childrenSource(containerId) : recentSource(containerId);
    }
    if (!source.trim()) {
      el.innerHTML = '<span class="dataview-empty">Empty query</span>';
      continue;
    }
    try {
      const result = await runDqlQuery(apiBase, source);
      if (kind === 'recent') result.rows = onlyViewed(result.rows);
      renderRows(el, result, getById);
    } catch (err) {
      el.innerHTML = `<span class="dataview-error">${esc(err instanceof Error ? err.message : 'Query failed')}</span>`;
    }
  }
}
