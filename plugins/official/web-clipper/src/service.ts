/** Clip orchestration: detect → parse → template → save. */
import { detectType, isUrl, splitBatch } from './detect';
import { parseText } from './parsers/text';
import { parseWebsite } from './parsers/website';
import { parseOEmbed } from './parsers/oembed';
import { parseBluesky, parseGithub, parseMastodon, parseStackExchange, parseWikipedia } from './parsers/api';
import { parseZhihu } from './parsers/zhihu';
import { renderTemplate } from './template';
import type { ClipOutcome, ContentType, ParsedContent, ParserContext } from './types';

/**
 * Settings keys the plugin owns. Content types follow the SiYuan ReadItLater
 * shape: each type carries its own slug (folder + tag), title template and
 * body template, so one plugin can file thirteen kinds of clips differently.
 *
 *   <type>_slug    — folder path segment and tag for that type
 *   <type>_title   — note title template
 *   <type>_body    — note body template
 */
export const SETTINGS = {
  workspaceId: 'workspace_id',
  inboxDir: 'inbox_dir',
  dateTitleFmt: 'date_title_fmt',
  fileExistsStrategy: 'file_exists_strategy',
  batchDelimiter: 'batch_delimiter',
  zhihuCookie: 'zhihu_cookie',
  slugSuffix: '_slug',
  titleSuffix: '_title',
  bodySuffix: '_body',
} as const;

/** Content types that get their own slug/title/body settings group. */
export const SETTING_TYPES: ContentType[] = [
  'youtube',
  'bilibili',
  'vimeo',
  'tiktok',
  'twitter',
  'bluesky',
  'mastodon',
  'stackexchange',
  'pinterest',
  'website',
  'text',
];

/** Default slug per type: the folder name and tag a clip of that type gets. */
export const DEFAULT_SLUGS: Record<string, string> = {
  zhihu: 'Zhihu',
  youtube: 'YouTube',
  bilibili: 'Bilibili',
  vimeo: 'Vimeo',
  tiktok: 'TikTok',
  twitter: 'X',
  bluesky: 'Bluesky',
  mastodon: 'Mastodon',
  stackexchange: 'Stack Exchange',
  pinterest: 'Pinterest',
  website: 'Articles',
  text: 'Snippets',
};

/** Default title templates. `date` is rendered with the date format setting. */
export const DEFAULT_TITLES: Record<string, string> = {
  zhihu: '{{ title }}',
  youtube: '{{ title }}',
  bilibili: '{{ title }}',
  vimeo: '{{ title }}',
  tiktok: '{{ title }}',
  twitter: '{{ author }} · {{ date }}',
  bluesky: '{{ author }} · {{ date }}',
  mastodon: '{{ author }} · {{ date }}',
  stackexchange: '{{ title }}',
  pinterest: '{{ title }}',
  website: '{{ title }}',
  text: '{{ date }}',
};

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
  /**
   * Ensure a folder path exists under a workspace and return the leaf id.
   * `path` is a list of folder names; empty means the workspace itself.
   */
  ensureFolder(workspaceId: string | undefined, path: string[]): Promise<string | undefined>;
  notify(message: string, opts?: { type?: 'info' | 'success' | 'warning' | 'error'; timeout?: number }): void;
  /** Locale-bound wording (`done`/`failed`/… keys) from the host UI language. */
  text(key: string, vars?: Record<string, string | number>): string;
}

async function parseByType(url: string, type: string, ctx: ParserContext): Promise<ParsedContent> {
  switch (type) {
    case 'zhihu':
      return (await parseZhihu(url, ctx)) ?? parseWebsite(url, ctx);
    case 'bilibili':
    case 'youtube':
    case 'vimeo':
    case 'tiktok':
      return (await parseOEmbed(url, ctx, type)) ?? parseWebsite(url, ctx);
    case 'twitter':
      return (await parseBluesky(url, ctx)) ?? parseWebsite(url, ctx);
    case 'bluesky':
      return (await parseBluesky(url, ctx)) ?? parseWebsite(url, ctx);
    case 'mastodon':
      return (await parseMastodon(url, ctx)) ?? parseWebsite(url, ctx);
    case 'stackexchange':
      return (await parseStackExchange(url, ctx)) ?? parseWebsite(url, ctx);
    case 'pinterest':
      return (await parseWebsite(url, ctx)) ?? parseWebsite(url, ctx);
    default:
      return parseWebsite(url, ctx);
  }
}

/** Today's date through the plugin's own format setting, falling back to ISO. */
async function formatNow(ctx: ParserContext, key: string, fallback: string): Promise<string> {
  const fmt = (await ctx.setting<string>(key)) || fallback;
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, '0');
  // Token subset: YYYY, YY, MM, DD, HH, mm, ss. Anything else stays literal.
  return fmt
    .replace(/YYYY/g, String(now.getFullYear()))
    .replace(/YY/g, String(now.getFullYear()).slice(-2))
    .replace(/MM/g, pad(now.getMonth() + 1))
    .replace(/DD/g, pad(now.getDate()))
    .replace(/HH/g, pad(now.getHours()))
    .replace(/mm/g, pad(now.getMinutes()))
    .replace(/ss/g, pad(now.getSeconds()));
}

