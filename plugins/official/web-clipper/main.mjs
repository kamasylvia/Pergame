// Minimal clipper: server-side fetch (ctx.net.proxy) → readable text → note.
// Site-specific parsers from ReadItLater are intentionally not ported: the
// proxy returns the page and this keeps the main article text.
const MAX_CHARS = 200000;

function pickTitle(html) {
  const match = html.match(/<title[^>]*>([\s\S]*?)<\/title>/i);
  return (match?.[1] ?? '').replace(/\s+/g, ' ').trim();
}

function toText(html) {
  const doc = new DOMParser().parseFromString(html, 'text/html');
  for (const node of doc.querySelectorAll('script, style, noscript, iframe, svg')) node.remove();
  const article = doc.querySelector('article, main, [role="main"]') ?? doc.body;
  const text = (article?.textContent ?? '').replace(/[ \t]+\n/g, '\n').replace(/\n{3,}/g, '\n\n').trim();
  return { text: text.slice(0, MAX_CHARS), title: pickTitle(html) };
}

export default class WebClipper {
  async onload(ctx) {
    ctx.ui.command('clip', 'Web clipper: clip a URL', async () => {
      const url = window.prompt('URL to clip');
      if (!url) return;
      let page;
      try {
        page = await ctx.net.proxy(url);
      } catch (err) {
        ctx.ui.notice(`Clip failed: ${err.message}`, { type: 'error', timeout: 8000 });
        return;
      }
      if (page.status >= 400) {
        ctx.ui.notice(`Clip failed: HTTP ${page.status}`, { type: 'error' });
        return;
      }
      const { text, title } = toText(page.body);
      if (!text) {
        ctx.ui.notice('Clip failed: no readable text (try pasting the article)', { type: 'error' });
        return;
      }
      const name = (title || new URL(url).hostname).slice(0, 60);
      const content = `# ${name}\n\nSource: ${url}\n\n${text}\n`;
      const nodes = await ctx.nodes.listChildren();
      const workspace = nodes.find(node => node.role === 1);
      await ctx.nodes.create({ name, content, parent_id: workspace?.id, role: 3 });
      ctx.ui.notice(`Clipped ${name} (${text.length} chars)`, { type: 'success' });
    });
  }
}
