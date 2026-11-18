import type { MarkdownIt } from 'markdown-it';

/**
 * This plugin decorates fenced code blocks in the rendered markdown: a copy
 * button, and collapsing for long blocks. Blocks beyond COLLAPSE_THRESHOLD
 * lines render with a fixed max-height and an expand toggle, so a 200-line
 * compose file no longer dominates an entire note.
 */

const ICON_COPY = `<svg class="copy-icon" xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>`;
const ICON_CHECK = `<svg class="check-icon" xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>`;

/** Blocks with more lines than this start collapsed (0 = never). */
const DEFAULT_THRESHOLD = 16;

const lineCount = (src: string): number => src.replace(/\n$/, '').split('\n').length;

export const copyCodePlugin = (md: MarkdownIt) => {
  const defaultFence = md.renderer.rules.fence;

  md.renderer.rules.fence = function (tokens, idx, options, env, self) {
    const original = defaultFence ? defaultFence(tokens, idx, options, env, self) : self.renderToken(tokens, idx, options);
    const token = tokens[idx]!;
    const lines = lineCount(token.content || '');
    // The threshold rides in the render env (set by compile() from the
    // codeBlockCollapseThreshold preference); absent env keeps the default.
    // 0 is a meaningful value ("never collapse"), so guard explicitly rather
    // than relying on truthiness.
    const rawThreshold = (env as Record<string, unknown> | undefined)?.codeCollapseThreshold;
    const threshold = rawThreshold === undefined || rawThreshold === null || Number.isNaN(Number(rawThreshold))
      ? DEFAULT_THRESHOLD
      : Number(rawThreshold);
    const collapsible = threshold > 0 && lines > threshold;

    // The toggle lives in a footer row centered below the code. It only
    // exists for long blocks; the click handler (ContentCompiled.vue) flips
    // `collapsed` on the wrapper and swaps the label via CSS. The copy
    // button stays top-right in the header.
    const footer = collapsible
      ? `<div class="code-block-footer">
           <button type="button" class="code-toggle-btn" aria-expanded="false">
             <span class="toggle-label-expand">${lines} lines · expand</span>
             <span class="toggle-label-collapse">collapse</span>
           </button>
         </div>`
      : '';

    return `
<div class="code-block-wrapper${collapsible ? ' collapsed' : ''}">
  <div class="code-block-header">
    <button type="button" class="code-copy-btn" aria-label="Copy code" title="Copy code">
      <span class="btn-icon">${ICON_COPY}</span>
      <span class="btn-icon-success">${ICON_CHECK}</span>
    </button>
  </div>
  ${original}
  ${footer}
</div>`;
  };
};
