/** Article pipeline: Readability → Turndown → images as attachments. */
import { Readability } from '@mozilla/readability';
import TurndownService from 'turndown';
import { gfm } from 'turndown-plugin-gfm';
import type { ParsedContent, ParserContext } from '../types';

const MAX_IMAGES = 40;

function turndown(): TurndownService {
  const service = new TurndownService({ headingStyle: 'atx', codeBlockStyle: 'fenced', hr: '---' });
  service.use(gfm);
  service.remove(['script', 'style', 'noscript', 'iframe']);
  return service;
}

function absolute(url: string, base: string): string | null {
  try {
    return new URL(url, base).toString();
  } catch {
    return null;
  }
}

function base64ToBytes(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

/** Fetch every image and replace its `src` with the attachment URL. */
async function rewriteImages(
  html: string,
  pageUrl: string,
  ctx: ParserContext,
): Promise<{ html: string; images: number }> {
  // Readability returns a fragment; wrap it so `body` always holds the
  // content (some DOMParser implementations drop a bare fragment).
  const doc = new DOMParser().parseFromString(`<!doctype html><html><body>${html}</body></html>`, 'text/html');
  const sources = Array.from(doc.querySelectorAll('img[src]')).slice(0, MAX_IMAGES);
  let images = 0;
  for (const [index, img] of sources.entries()) {
    const original = absolute(img.getAttribute('src') ?? '', pageUrl);
    if (!original) continue;
    try {
      const binary = await ctx.proxyBinary(original);
      const extension = (binary.contentType.split('/')[1] ?? 'bin').split(';')[0];
      const stored = await ctx.upload(base64ToBytes(binary.base64), `clip-${Date.now()}-${index}.${extension}`, binary.contentType);
      img.setAttribute('src', stored);
      images += 1;
    } catch {
      // Keep the remote URL: a broken image beats a failed clip.
      img.setAttribute('src', original);
    }
  }
  // Relative links would break once the note leaves the page context.
  for (const anchor of Array.from(doc.querySelectorAll('a[href]'))) {
    const href = anchor.getAttribute('href') ?? '';
    if (href.startsWith('#') || /^[a-z][a-z0-9+.-]*:/i.test(href)) continue;
    const absoluteHref = absolute(href, pageUrl);
    if (absoluteHref) anchor.setAttribute('href', absoluteHref);
  }
  return { html: doc.body.innerHTML, images };
}

/** Parse one web page into markdown. */
export async function parseWebsite(url: string, ctx: ParserContext): Promise<ParsedContent> {
  const response = await ctx.proxy(url);
  if (response.status >= 400) {
    throw new Error(`page answered ${response.status}`);
  }
  const document = new DOMParser().parseFromString(response.body, 'text/html');
  const article = new Readability(document, { charThreshold: 120 }).parse();
  if (!article?.content) {
    // Unreadable page: keep the link so the note is still useful.
    return {
      type: 'website',
      title: document.title || url,
      content: `[${document.title || url}](${url})\n`,
      url,
      description: '',
      images: 0,
    };
  }

  const { html, images } = await rewriteImages(article.content, url, ctx);
  const markdown = turndown().turndown(html);
  return {
    type: 'website',
    title: (article.title || document.title || url).trim(),
    content: markdown,
    description: (article.excerpt ?? '').slice(0, 240),
    author: article.byline ?? undefined,
    published: article.publishedTime ?? undefined,
    url,
    tags: [],
    images,
    extra: { wordCount: article.length ?? 0 },
  };
}
