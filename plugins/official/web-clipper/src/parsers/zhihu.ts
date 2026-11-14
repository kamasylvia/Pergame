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

/** Parse a Zhihu answer or column article; null when the page is a challenge. */
export async function parseZhihu(url: string, ctx: ParserContext): Promise<ParsedContent | null> {
  const cookie = await ctx.setting<string>(ZHIHU_COOKIE_KEY);
  const response = await ctx.proxy(url, {
    headers: {
      'user-agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126 Safari/537.36',
      'accept-language': 'zh-CN,zh;q=0.9',
      ...(cookie ? { cookie } : {}),
    },
  });
  if (response.status >= 400) {
    throw new Error(`Zhihu answered ${response.status}${cookie ? '' : ' (no cookie configured)'}`);
  }
  const document = new DOMParser().parseFromString(response.body, 'text/html');

  const isArticle = /\/p\/\d+/.test(url);
  const title = isArticle
    ? document.querySelector('.Post-Title')?.textContent?.trim() ?? document.title
    : document.querySelector('.QuestionHeader-title')?.textContent?.trim() ?? document.title;
  const body = isArticle
    ? document.querySelector('.Post-RichText')?.innerHTML
    : document.querySelector('.RichText')?.innerHTML ?? document.querySelector('.AnswerItem .RichText')?.innerHTML;

  if (!body) {
    // Login wall / JS challenge: let the caller offer the paste path.
    throw new Error('Zhihu returned a challenge page; paste the article instead or add a cookie in plugin settings');
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
