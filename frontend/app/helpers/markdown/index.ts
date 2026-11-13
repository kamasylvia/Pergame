import { usePreferencesStore } from '~/stores/preferences/preferences';
import MarkdownIt, { type MarkdownIt as MarkdownItType } from 'markdown-it';
import { containerPlugin } from './container';
import { parseFrontmatter } from './frontmatter';
import { anchorPlugin } from './anchor';
import { containerSvg } from './containers-svg';
import { copyCodePlugin } from './code-block';
import { advancedBlocks } from './cards';
import { mermaidPlugin } from './mermaid';
import { dataviewPlugin } from './dataview';
import underline from './underline';
import highlight from 'markdown-it-highlightjs';
// @ts-expect-error no types provided
import mark from 'markdown-it-mark';
// @ts-expect-error no types provided
import ins from 'markdown-it-ins';
import multimdTable from 'markdown-it-multimd-table';
// @ts-expect-error no types provided
import deflist from 'markdown-it-deflist';
// @ts-expect-error no types provided
import abbr from 'markdown-it-abbr';
import { abcPlugin } from './abc';
import colorPlugin from './colors';
import { embedsPlugin } from './embeds';
import { subscriptPlugin, superscriptPlugin, footNotePlugin, html5MediaPlugin, svgObjectPlugin } from './other';
import { markdownItCheckbox } from './checkbox';
import { markdownItKatexPlugin } from './katex';
import { sourceMapPlugin } from './source-map';
import { internalLinkPlugin } from './internal-links';
import { tooltipPlugin } from './tooltip';
import { counterPlugin } from './counters';
import { full as emojiPlugin } from 'markdown-it-emoji';
import { markdownHooks, runPostProcessors } from '~/services/plugins/markdown';
import { applyPluginHooks } from '~/services/plugins/markdown-hooks';

function buildCompiler(breaks: boolean, typographer: boolean): MarkdownItType {
  const md = new MarkdownIt({ html: true, linkify: true, breaks, typographer });
  // markdown-it-multimd-table 4.x calls md.utils.assign (removed in
  // markdown-it 13+); Object.assign is the identical semantic.
  (md.utils as unknown as Record<string, unknown>).assign ??= Object.assign;
  md.use(containerPlugin);
  md.use(anchorPlugin);
  md.use(containerSvg);
  md.use(advancedBlocks);
  md.use(mermaidPlugin);
  md.use(dataviewPlugin);
  md.use(abcPlugin);
  md.use(copyCodePlugin);
  md.use(underline);
  md.use(mark);
  md.use(ins);
  md.use(multimdTable);
  md.use(deflist);
  md.use(abbr);
  md.use(markdownItKatexPlugin);
  md.use(highlight);
  md.use(markdownItCheckbox);
  md.use(subscriptPlugin);
  md.use(superscriptPlugin);
  md.use(footNotePlugin);
  md.use(colorPlugin, {
    enableBracketSyntax: true, // also parse [text]{color=value}
    allowHex: true, // allow #rgb and #rrggbb
  });
  md.use(tooltipPlugin);
  md.use(counterPlugin);
  md.use(emojiPlugin);
  md.use(html5MediaPlugin);
  md.use(svgObjectPlugin);
  md.use(internalLinkPlugin);
  md.use(sourceMapPlugin);
  md.use(embedsPlugin);
  // Plugin hooks (PS-C): see services/plugins/markdown-hooks.
  applyPluginHooks(md);
  return md;
}

// Cached compilers per (breaks, typographer) pair; rebuilt when the user
// flips a markdown setting (instances are cheap, plugins are static).
const compilerCache = new Map<string, MarkdownItType>();

function getCompiler(breaks: boolean, typographer: boolean) {
  // Hook registrations bump `markdownHooks.version`: a new plugin renderer
  // takes effect on the next compile without a page reload.
  const key = `${breaks}:${typographer}:${markdownHooks.version}`;
  let md = compilerCache.get(key);
  if (!md) {
    md = buildCompiler(breaks, typographer);
    compilerCache.set(key, md);
  }
  return md;
}

export default function compile(str: string = '', resolveNode?: (id: string) => string | undefined): string {
  // Frontmatter renders as a properties table (ContentCompiled), never inline.
  str = parseFrontmatter(str).content;
  // Replace &lt; &gt; &amp; to < > & (to avoid markdown-it escape)
  str = str.replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&amp;/g, '&');
  let breaks = true;
  let typographer = false;
  try {
    const prefs = usePreferencesStore();
    breaks = prefs.preferences.markdownBreaks ?? true;
    typographer = prefs.preferences.markdownTypographer ?? false;
  } catch {
    // Store unavailable (SSR/tests): fall back to defaults.
  }
  const html = getCompiler(breaks, typographer).render(str, {
    html: true,
    resolveNode,
  });
  return markdownHooks.postProcessors.size ? runPostProcessors(html) : html;
}
