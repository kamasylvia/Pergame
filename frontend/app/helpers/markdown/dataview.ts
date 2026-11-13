import type { MarkdownIt } from 'markdown-it';

export const DATAVIEW_OPEN_RE = /^(:{3,}|`{3,})(dataview|index)\s*$/;

// Raw-capture block rule (mermaid.ts pattern): DQL source must not pass the
// nested block/inline parser ("#tag" would become a heading, etc.).
export function dataviewPlugin(md: MarkdownIt) {
  md.block.ruler.before(
    'fence',
    'dataview_block',
    (state, startLine, endLine, silent) => {
      const startPos = state.bMarks[startLine]! + state.tShift[startLine]!;
      const startMax = state.eMarks[startLine]!;
      const match = state.src.slice(startPos, startMax).match(DATAVIEW_OPEN_RE);
      if (!match) return false;

      const kind = match[2]!;
      const markerChar = match[1]![0];
      const minMarkerLen = match[1]!.length;

      let nextLine = startLine;
      let haveEndMarker = false;

      for (;;) {
        nextLine++;
        if (nextLine >= endLine) break;

        const pos = state.bMarks[nextLine]! + state.tShift[nextLine]!;
        const max = state.eMarks[nextLine]!;

        if (pos < max && state.sCount[nextLine]! < state.blkIndent) break;
        if (state.sCount[nextLine]! - state.blkIndent >= 4) continue;

        const lineText = state.src.slice(pos, max).trim();
        if (lineText.length >= minMarkerLen && lineText.split('').every(c => c === markerChar)) {
          haveEndMarker = true;
          break;
        }
      }

      if (silent) return true;

      const content = state.getLines(startLine + 1, nextLine, state.blkIndent, false);
      const token = state.push('dataview_block', 'div', 0);
      token.block = true;
      token.content = content;
      token.info = kind;
      token.map = [startLine, nextLine + (haveEndMarker ? 1 : 0)];

      state.line = nextLine + (haveEndMarker ? 1 : 0);
      return true;
    },
    { alt: ['paragraph', 'reference', 'blockquote', 'list'] },
  );

  md.renderer.rules.dataview_block = (tokens, idx) => {
    const token = tokens[idx]!;
    const kind = token.info || 'dataview';
    const encoded = md.utils.escapeHtml(token.content ?? '');
    return `<div class="dataview-block" data-dataview-kind="${kind}" data-dataview-source="${encodeURIComponent(token.content ?? '')}"><span class="dataview-loading">Loading…</span><!--${encoded}--></div>\n`;
  };
}
