/**
 * Web clipper — clip pages, posts and videos into Pergame notes.
 *
 * Mirrors the SiYuan ReadItLater plugin's job: per-content-type folder slug,
 * title template and body template, batch clipping, images as attachments.
 * Pages are fetched server-side through `ctx.net.proxy`; anything behind a
 * login uses the user's own cookie or a paste.
 */
import { clip, clipBatch, DEFAULT_TEMPLATES, SETTING_TYPES, SETTINGS, type ClipHost } from './service';
import type { ParserContext } from './types';

/** A container choice for the save dialog. */
interface FolderOption {
  value: string;
  label: string;
}

export default class WebClipper {
  private host!: ClipHost;
  private ctxRef!: unknown;

  async onload(ctx: any) {
    // Containers only: the full node listing walks the whole tree (seconds on
    // a large library), while this is one flat index scan.
    const loadWorkspaces = async (): Promise<FolderOption[]> => {
      const nodes: any[] = await ctx.nodes.listContainers();
      return nodes.filter(node => node.role === 1).map(node => ({ value: node.id, label: node.name }));
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
      render: (url, opts) => ctx.net.render(url, opts),
    };

    /**
     * Ensure `workspace/<segment>/<segment>/…` exists and return the leaf id.
     *
     * ReadItLater renders its `inboxDir` template inside one notebook; the
     * equivalent here is a folder path inside one workspace, so the template
     * can route thirteen content types into thirteen folders without extra
     * settings. Existing folders are matched by name at each level.
     */
    const ensureFolder = async (workspaceId: string | undefined, path: string[]): Promise<string | undefined> => {
      if (!workspaceId) return undefined;
      let parentId = workspaceId;
      for (const segment of path) {
        const children: any[] = (await ctx.nodes.listChildren(parentId)) as any[];
        const existing = children.find(node => node.role === 2 && node.name === segment.slice(0, 50));
        parentId = existing?.id ?? (await ctx.nodes.create({ name: segment.slice(0, 50), role: 2, parent_id: parentId })).id;
      }
      return parentId;
    };

    const host: ClipHost = {
      ctx: parserCtx,
      createNote: input => ctx.nodes.create({ ...input, role: 3 }),
      ensureFolder,
      notify: (message, opts) => ctx.ui.notice(message, opts),
    };
    this.host = host;
    this.ctxRef = ctx;


    const askAndClip = async (initialUrl: string) => {
      const saved = await ctx.settings.get<string>(SETTINGS.workspaceId);
      // Kick the listing off first, then open the dialog without awaiting it:
      // on a large library the listing takes seconds and the dialog must not
      // wait for it.
      void loadWorkspaces()
        .then(choices => ctx.ui.patchModalChoices('workspace', choices))
        .catch(() => undefined);
      const result = await ctx.ui.modal({
        title: 'Clip to note',
        description:
          'Paste a URL, or any text to save it as a note. The folder comes from the content-type and inbox path templates in settings.',
        fields: [
          { key: 'url', label: 'URL or text', type: 'textarea', value: initialUrl, required: true },
          { key: 'title', label: 'Title (optional)', type: 'text', placeholder: 'Use the title template' },
          { key: 'tags', label: 'Tags (optional)', type: 'text', placeholder: 'Defaults to the content-type slug' },
          { key: 'workspace', label: 'Workspace', type: 'select', value: saved ?? '', choices: [] },
        ],
        submitLabel: 'Clip',
      });
      if (!result) return;
      try {
        await clip(host, String(result.url ?? ''), {
          workspaceId: String(result.workspace ?? '') || undefined,
          tags: String(result.tags ?? ''),
          title: String(result.title ?? '') || undefined,
        });
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        ctx.ui.notice(`Clip failed: ${message}`, { type: 'error', timeout: 8000 });
      }
    };


    ctx.ui.command('clip', 'Web clipper: clip a URL', async () => {
      // No clipboard read here: the browser asks for permission first and the
      // dialog would arrive late behind that prompt. The dialog has the field.
      await askAndClip('');
    });

    ctx.ui.command('clip-from-clipboard', 'Web clipper: clip from clipboard', async () => {
      const text = await navigator.clipboard.readText().catch(() => '');
      if (!text.trim()) {
        ctx.ui.notice('Clipboard is empty', { type: 'warning' });
        return;
      }
      await askAndClip(text.trim());
    });

    ctx.ui.command('clip-batch', 'Web clipper: clip every URL in the clipboard', async () => {
      const text = await navigator.clipboard.readText().catch(() => '');
      await clipBatch(host, text);
    });

    ctx.ui.ribbon('clip', 'Web clipper', async () => {
      // Same reasoning as the command: asking for clipboard permission up
      // front makes the dialog look broken, so open it empty and let the user
      // paste into the field (browsers allow paste there without a prompt).
      await askAndClip('');
    });

    // Seed the template defaults so a user can copy them into the settings
    // form, and keep the body templates in sync for types that ship defaults.
    await ctx.settings.set('template_defaults', DEFAULT_TEMPLATES);
    void SETTING_TYPES;
  }

  async onunload() {
    void this.ctxRef;
  }
}
