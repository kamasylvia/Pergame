import type { Env, MarkdownIt, StateInline } from 'markdown-it';
import { mimeToExt } from '~/helpers/resources';
import { useNodesStore } from '~/stores/nodes.store';

function superscriptPlugin(md: MarkdownIt) {
  md.inline.ruler.after('emphasis', 'superscript', (state: StateInline, silent: boolean) => {
    const start = state.pos;
    if (state.src.charCodeAt(start) !== 0x5e /* ^ */) return false;

    let end = start + 1;
    while (end < state.src.length && state.src.charCodeAt(end) !== 0x5e /* ^ */) {
      end++;
    }

    if (end === start + 1 || end >= state.src.length) return false;

    if (!silent) {
      state.push('sup_open', 'sup', 1);
      state.push('text', '', 0).content = state.src.slice(start + 1, end);
      state.push('sup_close', 'sup', -1);
    }

    state.pos = end + 1;
    return true;
  });

  md.renderer.rules.sup_open = () => '<sup>';
  md.renderer.rules.sup_close = () => '</sup>';
}

function subscriptPlugin(md: MarkdownIt) {
  md.inline.ruler.after('emphasis', 'subscript', (state: StateInline, silent: boolean) => {
    const start = state.pos;
    if (state.src.charCodeAt(start) !== 0x7e /* ~ */) return false;

    let end = start + 1;
    while (end < state.src.length && state.src.charCodeAt(end) !== 0x7e /* ~ */) {
      end++;
    }

    if (end === start + 1 || end >= state.src.length) return false;

    if (!silent) {
      state.push('sub_open', 'sub', 1);
      state.push('text', '', 0).content = state.src.slice(start + 1, end);
      state.push('sub_close', 'sub', -1);
    }

    state.pos = end + 1;
    return true;
  });

  md.renderer.rules.sub_open = () => '<sub>';
  md.renderer.rules.sub_close = () => '</sub>';
}

function footNotePlugin(md: MarkdownIt) {
  const footnotes: Map<string, string> = new Map();

  // Parse footnote definitions: [^id]: content
  md.block.ruler.before('reference', 'footnote_def', (state, startLine, endLine, silent) => {
    const start = state.bMarks[startLine]! + state.tShift[startLine]!;
    const max = state.eMarks[startLine];
    const line = state.src.slice(start, max);

    const match = line.match(/^\[\^([^\]]+)\]:\s*(.*)$/);
    if (!match) return false;

    if (silent) return true;

    const id = match[1]!;
    const content = match[2]!;
    footnotes.set(id, content);

    state.line = startLine + 1;
    return true;
  });

  // Parse footnote references: [^id]
  md.inline.ruler.after('emphasis', 'footnote_ref', (state: StateInline, silent: boolean) => {
    const start = state.pos;
    const max = state.posMax;

    if (start + 2 >= max) return false;
    if (state.src.charCodeAt(start) !== 0x5b /* [ */) return false;
    if (state.src.charCodeAt(start + 1) !== 0x5e /* ^ */) return false;

    let end = start + 2;
    while (end < max && state.src.charCodeAt(end) !== 0x5d /* ] */) {
      end++;
    }

    if (end >= max) return false;

    const id = state.src.slice(start + 2, end);
    if (!id) return false;

    if (!silent) {
      const token = state.push('footnote_ref', '', 0);
      token.meta = { id };
    }

    state.pos = end + 1;
    return true;
  });

  md.renderer.rules.footnote_ref = (tokens, idx) => {
    const id = String(tokens[idx]?.meta?.id ?? '');
    const content = footnotes.get(id) || '';
    return `<sup class="footnote-ref"><a href="#fn-${id}" id="fnref-${id}" title="${md.utils.escapeHtml(content)}">[${id}]</a></sup>`;
  };

  // Add footnotes section at the end
  const originalRender = md.render.bind(md);
  md.render = (src: string, env?: Env) => {
    footnotes.clear();
    let result = originalRender(src, env);

    if (footnotes.size > 0) {
      result += '<section class="footnotes"><hr><ol>';
      footnotes.forEach((content, id) => {
        result += `<li id="fn-${id}">${md.renderInline(content)} <a href="#fnref-${id}" class="footnote-backref">↩</a></li>`;
      });
      result += '</ol></section>';
    }

    return result;
  };
}

