/**
 * Web clipper — clip pages, posts and videos into Pergame notes.
 *
 * Replaces the SiYuan ReadItLater plugin's job (this plugin started as
 * `web-clipper`): clipboard/batch clipping, per-type parsing, templates and
 * images stored as attachments. Browser automation is not involved: pages are
 * fetched server-side through `ctx.net.proxy`, and anything that still needs a
 * login is handled with the user's own cookie or a paste.
 */
import { clip, clipBatch, DEFAULT_TEMPLATES, SETTINGS, type ClipHost } from './service';
import type { ParsedContent, ParserContext } from './types';

/** Folder choices for the save dialog. */
interface FolderOption {
  value: string;
  label: string;
}

export default class WebClipper {
  private host!: ClipHost;
  private ctxRef!: unknown;

  async onload(ctx: any) {
    // The dialog must open immediately: listing every node can be slow on a
    // large library, so the picker gives up after a moment and falls back to
    // "top level" (the folder can still be set in plugin settings).
    const folders = async (): Promise<FolderOption[]> => {
      const top = [{ value: '', label: 'Top level' }];
      const listing = ctx.nodes
        .listChildren()
        .then((nodes: any[]) =>
          nodes.filter(node => node.role === 1 || node.role === 2).map(node => ({ value: node.id, label: node.name })),
        )
        .catch(() => []);
      const containers = await Promise.race([
        listing,
        new Promise<FolderOption[]>(resolve => setTimeout(() => resolve([]), 1500)),
      ]);
      return [...top, ...containers];
    };

    // Adapt the host context to the parser surface: parsers stay independent
    // of the plugin API shape, and the mapping is the only place that knows
    // where attachments and settings live.
    const parserCtx: ParserContext = {
      proxy: (url, init) => ctx.net.proxy(url, init),
      proxyBinary: url => ctx.net.proxyBinary(url),
      upload: async (bytes, filename, contentType) => {
        const blob = new Blob([bytes], { type: contentType });
        const stored = await ctx.resources.upload(new File([blob], filename, { type: contentType }));
        return `/api/resources/${stored.id}/file`;
      },
      setting: key => ctx.settings.get(key),
    };
    const host: ClipHost = {
      ctx: parserCtx,
      createNote: input => ctx.nodes.create({ ...input, role: 3 }),
      notify: (message, opts) => ctx.ui.notice(message, opts),
    };
    this.host = host;
    this.ctxRef = ctx;

    const askAndClip = async (initialUrl: string) => {
      const saved = await ctx.settings.get<string>(SETTINGS.parentId);
      const result = await ctx.ui.modal({
        title: 'Clip to note',
        description: 'Paste a URL, or any text to save it as a note. Leave the title empty to use the page title.',
        fields: [
          { key: 'url', label: 'URL or text', type: 'textarea', value: initialUrl, required: true },
          { key: 'title', label: 'Title (optional)', type: 'text', placeholder: 'Use the detected title' },
          { key: 'tags', label: 'Tags (optional)', type: 'text', placeholder: 'comma, separated' },
          { key: 'parent', label: 'Folder', type: 'select', value: saved ?? '', choices: await folders() },
        ],
        submitLabel: 'Clip',
      });
      if (!result) return;
      try {
        await clip(host, String(result.url ?? ''), {
          parentId: String(result.parent ?? '') || undefined,
          tags: String(result.tags ?? ''),
          title: String(result.title ?? '') || undefined,
        });
      } catch (err) {
        ctx.ui.notice(`Clip failed: ${err instanceof Error ? err.message : String(err)}`, { type: 'error', timeout: 8000 });
      }
    };

    ctx.ui.command('clip', 'Web clipper: clip a URL', async () => {
      const clipboard = await navigator.clipboard.readText().catch(() => '');
      await askAndClip(clipboard.trim());
    });

    ctx.ui.command('clip-clipboard', 'Web clipper: clip from clipboard', async () => {
      const text = await navigator.clipboard.readText().catch(() => '');
      if (!text.trim()) {
        ctx.ui.notice('Clipboard is empty', { type: 'warning' });
        return;
      }
      try {
        await clip(host, text);
      } catch (err) {
        ctx.ui.notice(`Clip failed: ${err instanceof Error ? err.message : String(err)}`, { type: 'error', timeout: 8000 });
      }
    });

    ctx.ui.command('clip-batch', 'Web clipper: clip every URL in the clipboard', async () => {
      const text = await navigator.clipboard.readText().catch(() => '');
      await clipBatch(host, text);
    });

    ctx.ui.ribbon('🗞️', 'Web clipper', async () => {
      const text = await navigator.clipboard.readText().catch(() => '');
      if (text.trim()) await askAndClip(text.trim());
      else await askAndClip('');
    });

    // Expose the template defaults so a user can copy them into settings.
    await ctx.settings.set('template_defaults', DEFAULT_TEMPLATES);
  }

  async onunload() {
    void this.ctxRef;
  }
}
