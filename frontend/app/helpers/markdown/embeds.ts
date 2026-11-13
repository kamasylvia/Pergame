import type { MarkdownIt } from 'markdown-it';

/**
 * External embeds (Joplin parity): a bare whitelisted video URL on its own
 * line renders a click-to-load shell instead of an auto-playing iframe
 * (privacy: no third-party request until the user consents).
 *
 * Supported: YouTube (watch/youtu.be/embed), Bilibili (BV), Vimeo.
 */
interface EmbedTarget {
  kind: 'youtube' | 'bilibili' | 'vimeo';
  embedUrl: string;
  title: string;
}

function parseTarget(url: string): EmbedTarget | null {
  let u: URL;
  try {
    u = new URL(url);
  } catch {
    return null;
  }
  const host = u.hostname.replace(/^www\./, '').toLowerCase();

  // YouTube: youtube.com/watch?v=ID, youtu.be/ID, youtube.com/embed/ID
  if (host === 'youtube.com' || host === 'youtu.be' || host === 'youtube-nocookie.com') {
    const id = host === 'youtu.be'
      ? u.pathname.slice(1).split('/')[0]
      : host.endsWith('youtube-nocookie.com')
        ? u.pathname.split('/').pop()
        : (u.searchParams.get('v') ?? u.pathname.split('/').pop());
    if (id && /^[\w-]{6,20}$/.test(id)) {
      return {
        kind: 'youtube',
        embedUrl: `https://www.youtube-nocookie.com/embed/${id}`,
        title: 'YouTube video',
      };
    }
    return null;
  }

  // Bilibili: bilibili.com/video/BVxxxx
  if (host === 'bilibili.com' || host === 'www.bilibili.com') {
    const m = u.pathname.match(/\/video\/(BV\w+)/);
    if (m) {
      return {
        kind: 'bilibili',
        embedUrl: `https://player.bilibili.com/player.html?bvid=${m[1]}`,
        title: 'Bilibili video',
      };
    }
    return null;
  }

  // Vimeo: vimeo.com/123456
  if (host === 'vimeo.com') {
    const m = u.pathname.match(/^\/(\d+)/);
    if (m) {
      return {
        kind: 'vimeo',
        embedUrl: `https://player.vimeo.com/video/${m[1]}`,
        title: 'Vimeo video',
      };
    }
    return null;
  }

  return null;
}

const URL_LINE_RE = /^https?:\/\/\S+$/;

export function embedsPlugin(md: MarkdownIt): void {
  // Post-process linkified bare URLs: link_open(text is the URL itself) →
  // consent shell. Runs as a core ruler pass over inline children.
  md.core.ruler.push('joplin_embeds', (state) => {
    for (const block of state.tokens) {
      if (block.type !== 'inline' || !block.children) continue;
      const kids = block.children;
      // Exactly one link whose text equals its href (bare URL line).
      if (kids.length !== 3) continue;
      const open = kids[0];
      const text = kids[1];
      const close = kids[2];
      if (!open || !text || !close) continue;
      if (open.type !== 'link_open' || text.type !== 'text' || close.type !== 'link_close') continue;
      const href = open.attrGet('href');
      if (!href || text.content !== href || !URL_LINE_RE.test(href.trim())) continue;
      const target = parseTarget(href.trim());
      if (!target) continue;
      const escUrl = target.embedUrl.replace(/&/g, '&amp;').replace(/"/g, '&quot;');
      const shell
        = `<div class="ext-embed" data-kind="${target.kind}" data-embed="${escUrl}">`
        + `<button type="button" class="ext-embed-consent">Load ${target.title}</button>`
        + `</div>\n`;
      // Replace the whole inline block with the shell marker.
      block.type = 'html_block';
      block.children = null;
      block.content = shell;
    }
  });
}

/** Hydrate consent shells under root (called after render, mermaid-style). */
export function renderExtEmbeds(root: HTMLElement): void {
  const shells = Array.from(root.querySelectorAll<HTMLElement>('.ext-embed[data-embed]:not([data-loaded])'));
  for (const el of shells) {
    const btn = el.querySelector('button');
    if (!btn) continue;
    btn.addEventListener('click', () => {
      const src = el.dataset.embed;
      if (!src) return;
      el.dataset.loaded = '1';
      el.textContent = '';
      const frame = document.createElement('iframe');
      frame.src = src;
      frame.title = el.dataset.kind ?? 'Embedded video';
      frame.loading = 'lazy';
      frame.allow = 'accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture';
      frame.allowFullscreen = true;
      el.appendChild(frame);
    }, { once: true });
  }
}
