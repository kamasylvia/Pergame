/** Video/audio embeds via public oEmbed endpoints (no API keys). */
import type { ContentType, ParsedContent, ParserContext } from '../types';

interface OEmbedConfig {
  type: ContentType;
  endpoint(url: string): string | null;
  embed(url: string): string;
}

const CONFIGS: OEmbedConfig[] = [
  {
    type: 'youtube',
    endpoint: url => `https://www.youtube.com/oembed?format=json&url=${encodeURIComponent(url)}`,
    embed: url => {
      const id = url.match(/(?:v=|youtu\.be\/|shorts\/|embed\/)([\w-]{6,})/)?.[1] ?? '';
      return `<iframe width="100%" height="420" src="https://www.youtube-nocookie.com/embed/${id}" title="YouTube" frameborder="0" allowfullscreen></iframe>`;
    },
  },
  {
    type: 'bilibili',
    endpoint: url => `https://www.bilibili.com/oembed?format=json&url=${encodeURIComponent(url)}`,
    embed: url => {
      const id = url.match(/\/video\/(BV[\w]+)/)?.[1] ?? '';
      return `<iframe width="100%" height="420" src="https://player.bilibili.com/player.html?bvid=${id}" frameborder="0" allowfullscreen></iframe>`;
    },
  },
  {
    type: 'vimeo',
    endpoint: url => `https://vimeo.com/api/oembed.json?url=${encodeURIComponent(url)}`,
    embed: url => {
      const id = url.match(/vimeo\.com\/(\d+)/)?.[1] ?? '';
      return `<iframe width="100%" height="420" src="https://player.vimeo.com/video/${id}" frameborder="0" allowfullscreen></iframe>`;
    },
  },
  {
    type: 'tiktok',
    endpoint: url => `https://www.tiktok.com/oembed?url=${encodeURIComponent(url)}`,
    embed: url => `[Watch on TikTok](${url})`,
  },
];

interface OEmbedResponse {
  title?: string;
  author_name?: string;
  provider_name?: string;
}

/** Parse one video URL; `null` when no config matches. */
export async function parseOEmbed(url: string, ctx: ParserContext): Promise<ParsedContent | null> {
  const config = CONFIGS.find(candidate => candidate.endpoint(url));
  if (!config) return null;
  const endpoint = config.endpoint(url)!;
  let meta: OEmbedResponse = {};
  try {
    const response = await ctx.proxy(endpoint);
    if (response.status < 400) meta = JSON.parse(response.body) as OEmbedResponse;
  } catch {
    // oEmbed down: still save the embed.
  }
  const title = meta.title || url;
  return {
    type: config.type,
    title,
    content: `${config.embed(url)}\n`,
    description: meta.author_name ? `by ${meta.author_name}` : '',
    author: meta.author_name,
    url,
    tags: [],
    extra: { provider: meta.provider_name ?? config.type },
  };
}
