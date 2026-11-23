/**
 * Web clipper — clip pages, posts and videos into Pergame notes.
 *
 * Mirrors the SiYuan ReadItLater plugin's job: per-content-type folder slug,
 * title template and body template, batch clipping, images as attachments.
 * Pages are fetched server-side through `ctx.net.proxy`; anything behind a
 * login uses the user's own cookie or a paste.
 */
import { clip, clipBatch, DEFAULT_TEMPLATES, SETTING_TYPES, SETTINGS, type ClipHost } from './service';
import { splitBatch } from './detect';
import type { ParserContext } from './types';

/** A container choice for the save dialog. */
interface FolderOption {
  value: string;
  label: string;
}

/**
 * UI wording by host locale (`ctx.api.host.locale`). The host translates the
 * progress label itself; these strings cover the plugin's own toasts. Locales
 * outside the map fall back to English.
 */
const TEXTS = {
  'zh-CN': {
    done: '剪藏完成：{title}',
    failed: '剪藏失败：{message}',
    clipFailed: '剪藏失败：{url} — {reason}',
    batchDone: '批量剪藏完成：{ok}/{total}',
    batchItem: '正在剪藏 {i}/{total}：{url}',
    stageFetching: '正在抓取：{url}',
    stageSaving: '正在保存…',
    batchFailedPart: '(失败 {count})',
    clipboardEmpty: '剪贴板中没有链接',
  },
  'zh-TW': {
    done: '剪藏完成：{title}',
    failed: '剪藏失敗：{message}',
    clipFailed: '剪藏失敗：{url} — {reason}',
    batchDone: '批次剪藏完成：{ok}/{total}',
    batchItem: '正在剪藏 {i}/{total}：{url}',
    stageFetching: '正在抓取：{url}',
    stageSaving: '正在儲存…',
    batchFailedPart: '(失敗 {count})',
    clipboardEmpty: '剪貼簿中沒有連結',
  },
  en: {
    done: 'Clipped: {title}',
    failed: 'Clip failed: {message}',
    clipFailed: 'Clip failed: {url} — {reason}',
    batchDone: 'Batch clip finished: {ok}/{total}',
    batchItem: 'Clipping {i}/{total}: {url}',
    stageFetching: 'Fetching: {url}',
    stageSaving: 'Saving…',
    batchFailedPart: ' ({count} failed)',
    clipboardEmpty: 'No link found in the clipboard',
  },
} as const;

type ClipTextKey = keyof typeof TEXTS.en;

