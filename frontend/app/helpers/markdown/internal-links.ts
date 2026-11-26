import type { MarkdownIt, StateInline } from 'markdown-it';

// Internal links: `#<uuid>` (what the editor inserts) or legacy `#<15-20
// digits>` (SiYuan-era snowflake ids) => link to the document, displayed as
// "#<document name>".
const UUID_BODY = '[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}';
const LEGACY_BODY = '\\d{15,20}';
export const INTERNAL_LINK_REGEX = new RegExp(`#(${UUID_BODY}(?![0-9a-fA-F])|${LEGACY_BODY}(?!\\d))`, 'g');
/** Same alternation anchored for single-position matching (inline ruler). */
export const INTERNAL_LINK_ANCHORED = new RegExp(`^(?:${UUID_BODY}(?![0-9a-fA-F])|${LEGACY_BODY}(?!\\d))`);
/** Bare `#annotation:<uuid>` (typed by hand; pasted links carry their own text). */
export const ANNOTATION_LINK_ANCHORED = new RegExp(`^annotation:(${UUID_BODY})(?![0-9a-fA-F])`);
export interface InternalLinkEnv {
  resolveNode?: (id: string) => string | undefined;
}

export function internalLinkPlugin(md: MarkdownIt) {
  md.inline.ruler.after('emphasis', 'internal_link', (state: StateInline, silent: boolean) => {
    const start = state.pos;
    if (state.src.charCodeAt(start) !== 0x23 /* # */) return false;

    // Word boundary before the # (avoid matching inside entities or other syntaxes)
    const prev = start > 0 ? state.src[start - 1] : '';
    if (prev && /[\w#&]/.test(prev)) return false;

    const rest = state.src.slice(start + 1, state.posMax);
    const ann = ANNOTATION_LINK_ANCHORED.exec(rest);
    if (ann) {
      if (!silent) {
        const token = state.push('internal_link', '', 0);
        token.meta = { id: ann[1], annotation: true };
      }
      state.pos = start + 1 + ann[0].length;
      return true;
    }

    const match = INTERNAL_LINK_ANCHORED.exec(rest);
    if (!match) return false;
    const end = start + 1 + match[0].length;

    if (!silent) {
      const token = state.push('internal_link', '', 0);
      token.meta = { id: match[0] };
    }

    state.pos = end;
    return true;
  });

  md.renderer.rules.internal_link = (tokens, idx, options, env) => {
    const id = String(tokens[idx]?.meta?.id ?? '');
    if (tokens[idx]?.meta?.annotation) {
      const short = id.slice(0, 8);
      return `<a href="#annotation:${id}" class="internal-link annotation-link" data-annotation-link="${id}">#annotation:${short}</a>`;
    }
    const name = (env as InternalLinkEnv)?.resolveNode?.(id);
    const label = md.utils.escapeHtml(name ? `#${name}` : `#${id}`);
    return `<a href="/dashboard/docs/${id}" class="internal-link" data-internal-link="${id}">${label}</a>`;
  };
}
