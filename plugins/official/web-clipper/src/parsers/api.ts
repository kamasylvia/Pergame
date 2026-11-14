/** Parsers backed by public JSON APIs: Stack Exchange, Bluesky, Mastodon,
 *  GitHub and Wikipedia. Each returns null when the URL is not its own. */
import type { ParsedContent, ParserContext } from '../types';

function decodeEntities(text: string): string {
  return text
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&');
}

/** Stack Exchange question + answers, via the official API. */
export async function parseStackExchange(url: string, ctx: ParserContext): Promise<ParsedContent | null> {
  const match = url.match(/^https?:\/\/([a-z0-9-]+)\.(?:stackexchange|com)\/questions\/(\d+)/i);
  if (!match) return null;
  const [, site, id] = match;
  const host = url.includes('stackoverflow') ? 'stackoverflow' : url.includes('superuser') ? 'superuser' : url.includes('serverfault') ? 'serverfault' : url.includes('askubuntu') ? 'askubuntu' : site;
  const api = `https://api.stackexchange.com/2.3/questions/${id}?site=${host}&filter=withbody`;
  const answersApi = `https://api.stackexchange.com/2.3/questions/${id}/answers?site=${host}&filter=withbody&sort=votes&order=desc&pagesize=5`;
  const [question, answers] = await Promise.all([ctx.proxy(api), ctx.proxy(answersApi)]);
  if (question.status >= 400) return null;
  const parsed = JSON.parse(question.body) as { items?: Array<Record<string, unknown>> };
  const item = parsed.items?.[0];
  if (!item) return null;
  const answerBody = (() => {
    try {
      const list = JSON.parse(answers.body) as { items?: Array<Record<string, unknown>> };
      const best = list.items?.[0];
      return best ? `\n## Best answer\n\n${decodeEntities(String(best.body ?? ''))}\n` : '';
    } catch {
      return '';
    }
  })();
  return {
    type: 'stackexchange',
    title: decodeEntities(String(item.title ?? url)),
    content: `${decodeEntities(String(item.body ?? ''))}\n${answerBody}`,
    description: `${String(item.answer_count ?? 0)} answers · score ${String(item.score ?? 0)}`,
    url,
    tags: Array.isArray(item.tags) ? (item.tags as string[]) : [],
    extra: { score: item.score ?? 0, answers: item.answer_count ?? 0 },
  };
}

