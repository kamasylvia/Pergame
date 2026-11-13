import type { Node } from '~/stores';

interface QueryResult {
  columns: string[] | null;
  rows: Node[];
}

function esc(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

// Node path from the store ("Projects/2024"), for the ```index preset.
export function nodeDirPath(node: Partial<Node> | undefined, getById: (id: string) => Partial<Node> | undefined): string | null {
  if (!node) return null;
  // Index blocks live on a doc inside a folder: start from the parent.
  let cur = node.parent_id ? getById(node.parent_id) : undefined;
  // If the node itself is a folder/workspace, list it directly.
  if (node.role === 1 || node.role === 2) cur = node;
  if (!cur) return null;
  const segs: string[] = [];
  while (cur) {
    if (!cur.name) break;
    segs.unshift(cur.name);
    if (!cur.parent_id) break;
    cur = getById(cur.parent_id);
    if (segs.length > 32) break;
  }
  return segs.length ? segs.join('/') : null;
}

function cellText(node: Node, col: string): string {
  if (col.startsWith('metadata.')) {
    const v = (node.metadata as Record<string, unknown> | undefined)?.[col.slice(9)];
    return v == null ? '' : String(v);
  }
  const v = (node as unknown as Record<string, unknown>)[col];
  if (col === 'created_timestamp' || col === 'updated_timestamp') {
    return typeof v === 'number' ? new Date(v).toLocaleDateString() : '';
  }
  return v == null ? '' : String(v);
}

function indexRowLabel(n: Node): string {
  if (n.role === 1 || n.role === 2) return `📁 ${n.name || '(untitled)'}`;
  if (n.role === 4) return `📎 ${n.name || '(untitled)'}`;
  return n.name || '(untitled)';
}

function renderRows(el: HTMLElement, result: QueryResult, kind: string, currentNode?: Partial<Node>) {
  let rows = result.rows;
  if (kind === 'index') {
    // Subtree listing (issue #4): the folder itself, its index file and any
    // other index rows are not entries; folders sort first.
    const containerId = currentNode?.parent_id;
    rows = rows.filter(n => n.metadata?.index !== true && n.id !== currentNode?.id && n.id !== containerId);
    rows = [...rows].sort((a, b) => {
      const aFolder = a.role === 1 || a.role === 2 ? 0 : 1;
      const bFolder = b.role === 1 || b.role === 2 ? 0 : 1;
      if (aFolder !== bFolder) return aFolder - bFolder;
      const orderA = a.order ?? 0;
      const orderB = b.order ?? 0;
      if (orderA !== orderB) return orderA - orderB;
      return (a.name || '').localeCompare(b.name || '');
    });
  }
  if (!rows.length) {
    el.innerHTML = '<span class="dataview-empty">No results</span>';
    return;
  }
  if (!result.columns) {
    // LIST: one clickable row per node.
    el.innerHTML = `<ul class="dataview-list">${rows
      .map(n => `<li><a data-internal-link="${esc(n.id)}" href="#">${esc(kind === 'index' ? indexRowLabel(n) : n.name || '(untitled)')}</a></li>`)
      .join('')}</ul>`;
    return;
  }
  // TABLE: first column links to the node.
  const cols = result.columns;
  el.innerHTML = `<table class="dataview-table"><thead><tr>${cols
    .map(c => `<th>${esc(c)}</th>`)
    .join('')}</tr></thead><tbody>${rows
    .map(
      n =>
        `<tr>${cols
          .map(
            (c, i) =>
              i === 0
                ? `<td><a data-internal-link="${esc(n.id)}" href="#">${esc(cellText(n, c) || n.name || '(untitled)')}</a></td>`
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
    if (kind === 'index') {
      // Preset sugar: LIST the containing folder (issue #4 over the #5 engine).
      const dir = nodeDirPath(currentNode, getById);
      if (!dir) {
        el.innerHTML = '<span class="dataview-error">Index: not inside a folder</span>';
        continue;
      }
      source = `LIST FROM "${dir.replace(/"/g, '')}"`;
    }
    if (!source.trim()) {
      el.innerHTML = '<span class="dataview-empty">Empty query</span>';
      continue;
    }
    try {
      const resp = await fetch(`${apiBase}/api/nodes/query`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
        body: JSON.stringify({ query: source }),
      });
      const body = (await resp.json()) as { status: string; result?: QueryResult; message?: string };
      if (body.status !== 'success' || !body.result) {
        el.innerHTML = `<span class="dataview-error">${esc(body.message || 'Query failed')}</span>`;
        continue;
      }
      renderRows(el, body.result, kind, currentNode);
    } catch (err) {
      el.innerHTML = `<span class="dataview-error">${esc(err instanceof Error ? err.message : 'Query failed')}</span>`;
    }
  }
}
