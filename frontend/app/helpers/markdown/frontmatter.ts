/**
 * Frontmatter (YAML subset) for document properties display.
 *
 * Parses a leading `---` block into key/value pairs (scalars + simple
 * lists). Rendering strips the block from the markdown and shows it as
 * a properties table instead (Obsidian-style, read-only).
 */

export interface FrontmatterResult {
  /** Properties in document order. Values are display strings. */
  properties: Array<{ key: string; value: string }>;
  /** Markdown with the frontmatter block removed. */
  content: string;
  /** True when a frontmatter block was present. */
  hasFrontmatter: boolean;
}

function cleanScalar(raw: string): string {
  let v = raw.trim();
  if ((v.startsWith('"') && v.endsWith('"')) || (v.startsWith("'") && v.endsWith("'"))) {
    v = v.slice(1, -1);
  }
  return v;
}

export function parseFrontmatter(markdown: string): FrontmatterResult {
  const empty: FrontmatterResult = { properties: [], content: markdown, hasFrontmatter: false };
  if (!markdown.startsWith('---\n') && !markdown.startsWith('---\r\n')) return empty;
  const end = markdown.indexOf('\n---', 3);
  if (end === -1) return empty;
  const body = markdown.slice(4, end);
  const rest = markdown.slice(end + 4).replace(/^\r?\n/, '');

  const properties: Array<{ key: string; value: string }> = [];
  const lines = body.split('\n');
  let currentKey: string | null = null;
  let currentList: string[] | null = null;
  const flush = () => {
    if (currentKey && currentList) {
      properties.push({ key: currentKey, value: currentList.join(', ') });
    }
    currentKey = null;
    currentList = null;
  };

  for (const line of lines) {
    if (!line.trim() || line.trim().startsWith('#')) continue;
    const listItem = line.match(/^\s*-\s+(.*)$/);
    if (listItem && currentKey) {
      currentList ??= [];
      currentList.push(cleanScalar(listItem[1] ?? ''));
      continue;
    }
    const kv = line.match(/^([\w-]+)\s*:\s*(.*)$/);
    if (kv?.[1]) {
      flush();
      const value = cleanScalar(kv[2] ?? '');
      // Inline list: tags: [a, b]
      const inline = value.match(/^\[(.*)\]$/);
      if (inline?.[1] !== undefined) {
        const items = inline[1].split(',').map(s => cleanScalar(s)).filter(Boolean);
        properties.push({ key: kv[1], value: items.join(', ') });
      } else if (value === '') {
        currentKey = kv[1];
        currentList = [];
      } else {
        properties.push({ key: kv[1], value });
      }
    }
  }
  flush();

  return { properties, content: rest, hasFrontmatter: true };
}
