/**
 * `:::name` containers claimed by plugins (PS-C).
 *
 * A raw-capture block rule (same shape as the dataview block): the plugin owns
 * the body, the host owns the markers. Only names present in the registry at
 * parse time are captured, so the built-in container plugin keeps every other
 * `:::name` and the two never fight over the same block.
 */
import type { MarkdownIt as MarkdownItType } from 'markdown-it';
import { markdownHooks } from './markdown';

const OPEN_RE = /^:{3,}([a-z0-9][a-z0-9-]{0,31})(?:\s+(.*))?$/;

/** Register the dynamic container rule on one compiler instance. */
export function pluginContainers(md: MarkdownItType) {
  md.block.ruler.before(
    'fence',
    'plugin_container',
    (state, startLine, endLine, silent) => {
      const startPos = state.bMarks[startLine]! + state.tShift[startLine]!;
      const startMax = state.eMarks[startLine]!;
      const match = state.src.slice(startPos, startMax).match(OPEN_RE);
      if (!match) return false;
      const name = match[1]!;
      if (!markdownHooks.containers.has(name)) return false;

      const markerLen = match[0].match(/^:{3,}/)?.[0].length ?? 3;
      let nextLine = startLine;
      let haveEnd = false;
      for (;;) {
        nextLine++;
        if (nextLine >= endLine) break;
        const pos = state.bMarks[nextLine]! + state.tShift[nextLine]!;
        const max = state.eMarks[nextLine]!;
        if (pos < max && state.sCount[nextLine]! < state.blkIndent) break;
        if (state.sCount[nextLine]! - state.blkIndent >= 4) continue;
        const line = state.src.slice(pos, max).trim();
        if (/^:{3,}$/.test(line) && line.length >= markerLen) {
          haveEnd = true;
          break;
        }
      }
      if (silent) return true;

      const content = state.getLines(startLine + 1, nextLine, state.blkIndent, false);
      const token = state.push('plugin_container', 'div', 0);
      token.block = true;
      token.info = name;
      token.meta = { info: (match[2] ?? '').trim() };
      token.content = content;
      token.map = [startLine, nextLine + (haveEnd ? 1 : 0)];
      state.line = nextLine + (haveEnd ? 1 : 0);
      return true;
    },
    { alt: ['paragraph', 'reference', 'blockquote', 'list'] },
  );

  md.renderer.rules.plugin_container = (tokens, idx) => {
    const token = tokens[idx]!;
    const name = token.info;
    const render = markdownHooks.containers.get(name);
    if (!render) return md.utils.escapeHtml(token.content ?? '');
    try {
      const html = render({
        name,
        content: token.content ?? '',
        info: (token.meta as { info?: string } | undefined)?.info ?? '',
        escapeHtml: md.utils.escapeHtml as (text: string) => string,
      });
      if (html != null) return html;
    } catch (err) {
      console.warn(`[plugins] container '${name}' threw`, err);
    }
    return md.utils.escapeHtml(token.content ?? '');
  };
}