/** Build the locale-bound text helper handed to the service layer. */
function makeText(locale: string | undefined) {
  const dict = TEXTS[locale as keyof typeof TEXTS] ?? TEXTS.en;
  return (key: ClipTextKey, vars: Record<string, string | number> = {}) => {
    let out: string = dict[key];
    for (const [k, v] of Object.entries(vars)) out = out.replaceAll(`{${k}}`, String(v));
    return out;
  };
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
    /** Progress sink of the clip in flight; parsers call ctx.render blind. */
    let activeProgress: ((p: { percent: number; stage: string; label?: string }) => void) | undefined;
    const parserCtx: ParserContext = {
      proxy: (url, init) => ctx.net.proxy(url, init),
      proxyBinary: url => ctx.net.proxyBinary(url),
      upload: async (bytes, filename, contentType) => {
        const blob = new Blob([bytes], { type: contentType });
        const stored = await ctx.resources.upload(new File([blob], filename, { type: contentType }));
        return `/api/resources/${stored.id}/file`;
      },
      setting: key => ctx.settings.get(key),
      // Closure-inject the clip's progress sink: parsers call render
      // blind, and the sink is whatever clip is currently in flight.
      render: (url, opts) => ctx.net.render(url, opts, p => activeProgress?.(p)),
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

    const text = makeText(ctx.api?.host?.locale);

    const host: ClipHost = {
      ctx: parserCtx,
      createNote: input => ctx.nodes.create({ ...input, role: 3 }),
      ensureFolder,
      notify: (message, opts) => ctx.ui.notice(message, opts),
      text: (key, vars) => text(key as ClipTextKey, vars),
    };
    this.host = host;
    this.ctxRef = ctx;


    /**
     * One toast updated in place: percent rolls, then flips to the final
     * state. Without opts.id each stage would stack its own toast.
     * The host renders the stage label in the UI language (older hosts
     * without `label` fall back to the raw stage key). Also arms the
     * parser render sink so browser-render stages join the same toast.
     */
    const trackProgress = (toastId: string) => {
      let lastLabel = '';
      const onProgress = (p: { percent: number; stage: string; label?: string }) => {
        const label = p.label ?? p.stage;
        if (label !== lastLabel) {
          lastLabel = label;
          ctx.ui.notice(label, { timeout: 300000, id: toastId });
        }
      };
      activeProgress = onProgress;
      return onProgress;
    };

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
          'Paste a URL — or multiple URLs (newline/comma separated) to clip them in batch. The folder comes from the content-type and inbox path templates in settings.',
        fields: [
          { key: 'url', label: 'URL or text', type: 'textarea', value: initialUrl, required: true },
          { key: 'title', label: 'Title (optional)', type: 'text', placeholder: 'Use the title template' },
          { key: 'tags', label: 'Tags (optional)', type: 'text', placeholder: 'Defaults to the content-type slug' },
          { key: 'workspace', label: 'Workspace', type: 'select', value: saved ?? '', choices: [] },
        ],
        submitLabel: 'Clip',
      });
      if (!result) return;
      // Multiple URLs in the field -> batch flow (mirrors the clipboard batch
      // command); a single URL keeps the single-clip path with its templates.
      const batchUrls = splitBatch(
        String(result.url ?? ''),
        String((await ctx.settings.get<string>(SETTINGS.batchDelimiter)) || 'auto'),
      );
      // Rendering runs a real browser (tens of seconds); say so up front or
      // the dialog closure reads as a silent hang.
      const toastId = 'clip-progress';
      const onProgress = trackProgress(toastId);
      if (batchUrls.length > 1) {
        try {
          const { ok, failed } = await clipBatch(host, String(result.url ?? ''), {
            workspaceId: String(result.workspace ?? '') || undefined,
            tags: String(result.tags ?? '') || undefined,
            onProgress,
            announceSummary: false,
          });
          activeProgress = undefined;
          const doneSummary =
            text('batchDone', { ok, total: batchUrls.length }) + (failed ? text('batchFailedPart', { count: failed }) : '');
          ctx.ui.notice(doneSummary, {
            type: failed ? 'warning' : 'success',
            timeout: 6000,
            id: toastId,
          });
        } catch (err) {
          activeProgress = undefined;
          const message = err instanceof Error ? err.message : String(err);
          ctx.ui.notice(text('failed', { message }), { type: 'error', timeout: 8000, id: toastId });
        }
        return;
      }
      try {
        const outcome = await clip(host, String(result.url ?? ''), {
          workspaceId: String(result.workspace ?? '') || undefined,
          tags: String(result.tags ?? ''),
          title: String(result.title ?? '') || undefined,
          onProgress,
        });
        activeProgress = undefined;
        ctx.ui.notice(text('done', { title: outcome.title }), { type: 'success', timeout: 4000, id: toastId });
      } catch (err) {
        activeProgress = undefined;
        const message = err instanceof Error ? err.message : String(err);
        ctx.ui.notice(text('failed', { message }), { type: 'error', timeout: 8000, id: toastId });
      }
    };


    ctx.ui.command('clip', 'Web clipper: clip a URL', async () => {
      // No clipboard read here: the browser asks for permission first and the
      // dialog would arrive late behind that prompt. The dialog has the field.
      await askAndClip('');
    });

    ctx.ui.command('clip-from-clipboard', 'Web clipper: clip from clipboard', async () => {
      const clipboard = await navigator.clipboard.readText().catch(() => '');
      if (!clipboard.trim()) {
        ctx.ui.notice(text('clipboardEmpty'), { type: 'warning' });
        return;
      }
      await askAndClip(clipboard.trim());
    });

    ctx.ui.command('clip-batch', 'Web clipper: clip every URL in the clipboard', async () => {
      const clipboard = await navigator.clipboard.readText().catch(() => '');
      const toastId = 'clip-batch-progress';
      const onProgress = trackProgress(toastId);
      try {
        const { ok, failed } = await clipBatch(host, clipboard, { onProgress, announceSummary: false });
        const total = ok + failed;
        if (!total) return; // clipBatch already warned about the empty clipboard
        const summary = text('batchDone', { ok, total }) + (failed ? text('batchFailedPart', { count: failed }) : '');
        ctx.ui.notice(summary, { type: failed ? 'warning' : 'success', timeout: 6000, id: toastId });
      } finally {
        activeProgress = undefined;
      }
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
