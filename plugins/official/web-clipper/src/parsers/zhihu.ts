/** Zhihu answers and columns.
 *
 * ReadItLater delegated this to a browser-automation service because the
 * SiYuan sandbox had no way to carry a session. Here the fetch happens
 * server-side with the user's own cookie (stored in plugin settings), which
 * is enough for the HTML variants; when Zhihu still serves a challenge the
 * caller falls back to "paste the article".
 */
import TurndownService from 'turndown';
import { gfm } from 'turndown-plugin-gfm';
import type { ParsedContent, ParserContext } from '../types';

const ZHIHU_COOKIE_KEY = 'zhihu_cookie';

function toMarkdown(html: string): string {
  const service = new TurndownService({ headingStyle: 'atx', codeBlockStyle: 'fenced' });
  service.use(gfm);
  service.remove(['script', 'style', 'noscript']);
  return service.turndown(html);
}

/** Parse a Zhihu answer or column article.
 *
 * Escalation chain (mirrors ReadItLater v0.12): direct fetch with the user's
 * cookie first (fast), then the camoufox browser render for pages that serve
 * a challenge to plain HTTP clients. Both fail → error with guidance.
 */
export async function parseZhihu(url: string, ctx: ParserContext): Promise<ParsedContent | null> {
  const cookie = await ctx.setting<string>(ZHIHU_COOKIE_KEY);
  const selectors = {
    waitSelector: /\/p\/\d+/.test(url) ? '.Post-RichText' : '.RichContent-inner',
    htmlSelector: /\/p\/\d+/.test(url) ? '.Post-RichText' : '.RichContent-inner',
  };

  let html = '';
  if (cookie) {
    const response = await ctx.proxy(url, {
      headers: {
        'user-agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126 Safari/537.36',
        'accept-language': 'zh-CN,zh;q=0.9',
        cookie,
      },
    });
    if (response.status < 400 && /RichText/i.test(response.body)) {
      html = response.body;
    }
  }
  if (!html) {
    // Camoufox render: user-configured self-hosted service, same as the
    // SiYuan ReadItLater path. Errors carry through with guidance.
    const rendered = await ctx.render(url, selectors);
    html = rendered.html;
  }

  const document = new DOMParser().parseFromString(html, 'text/html');
  const isArticle = /\/p\/\d+/.test(url);
  const title = isArticle
    ? document.querySelector('.Post-Title')?.textContent?.trim() ?? document.title
    : document.querySelector('.QuestionHeader-title')?.textContent?.trim() ?? document.title;
  const body = isArticle
    ? document.querySelector('.Post-RichText')?.innerHTML
    : document.querySelector('.RichText')?.innerHTML ?? document.querySelector('.AnswerItem .RichText')?.innerHTML;

  if (!body) {
    throw new Error(
      'Zhihu returned no article body. Configure the camoufox service or a valid cookie in plugin settings.',
    );
  }
  const author = document.querySelector('.AuthorInfo-name')?.textContent?.trim() ?? undefined;
  return {
    type: 'zhihu',
    title: title ?? url,
    content: toMarkdown(body),
    description: isArticle ? 'Zhihu column' : 'Zhihu answer',
    author,
    url,
    tags: ['zhihu'],
    extra: {},
  };
}
