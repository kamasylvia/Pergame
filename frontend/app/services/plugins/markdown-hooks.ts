/**
 * Wires the plugin registries into one markdown-it instance (PS-C).
 *
 * Kept separate from the app compiler so it can be exercised without Nuxt:
 * the same function builds the real pipeline and a test instance, so a
 * regression here shows up in both.
 */
import type { MarkdownIt as MarkdownItType } from 'markdown-it';
import { markdownHooks } from './markdown';
import { pluginContainers } from './markdown-containers';

/** Apply plugin fence/container renderers and register the container rule. */
export function applyPluginHooks(md: MarkdownItType) {
  md.use(pluginContainers);

  const baseFence = md.renderer.rules.fence;
  const escapeHtml = md.utils.escapeHtml as (text: string) => string;
  md.renderer.rules.fence = (tokens, idx, options, env, self) => {
    const token = tokens[idx]!;
    const rawInfo = (token.info || '').trim();
    const lang = rawInfo.split(/\s+/)[0]?.toLowerCase() ?? '';
    const render = lang ? markdownHooks.codeBlocks.get(lang) : undefined;
    if (render) {
      try {
        // `info` is what follows the language (` ```chart title=1 ` → `title=1`).
        const html = render({ content: token.content, info: rawInfo.slice(lang.length).trim(), escapeHtml });
        if (html != null) return html;
      } catch (err) {
        console.warn(`[plugins] code block '${lang}' threw`, err);
      }
    }
    if (baseFence) return baseFence(tokens, idx, options, env, self);
    return self.renderToken(tokens, idx, options);
  };
}