/** Clip one URL (or clipboard text) into a note. */
export async function clip(
  host: ClipHost,
  input: string,
  overrides: { workspaceId?: string; tags?: string; title?: string; onProgress?: (p: { percent: number; stage: string; label?: string }) => void } = {},
): Promise<ClipOutcome> {
  const { ctx } = host;
  const trimmed = input.trim();
  if (!trimmed) throw new Error('nothing to clip');

  const type = isUrl(trimmed) ? detectType(trimmed) : 'text';
  const parsed = isUrl(trimmed) ? await parseByType(trimmed, type, ctx) : parseText(trimmed);

  // The settings key follows the *detected* type, so each content kind can
  // carry its own slug, title template and body template.
  const kind = parsed.type;
  const slug = (await ctx.setting<string>(`${kind}${SETTINGS.slugSuffix}`)) || DEFAULT_SLUGS[kind] || '';
  const titleTemplate = (await ctx.setting<string>(`${kind}${SETTINGS.titleSuffix}`)) || DEFAULT_TITLES[kind] || '{{ title }}';
  const bodyTemplate =
    (await ctx.setting<string>(`${kind}${SETTINGS.bodySuffix}`)) || DEFAULT_TEMPLATES[kind] || DEFAULT_TEMPLATES.website!;

  const date = await formatNow(ctx, SETTINGS.dateTitleFmt, 'YYYY-MM-DD');
  const vars = {
    ...parsed.extra,
    title: parsed.title,
    content: parsed.content,
    description: parsed.description ?? '',
    author: parsed.author ?? '',
    published: parsed.published ?? '',
    url: parsed.url ?? '',
    tags: parsed.tags ?? [],
    date,
    contentType: slug,
  };

  const body = renderTemplate(bodyTemplate, vars);
  const renderedTitle = renderTemplate(titleTemplate, vars).trim();
  const name = (overrides.title || renderedTitle || parsed.title || 'Clipped note').slice(0, 60);

  // Folder: workspace / <inbox template rendered with contentType + date>.
  // `{{contentType}}` is the slug above, so thirteen kinds can each land in
  // their own folder without a per-type folder setting.
  const workspaceId = overrides.workspaceId || (await ctx.setting<string>(SETTINGS.workspaceId)) || undefined;
  const inboxTemplate = (await ctx.setting<string>(SETTINGS.inboxDir)) || '{{contentType}}';
  const inboxPath = renderTemplate(inboxTemplate, vars)
    .split('/')
    .map(segment => segment.trim())
    .filter(Boolean);
  const parentId = await host.ensureFolder(workspaceId, inboxPath);

  // The slug doubles as the tag, like ReadItLater's content type.
  const overrideTags = overrides.tags ?? '';
  const tags = overrideTags || slug;

  const note = await host.createNote({ name, content: body, parent_id: parentId, tags: tags || undefined });

  // Success toast is shown by the caller (same toast id as the progress).
  return { type: kind, title: name, noteId: note.id, images: parsed.images ?? 0 };
}

/** Clip every URL in a clipboard batch, reporting failures instead of aborting. */
export async function clipBatch(
  host: ClipHost,
  text: string,
  overrides: {
    workspaceId?: string;
    tags?: string;
    onProgress?: (p: { percent: number; stage: string; label?: string }) => void;
    /** False when the caller announces completion itself (dialog batch reuses the progress toast). */
    announceSummary?: boolean;
  } = {},
): Promise<{ ok: number; failed: number }> {
  const delimiter = (await host.ctx.setting<string>(SETTINGS.batchDelimiter)) || 'auto';
  const urls = splitBatch(text, delimiter);
  if (!urls.length) {
    host.notify(host.text('clipboardEmpty'), { type: 'warning' });
    return { ok: 0, failed: 0 };
  }
  let ok = 0;
  let failed = 0;
  for (const [index, url] of urls.entries()) {
    // Proxy-path clips emit no render progress of their own; without this
    // tick a batch of plain pages runs minutes in total silence.
    overrides.onProgress?.({
      percent: Math.round((index / urls.length) * 100),
      stage: 'batch',
      label: host.text('batchItem', { i: index + 1, total: urls.length, url: shortUrl(url) }),
    });
    try {
      await clip(host, url, overrides);
      ok += 1;
    } catch (err) {
      failed += 1;
      const reason = err instanceof Error ? err.message : String(err);
      host.notify(host.text('clipFailed', { url, reason }), { type: 'error', timeout: 6000 });
    }
  }
  if (overrides.announceSummary ?? true) {
    const summary = host.text('batchDone', { ok, total: urls.length }) + (failed ? host.text('batchFailedPart', { count: failed }) : '');
    host.notify(summary, { type: failed ? 'warning' : 'success' });
  }
  return { ok, failed };
}

/** Toast-sized URL: host plus a trimmed path, so the batch tick fits one line. */
function shortUrl(url: string): string {
  const bare = url.replace(/^https?:\/\//, '');
  return bare.length > 60 ? `${bare.slice(0, 57)}…` : bare;
}