const VIDEO_REGEX = /\.(mp4|webm|ogg|ogv|mov|avi|mkv|flv|wmv)([?#].*)?$/i;
const AUDIO_REGEX = /\.(mp3|wav|ogg|m4a|flac|aac)([?#].*)?$/i;
const PDF_REGEX = /\.pdf([?#].*)?$/i;
// Bare resource ids already fetch-triggered (rules re-run on store changes;
// without this each recompile would refire missing ids).
const bareFetchTried = new Set<string>();
// Keep in sync with backend-rs/src/utils.rs (CODE_EXTS). Named resource URLs
// (/api/resources/{id}/file/{name}) carry the extension; legacy bare URLs
// resolve the stored mime via the nodes store (see matchSrc below).
const CODE_REGEX
  = /\.(py|rs|go|js|mjs|cjs|ts|tsx|jsx|java|c|h|cpp|hpp|cs|rb|php|sh|bash|zsh|fish|sql|r|swift|kt|scala|pl|lua|toml|yaml|yml|json|xml|ini|md|markdown|txt|csv|diff|log|dockerfile|makefile|bat|ps1|vim|el)([?#].*)?$/i;

function html5MediaPlugin(md: MarkdownIt) {
  const defaultImageRenderer =
    md.renderer.rules.image ||
    function (tokens, idx, options, env, self) {
      return self.renderToken(tokens, idx, options);
    };

  md.renderer.rules.image = (tokens, idx, options, env, self) => {
    const token = tokens[idx];
    if (!token) return '';

    const src = String(token.attrGet('src') ?? '');
    let alt = token.content || '';

    // Parse: "Alt text | 100x200", "Alt | 100x", "Alt | x200", "Alt | 100"
    const match = alt.match(/^(.*?)\s*\|\s*(?:(\d+)?x(\d+)?|(\d+))\s*$/);

    if (match) {
      alt = match[1]!.trim();
      token.content = alt;

      const width = match[4] || match[2];
      const height = match[3];

      if (width) {
        token.attrSet('width', width);
        token.attrJoin('style', `width:${width}px;`);
      }
      if (height) {
        token.attrSet('height', height);
        token.attrJoin('style', `height:${height}px;`);
      }
    }

    // Legacy bare resource URLs (/api/resources/{id}/file, no filename)
    // predate the named route: resolve the stored mime so media/PDF/code
    // dispatch by inferred extension. Match-only — output keeps real src.
    // getById registers a dep, so landing rows recompile automatically.
    let matchSrc = src;
    const bare = src.match(/\/api\/resources\/([0-9a-fA-F-]{36})\/file\/?$/);
    if (bare) {
      try {
        const store = useNodesStore();
        const id = bare[1]!;
        const node = store.getById(id);
        if (!node && !bareFetchTried.has(id)) {
          bareFetchTried.add(id);
          // Failed fetches re-arm: the next unrelated store change
          // recompiles and retries (no timer, no hot loop).
          void store.fetch({ id }).catch(() => bareFetchTried.delete(id));
        }
        const filetype = (node?.metadata as Record<string, unknown> | undefined)?.filetype;
        if (typeof filetype === 'string') {
          const ext = mimeToExt(filetype);
          if (ext) matchSrc = `${src}.${ext}`;
        }
      } catch {
        // No store (SSR/tests): extension matching only.
      }
    }


    const escSrc = md.utils.escapeHtml(src);
    const escAlt = md.utils.escapeHtml(alt);
    if (matchSrc.match(VIDEO_REGEX)) {
      return `<video controls preload="metadata" style="max-width:100%" crossorigin="use-credentials" src="${escSrc}">${escAlt}</video>`;
    }

    if (matchSrc.match(AUDIO_REGEX)) {
      return `<audio controls preload="metadata" style="max-width:100%" crossorigin="use-credentials" src="${escSrc}">${escAlt}</audio>`;
    }

    if (matchSrc.match(PDF_REGEX)) {
      // Placeholder: ContentCompiled mounts PDFViewer here after render
      // (same post-mount pattern as mermaid diagrams).
      return `<div class="pdf-embed" data-src="${escSrc}" data-alt="${escAlt}"></div>`;
    }

    if (matchSrc.match(CODE_REGEX)) {
      // Placeholder: ContentCompiled hydrates the fetched text into a
      // highlighted code block (code-embed.ts), falling back to a link.
      return `<div class="code-embed" data-src="${escSrc}" data-name="${escAlt}"></div>`;
    }

    return defaultImageRenderer(tokens, idx, options, env, self);
  };
}

const SVG_REGEX = /\.svg(\?.*)?$/i;

function svgObjectPlugin(md: MarkdownIt) {
  const defaultImageRenderer =
    md.renderer.rules.image ||
    function (tokens, idx, options, env, self) {
      return self.renderToken(tokens, idx, options);
    };

  md.renderer.rules.image = (tokens, idx, options, env, self) => {
    const token = tokens[idx];
    if (!token) return '';

    const src = String(token.attrGet('src') ?? '');
    const alt = token.content || '';
    const title = String(token.attrGet('title') ?? '');

    if (SVG_REGEX.test(src)) {
      const escapedSrc = md.utils.escapeHtml(src);
      const escapedAlt = md.utils.escapeHtml(alt);
      const escapedTitle = md.utils.escapeHtml(title);

      return `
<object
  data="${escapedSrc}"
  type="image/svg+xml"
  aria-label="${escapedAlt}"
  ${escapedTitle ? `title="${escapedTitle}"` : ''}
>
  ${escapedAlt}
  </object>`;
    }

    return defaultImageRenderer(tokens, idx, options, env, self);
  };
}

export { superscriptPlugin, subscriptPlugin, footNotePlugin, html5MediaPlugin, svgObjectPlugin };
