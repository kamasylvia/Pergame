/** URL → content type, in ReadItLater's priority order (trimmed set). */
import type { ContentType } from './types';

interface Rule {
  type: ContentType;
  test: RegExp;
}

const RULES: Rule[] = [
  { type: 'zhihu', test: /^https?:\/\/(?:www\.)?zhihu\.com\/(?:question\/\d+\/answer\/\d+|p\/\d+)/i },
  { type: 'youtube-channel', test: /^https?:\/\/(?:www\.)?youtube\.com\/(?:@|channel\/|c\/|user\/)/i },
  { type: 'youtube', test: /^https?:\/\/(?:www\.|m\.)?(?:youtube\.com\/(?:watch\?|shorts\/|embed\/)|youtu\.be\/)/i },
  { type: 'bilibili', test: /^https?:\/\/(?:www\.|m\.)?bilibili\.com\/video\//i },
  { type: 'vimeo', test: /^https?:\/\/(?:www\.)?vimeo\.com\/\d+/i },
  { type: 'tiktok', test: /^https?:\/\/(?:www\.)?tiktok\.com\/@[^/]+\/video\/\d+/i },
  { type: 'twitter', test: /^https?:\/\/(?:www\.)?(?:twitter\.com|x\.com)\/[^/]+\/status\/\d+/i },
  { type: 'bluesky', test: /^https?:\/\/(?:www\.)?bsky\.app\/profile\/[^/]+\/post\/[^/?#]+/i },
  { type: 'mastodon', test: /^https?:\/\/[^/]+\/@[^/]+\/\d+/i },
  { type: 'stackexchange', test: /^https?:\/\/(?:[a-z0-9-]+\.)?(?:stackexchange|stackoverflow|superuser|serverfault|askubuntu)\.com\/(?:questions|q)\/\d+/i },
  { type: 'github', test: /^https?:\/\/(?:www\.)?github\.com\/[^/]+\/[^/]+\/?(?:(?:issues|pull)\/\d+)?$/i },
  { type: 'wikipedia', test: /^https?:\/\/[a-z-]+\.wikipedia\.org\/wiki\//i },
  { type: 'pinterest', test: /^https?:\/\/(?:www\.)?pinterest\.[a-z.]+\/pin\/\d+/i },
];

/** True when the text is a single http(s) URL. */
export function isUrl(text: string): boolean {
  return /^https?:\/\/\S+$/i.test(text.trim());
}

/** Content type for one URL (`website` when nothing specific matches). */
export function detectType(url: string): ContentType {
  const trimmed = url.trim();
  return RULES.find(rule => rule.test.test(trimmed))?.type ?? 'website';
}

/** Split a clipboard batch into URLs (newlines, commas, or spaces). */
export function splitBatch(text: string): string[] {
  return text
    .split(/[\s,;]+/)
    .map(part => part.trim())
    .filter(part => isUrl(part));
}
