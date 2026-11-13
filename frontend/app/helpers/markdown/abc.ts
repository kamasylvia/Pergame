import type { MarkdownIt } from 'markdown-it';

export const ABC_OPEN_RE = /^(:{3,}|`{3,})abc\s*$/;

// Raw-capture block rule (mermaid.ts twin): ABC notation contains |: :|
// bar lines and [chords] that the nested block/inline parser would misread,
// so capture source verbatim like `fence` does.
export function abcPlugin(md: MarkdownIt) {
  md.block.ruler.before(
    'fence',
    'abc_block',
    (state, startLine, endLine, silent) => {
      const startPos = state.bMarks[startLine]! + state.tShift[startLine]!;
      const startMax = state.eMarks[startLine]!;
      const match = state.src.slice(startPos, startMax).match(ABC_OPEN_RE);
      if (!match) return false;

      const markerChar = match[1]![0];
      const minMarkerLen = match[1]!.length;

      let nextLine = startLine;
      let haveEndMarker = false;

      for (;;) {
        nextLine++;
        if (nextLine >= endLine) break; // unterminated: autoclose at end of scope

        const pos = state.bMarks[nextLine]! + state.tShift[nextLine]!;
        const max = state.eMarks[nextLine]!;

        if (pos < max && state.sCount[nextLine]! < state.blkIndent) break; // de-indented, stop
        if (state.sCount[nextLine]! - state.blkIndent >= 4) continue; // closing marker can't be indented

        const lineText = state.src.slice(pos, max).trim();
        if (lineText.length >= minMarkerLen && lineText.split('').every(c => c === markerChar)) {
          haveEndMarker = true;
          break;
        }
      }

      if (silent) return true;

      const content = state.getLines(startLine + 1, nextLine, state.blkIndent, false);
      const token = state.push('abc_block', 'pre', 0);
      token.block = true;
      token.content = content;
      token.map = [startLine, nextLine + (haveEndMarker ? 1 : 0)];

      state.line = nextLine + (haveEndMarker ? 1 : 0);
      return true;
    },
    { alt: ['paragraph', 'reference', 'blockquote', 'list'] },
  );

  md.renderer.rules.abc_block = (tokens, idx) => {
    return `<pre class="abc-notation">${md.utils.escapeHtml(tokens[idx]?.content ?? '')}</pre>\n`;
  };
}
