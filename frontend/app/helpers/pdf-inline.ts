/**
 * Inline PDF rendering inside compiled notes (Obsidian mode):
 * - `![]()` embeds (`.pdf-embed` placeholders) always mount the stock
 *   viewer in an iframe, both modes.
 * - Plain PDF links mount inline viewers only when the note's effective
 *   mode is `inline`; in `side` mode they stay links (click opens panel).
 * Overlays are view-only here (hover + click-through to the side panel);
 * annotation creation lives in the side panel.
 */
import { attachPdfAnnotations, type PdfAnn, type PdfAnnBridge } from './pdf-annots';
import { pdfViewerUrl, type PdfPreviewMode } from './pdf-mode';

export interface PdfPanelPayload {
  resourceId: string;
  fileUrl: string;
  title: string;
  page?: number;
  annotationId?: string | null;
}

export interface InlinePdfCtx {
  mode: () => PdfPreviewMode;
  openPanel: (p: PdfPanelPayload) => void;
  listAnnotations: (resourceId: string) => Promise<PdfAnn[]>;
  colorOf: (index: number) => string;
  /** Sync store lookup for legacy bare /file URLs (miss = not a PDF). */
  isPdfResource: (id: string) => boolean;
  /** Filename for building named URLs (fallback 'file.pdf'). */
  fileNameOf: (id: string) => string;
}

const PDF_PATH_RE = /^\/api\/resources\/([0-9a-fA-F-]{36})\/file(\/.*)?$/;

interface PdfHref {
  resourceId: string;
  fileUrl: string;
  named: boolean;
}

function parsePdfHref(href: string, ctx: InlinePdfCtx): PdfHref | null {
  let url: URL;
  try {
    url = new URL(href, window.location.origin);
  } catch {
    return null;
  }
  if (url.origin !== window.location.origin) return null;
  const m = PDF_PATH_RE.exec(url.pathname);
  const resourceId = m?.[1];
  if (!m || !resourceId) return null;
  if (/\.pdf([?#].*)?$/i.test(href)) return { resourceId, fileUrl: href, named: true };
  if (m[2]) return null; // named non-PDF
  if (!ctx.isPdfResource(resourceId)) return null;
  return { resourceId, fileUrl: `/api/resources/${resourceId}/file/${encodeURIComponent(ctx.fileNameOf(resourceId))}`, named: false };
}

const FRAME_HEIGHT = '620px';

function inlineFrame(fileUrl: string, title: string): HTMLIFrameElement {
  const frame = document.createElement('iframe');
  frame.src = pdfViewerUrl(fileUrl);
  frame.title = title;
  frame.className = 'pdf-inline-frame';
  frame.style.width = '100%';
  frame.style.height = FRAME_HEIGHT;
  frame.style.border = '1px solid var(--border-color)';
  frame.style.borderRadius = '8px';
  frame.style.background = '#fff';
  return frame;
}

export function createInlinePdf(root: HTMLElement, ctx: InlinePdfCtx) {
  const bridges: PdfAnnBridge[] = [];
  const embedFrames: HTMLIFrameElement[] = [];
  let linkCleanups: (() => void)[] = [];
  let linkedMode: PdfPreviewMode | null = null;
  let dead = false;

  const rowsByBridge = new Map<PdfAnnBridge, PdfAnn[]>();

  const attachView = async (frame: HTMLIFrameElement, resourceId: string) => {
    let bridge: PdfAnnBridge;
    try {
      bridge = await attachPdfAnnotations(frame, {
        colorOf: ctx.colorOf,
        onAnnotationClick: id => {
          const ann = rowsByBridge.get(bridge)?.find(a => a.id === id);
          ctx.openPanel({
            resourceId,
            fileUrl: frame.dataset.fileUrl ?? '',
            title: frame.title,
            page: ann?.page,
            annotationId: id,
          });
        },
        onSelectionChange: () => {},
      });
    } catch {
      // Viewer failed to boot (offline file etc.): the iframe shows its
      // own error state; overlays are a bonus.
      return;
    }
    if (dead) {
      bridge.destroy();
      return;
    }
    bridges.push(bridge);
    try {
      const rows = await ctx.listAnnotations(resourceId);
      rowsByBridge.set(bridge, rows);
      bridge.setAnnotations(rows);
    } catch {
      // Annotations are a bonus on top of the stock viewer.
    }
  };

  /** `![]()` embeds -> viewer iframes (both modes; idempotent). */
  const mountEmbeds = () => {
    if (dead) return;
    const holders = root.querySelectorAll<HTMLElement>('.pdf-embed[data-src]:not([data-pg-mounted])');
    for (const holder of holders) {
      const src = holder.dataset.src ?? '';
      const parsed = parsePdfHref(src, ctx);
      if (!parsed) continue;
      holder.dataset.pgMounted = '1';
      holder.textContent = '';
      const frame = inlineFrame(parsed.fileUrl, holder.dataset.alt || 'PDF');
      frame.dataset.fileUrl = parsed.fileUrl;
      holder.appendChild(frame);
      embedFrames.push(frame);
      void attachView(frame, parsed.resourceId);
    }
  };

  const inlineLinks = () => {
    const anchors = root.querySelectorAll<HTMLAnchorElement>('a[href]');
    for (const a of anchors) {
      if (a.closest('.pdf-inline-block')) continue;
      const parsed = parsePdfHref(a.getAttribute('href') ?? '', ctx);
      if (!parsed) continue;
      const marker = document.createComment('pdf-inline');
      const block = document.createElement('div');
      block.className = 'pdf-inline-block';
      const frame = inlineFrame(parsed.fileUrl, a.textContent?.trim() || 'PDF');
      frame.dataset.fileUrl = parsed.fileUrl;
      block.appendChild(frame);
      a.replaceWith(marker);
      marker.after(block);
      void attachView(frame, parsed.resourceId);
      const cleanup = () => {
        block.remove();
        marker.replaceWith(a);
      };
      linkCleanups.push(cleanup);
    }
  };

  const restoreLinks = () => {
    const fns = linkCleanups;
    linkCleanups = [];
    for (const fn of fns) {
      try {
        fn();
      } catch { /* DOM already replaced (html re-render) */ }
    }
  };

  /** Reconcile plain links to the current mode (call on mode change). */
  const ensureLinkMode = () => {
    if (dead) return;
    const mode = ctx.mode();
    if (mode === linkedMode) return;
    restoreLinks();
    linkedMode = mode;
    if (mode === 'inline') inlineLinks();
  };

  const destroy = () => {
    dead = true;
    restoreLinks();
    for (const b of bridges.splice(0)) {
      try {
        b.destroy();
      } catch { /* iframe already gone */ }
    }
    for (const f of embedFrames.splice(0)) f.remove();
    root.querySelectorAll('.pdf-embed[data-pg-mounted]').forEach(el => {
      el.removeAttribute('data-pg-mounted');
    });
  };

  return { mountEmbeds, ensureLinkMode, destroy };
}
