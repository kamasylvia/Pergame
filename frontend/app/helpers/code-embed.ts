import hljs from 'highlight.js/lib/common';
import { codeLanguage } from '~/helpers/resources';

// Hydrates `.code-embed[data-src]` placeholders (emitted by html5MediaPlugin
// for named resource URLs ending in a code extension, see CODE_REGEX in
// helpers/markdown/other.ts) into highlighted code blocks with the same
// wrapper structure as copyCodePlugin — so the copy button and collapse
// toggle of ContentCompiled work on them unchanged. Failures (private
// resource, too large, non-UTF-8) fall back to a plain link.

/** Blocks with more lines than this start collapsed (copyCodePlugin parity). */
const COLLAPSE_THRESHOLD = 16;
/** Refuse to render bodies beyond this size as a code block. */
const MAX_BYTES = 256 * 1024;

const ICON_COPY = `<svg class="copy-icon" xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>`;
const ICON_CHECK = `<svg class="check-icon" xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>`;

function esc(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

/** Highlight once; unknown languages and hljs failures fall back to escaped text. */
export function highlightCode(name: string, code: string): { lang: string; html: string } {
  const lang = codeLanguage(name);
  try {
    if (lang && hljs.getLanguage(lang)) return { lang, html: hljs.highlight(code, { language: lang }).value };
  } catch {
    /* fall through to plain text */
  }
  return { lang: lang || 'plaintext', html: esc(code) };
}

function renderInto(el: HTMLElement, name: string, code: string) {
  const { lang, html: body } = highlightCode(name, code);
  const lines = code.replace(/\n$/, '').split('\n').length;
  const collapsible = lines > COLLAPSE_THRESHOLD;
  const footer = collapsible
    ? `<div class="code-block-footer">
         <button type="button" class="code-toggle-btn" aria-expanded="false">
           <span class="toggle-label-expand">${lines} lines · expand</span>
           <span class="toggle-label-collapse">collapse</span>
         </button>
       </div>`
    : '';
  el.innerHTML = `<div class="code-block-wrapper${collapsible ? ' collapsed' : ''}">
  <div class="code-block-header">
    <span class="code-embed-name">${esc(name)}</span>
    <button type="button" class="code-copy-btn" aria-label="Copy code" title="Copy code">
      <span class="btn-icon">${ICON_COPY}</span>
      <span class="btn-icon-success">${ICON_CHECK}</span>
    </button>
  </div>
  <pre class="hljs"><code class="language-${esc(lang || 'plaintext')}">${body}</code></pre>
  ${footer}
</div>`;
}

function fallbackLink(el: HTMLElement, src: string, name: string) {
  el.innerHTML = `<a href="${esc(src)}" target="_blank" rel="noopener">${esc(name || src)}</a>`;
}

/**
 * Fetches a resource as text for code display. Rejects when the body is
 * missing, beyond MAX_BYTES, or looks binary (lone replacement char).
 */
export async function fetchCodeText(src: string): Promise<string> {
  const resp = await fetch(src, { credentials: 'include' });
  if (!resp.ok) throw new Error(`fetch ${resp.status}`);
  const len = Number(resp.headers.get('content-length') || '0');
  if (len > MAX_BYTES) throw new Error('too large');
  const text = await resp.text();
  if (text.length > MAX_BYTES || (text.length > 0 && text.includes('\uFFFD'))) {
    throw new Error(text.length > MAX_BYTES ? 'too large' : 'binary');
  }
  return text;
}

/**
 * Fetches each placeholder's resource text and swaps in the code block.
 * re-render marks handled placeholders with data-code-embed-done.
 */
export async function renderCodeEmbeds(root: HTMLElement): Promise<void> {
  const placeholders = Array.from(root.querySelectorAll<HTMLElement>('div.code-embed[data-src]:not([data-code-embed-done])'));
  for (const el of placeholders) {
    el.dataset.codeEmbedDone = '1';
    const src = el.dataset.src ?? '';
    const name = el.dataset.name || src.split('/').pop() || '';
    if (!src) continue;
    try {
      renderInto(el, name, await fetchCodeText(src));
    } catch {
      fallbackLink(el, src, name);
    }
  }
}