/** Bluesky post, via the public AppView API. */
export async function parseBluesky(url: string, ctx: ParserContext): Promise<ParsedContent | null> {
  const match = url.match(/bsky\.app\/profile\/([^/]+)\/post\/([^/?#]+)/i);
  if (!match) return null;
  const [, handle, rkey] = match;
  const atUri = `at://${handle}/app.bsky.feed.post/${rkey}`;
  const api = `https://public.api.bsky.app/xrpc/app.bsky.feed.getPostThread?uri=${encodeURIComponent(atUri)}&depth=0`;
  const response = await ctx.proxy(api);
  if (response.status >= 400) return null;
  const data = JSON.parse(response.body) as {
    thread?: { post?: { record?: { text?: string; createdAt?: string }; author?: { displayName?: string; handle?: string }; likeCount?: number; embed?: { images?: Array<{ fullsize?: string }> } } };
  };
  const post = data.thread?.post;
  if (!post?.record?.text) return null;
  const images = post.embed?.images ?? [];
  const imageMarkdown = images
    .map(image => image.fullsize)
    .filter((src): src is string => Boolean(src))
    .map(src => `![](${src})`)
    .join('\n');
  return {
    type: 'bluesky',
    title: `${post.author?.displayName || post.author?.handle || handle}: ${post.record.text.slice(0, 60)}`,
    content: `${post.record.text}\n\n${imageMarkdown}\n`,
    description: `${post.likeCount ?? 0} likes`,
    author: post.author?.displayName || post.author?.handle,
    published: post.record.createdAt,
    url,
    extra: { likes: post.likeCount ?? 0 },
  };
}

/** Mastodon status, via the instance's public API. */
export async function parseMastodon(url: string, ctx: ParserContext): Promise<ParsedContent | null> {
  const match = url.match(/^(https?:\/\/[^/]+)\/@([^/]+)\/(\d+)/i);
  if (!match) return null;
  const [, instance, , id] = match;
  const api = `${instance}/api/v1/statuses/${id}`;
  const response = await ctx.proxy(api);
  if (response.status >= 400) return null;
  const status = JSON.parse(response.body) as {
    content?: string;
    account?: { display_name?: string; acct?: string };
    created_at?: string;
    favourites_count?: number;
    media_attachments?: Array<{ url?: string }>;
  };
  if (!status.content) return null;
  const media = (status.media_attachments ?? [])
    .map(attachment => attachment.url)
    .filter((src): src is string => Boolean(src))
    .map(src => `![](${src})`)
    .join('\n');
  return {
    type: 'mastodon',
    title: `${status.account?.display_name || status.account?.acct || 'Mastodon'}: ${decodeEntities(status.content).slice(0, 60)}`,
    content: `${status.content}\n\n${media}\n`,
    description: `${status.favourites_count ?? 0} favourites`,
    author: status.account?.display_name || status.account?.acct,
    published: status.created_at,
    url,
    extra: { favourites: status.favourites_count ?? 0 },
  };
}

/** GitHub repo or issue, via the public API (rate-limited without a token). */
export async function parseGithub(url: string, ctx: ParserContext): Promise<ParsedContent | null> {
  const match = url.match(/github\.com\/([^/]+)\/([^/]+)(?:\/(issues|pull)\/(\d+))?/i);
  if (!match) return null;
  const [, owner, repo, kind, number] = match;
  const api = kind && number
    ? `https://api.github.com/repos/${owner}/${repo}/issues/${number}`
    : `https://api.github.com/repos/${owner}/${repo}`;
  const response = await ctx.proxy(api, { headers: { accept: 'application/vnd.github+json', 'user-agent': 'pergame-clipper' } });
  if (response.status >= 400) return null;
  const data = JSON.parse(response.body) as Record<string, unknown>;
  if (kind && number) {
    return {
      type: 'github',
      title: `${owner}/${repo}#${number} ${String(data.title ?? '')}`.trim(),
      content: `${String(data.body ?? '')}\n`,
      description: `${String(data.state ?? '')} · ${String(data.comments ?? 0)} comments`,
      author: (data.user as { login?: string } | undefined)?.login,
      published: String(data.created_at ?? ''),
      url,
      tags: Array.isArray(data.labels)
        ? (data.labels as Array<{ name?: string }>).map(label => label.name ?? '').filter(Boolean)
        : [],
      extra: { state: data.state ?? '', comments: data.comments ?? 0 },
    };
  }
  return {
    type: 'github',
    title: `${owner}/${repo}`,
    content: `${String(data.description ?? '')}\n\n${String(data.html_url ?? url)}\n`,
    description: `${String(data.language ?? '')} · ★ ${String(data.stargazers_count ?? 0)}`,
    author: owner,
    url,
    tags: Array.isArray(data.topics) ? (data.topics as string[]) : [],
    extra: { stars: data.stargazers_count ?? 0, forks: data.forks_count ?? 0 },
  };
}

/** Wikipedia article summary + link (full wikitext is out of scope). */
export async function parseWikipedia(url: string, ctx: ParserContext): Promise<ParsedContent | null> {
  const match = url.match(/^https?:\/\/([a-z-]+)\.wikipedia\.org\/wiki\/(.+)$/i);
  if (!match) return null;
  const [, lang, title] = match;
  const api = `https://${lang}.wikipedia.org/api/rest_v1/page/summary/${encodeURIComponent(decodeURIComponent(title!))}`;
  const response = await ctx.proxy(api);
  if (response.status >= 400) return null;
  const data = JSON.parse(response.body) as { title?: string; extract?: string; description?: string };
  if (!data.extract) return null;
  return {
    type: 'wikipedia',
    title: data.title ?? url,
    content: `${data.extract}\n\n[Read on Wikipedia](${url})\n`,
    description: data.description ?? '',
    url,
    extra: {},
  };
}
