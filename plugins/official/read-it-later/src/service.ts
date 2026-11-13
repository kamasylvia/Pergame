/** Clip orchestration: detect → parse → template → save. */
import { detectType, isUrl, splitBatch } from './detect';
import { parseText } from './parsers/text';
import { parseWebsite } from './parsers/website';
import { parseOEmbed } from './parsers/oembed';
import { parseBluesky, parseGithub, parseMastodon, parseStackExchange, parseWikipedia } from './parsers/api';
import { parseZhihu } from './parsers/zhihu';
import { renderTemplate } from './template';
import type { ClipOutcome, ParsedContent, ParserContext } from './types';

/** Settings keys the plugin owns. */
export const SETTINGS = {
  parentId: 'parent_id',
  tags: 'default_tags',
  templatePrefix: 'template_',
  zhihuCookie: 'zhihu_cookie',
} as const;

/** Default body templates per content type. */
export const DEFAULT_TEMPLATES: Record<string, string> = {
  website: '> {{ description }}\n\n[Source]({{ url }})\n\n{{ content }}\n',
  zhihu: '[Source]({{ url }}){{ author | default("") }}\n\n{{ content }}\n',
  youtube: '{{ content }}\n\n[Watch on YouTube]({{ url }})\n',
  'youtube-channel': '{{ content }}\n\n[Channel]({{ url }})\n',
  bilibili: '{{ content }}\n\n[Watch on Bilibili]({{ url }})\n',
  vimeo: '{{ content }}\n\n[Watch on Vimeo]({{ url }})\n',
  tiktok: '{{ content }}\n\n[Watch on TikTok]({{ url }})\n',
  twitter: '[Source]({{ url }})\n\n{{ content }}\n',
  bluesky: '[Source]({{ url }})\n\n{{ content }}\n',
  mastodon: '[Source]({{ url }})\n\n{{ content }}\n',
  stackexchange: '[Source]({{ url }})\n\n{{ content }}\n',
  github: '[Source]({{ url }})\n\n{{ content }}\n',
  wikipedia: '{{ content }}\n',
  pinterest: '[Source]({{ url }})\n\n{{ content }}\n',
  text: '{{ content }}\n',
};

/** Everything the service needs from the host. */
export interface ClipHost {
  ctx: ParserContext;
  createNote(input: { name: string; content: string; parent_id?: string; tags?: string }): Promise<{ id: string }>;
  notify(message: string, opts?: { type?: 'info' | 'success' | 'warning' | 'error'; timeout?: number }): void;
}

/** Parser dispatch, in detection order. */
async function parseByType(url: string, type: string, ctx: ParserContext): Promise<ParsedContent> {
  switch (type) {
    case 'zhihu':
      return (await parseZhihu(url, ctx)) ?? parseWebsite(url, ctx);
    case 'youtube':
    case 'bilibili':
    case 'vimeo':
    case 'tiktok': {
      const embedded = await parseOEmbed(url, ctx);
      return embedded ?? parseWebsite(url, ctx);
    }
    case 'stackexchange':
      return (await parseStackExchange(url, ctx)) ?? parseWebsite(url, ctx);
    case 'bluesky':
      return (await parseBluesky(url, ctx)) ?? parseWebsite(url, ctx);
    case 'mastodon':
      return (await parseMastodon(url, ctx)) ?? parseWebsite(url, ctx);
    case 'github':
      return (await parseGithub(url, ctx)) ?? parseWebsite(url, ctx);
    case 'wikipedia':
      return (await parseWikipedia(url, ctx)) ?? parseWebsite(url, ctx);
    default:
      return parseWebsite(url, ctx);
  }
}

/** Clip one URL (or clipboard text) into a note. */
export async function clip(host: ClipHost, input: string, overrides: { parentId?: string; tags?: string; title?: string } = {}): Promise<ClipOutcome> {
  const { ctx } = host;
  const trimmed = input.trim();
  if (!trimmed) throw new Error('nothing to clip');

  const type = isUrl(trimmed) ? detectType(trimmed) : 'text';
  const parsed = isUrl(trimmed) ? await parseByType(trimmed, type, ctx) : parseText(trimmed);

  const templateKey = `${SETTINGS.templatePrefix}${parsed.type}`;
  const template = (await ctx.setting<string>(templateKey)) || DEFAULT_TEMPLATES[parsed.type] || DEFAULT_TEMPLATES.website!;
  const body = renderTemplate(template, {
    ...parsed.extra,
    title: parsed.title,
    content: parsed.content,
    description: parsed.description ?? '',
    author: parsed.author ?? '',
    published: parsed.published ?? '',
    url: parsed.url ?? '',
    tags: parsed.tags ?? [],
  });

  const parentId = overrides.parentId || (await ctx.setting<string>(SETTINGS.parentId)) || undefined;
  const tags = overrides.tags ?? (await ctx.setting<string>(SETTINGS.tags)) ?? '';
  const name = (overrides.title || parsed.title || 'Clipped note').slice(0, 60);
  const note = await host.createNote({
    name,
    content: body,
    parent_id: parentId,
    tags: tags || undefined,
  });

  host.notify(`Clipped ${name}`, { type: 'success', timeout: 4000 });
  return { type: parsed.type, title: name, noteId: note.id, images: parsed.images ?? 0 };
}

/** Clip every URL in a clipboard batch, reporting failures instead of aborting. */
export async function clipBatch(host: ClipHost, text: string, overrides: { parentId?: string; tags?: string } = {}): Promise<{ ok: number; failed: number }> {
  const urls = splitBatch(text);
  if (!urls.length) {
    host.notify('No URLs in the clipboard', { type: 'warning' });
    return { ok: 0, failed: 0 };
  }
  let ok = 0;
  let failed = 0;
  for (const url of urls) {
    try {
      await clip(host, url, overrides);
      ok += 1;
    } catch (err) {
      failed += 1;
      const reason = err instanceof Error ? err.message : String(err);
      host.notify(`Clip failed: ${url} — ${reason}`, { type: 'error', timeout: 6000 });
    }
  }
  host.notify(`Clipped ${ok}/${urls.length}${failed ? ` (${failed} failed)` : ''}`, { type: failed ? 'warning' : 'success' });
  return { ok, failed };
}
