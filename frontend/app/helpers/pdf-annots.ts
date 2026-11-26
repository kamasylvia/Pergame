/**
 * SiYuan-style annotation overlay on top of the stock pdf.js viewer
 * (vendored at /pdfjs, loaded in a same-origin iframe).
 *
 * The stock annotation editor is disabled (mode DISABLE + buttons hidden);
 * Pergame annotations live in our DB and render as highlight divs inside
 * each `.page` element. Coordinates are PDF default-user-space points, so
 * zoom/rotation are handled by the viewport conversions.
 */

export interface PdfQuad {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

export interface PdfAnn {
  id: string;
  user_id: string;
  resource_id: string;
  page: number; // 1-based
  quads: PdfQuad[];
  quote: string;
  comment: string;
  color: number;
  created_timestamp: number;
  updated_timestamp: number;
}
export interface PdfSelection {
  page: number; // 1-based
  quads: PdfQuad[];
  quote: string;
}

export interface PdfAnnBridge {
  /** Re-render all overlays (call after CRUD). */
  setAnnotations(anns: PdfAnn[]): void;
  /** Jump to an annotation and flash it. */
  gotoAnnotation(ann: PdfAnn): void;
  /** Current text selection as annotation geometry, if usable. */
  getSelection(): PdfSelection | null;
  clearSelection(): void;
  destroy(): void;
}

interface ViewerWindow extends Window {
  PDFViewerApplication?: {
    pdfViewer?: {
      getPageView(index: number): {
        div: HTMLElement;
        viewport: {
          convertToPdfPoint(x: number, y: number): [number, number];
          convertToViewportPoint(x: number, y: number): [number, number];
        };
      } | undefined;
      pagesCount: number;
      currentPageNumber: number;
      scrollPageIntoView(params: { pageNumber: number }): void;
      eventBus: {
        on(name: string, handler: () => void): void;
        off(name: string, handler: () => void): void;
      };
      annotationEditorMode: unknown;
    };
  };
}

const OVERLAY_CLASS = 'pg-ann-overlay';
const HIGHLIGHT_CLASS = 'pg-ann';
/** Static rules live in an external file: the viewer's CSP (`style-src
 * 'self'`) kills inline <style>. Positioning via CSSOM is CSP-clean. */
const OVERLAY_CSS_HREF = 'pergame-annots.css';

async function waitForViewer(win: ViewerWindow, tries = 100): Promise<NonNullable<ViewerWindow['PDFViewerApplication']>> {
  for (let i = 0; i < tries; i++) {
    const app = win.PDFViewerApplication;
    if (app?.pdfViewer && app.pdfViewer.pagesCount) return app;
    await new Promise<void>(resolve => setTimeout(resolve, 100));
  }
  throw new Error('PDF viewer did not boot');
}

export async function attachPdfAnnotations(
  iframe: HTMLIFrameElement,
  opts: {
    colorOf: (index: number) => string;
    onAnnotationClick: (id: string) => void;
    onSelectionChange: () => void;
  },
): Promise<PdfAnnBridge> {
  const win = iframe.contentWindow as ViewerWindow | null;
  if (!win) throw new Error('PDF iframe has no window');
  const app = await waitForViewer(win);
  const viewer = app.pdfViewer!;
  const doc = win.document;

  // Lock out the stock annotation editor (competing system): hide its
  // toolbar via the external overlay stylesheet, and force the mode back
  // to NONE whenever anything (shortcuts, future UI) enables it.
  const cssLink = doc.createElement('link');
  cssLink.rel = 'stylesheet';
  cssLink.href = OVERLAY_CSS_HREF;
  doc.head.appendChild(cssLink);

  let anns: PdfAnn[] = [];
  let destroyed = false;

  const overlayFor = (pageNumber: number): HTMLElement | null => {
    const page = doc.querySelector<HTMLElement>(`.page[data-page-number="${pageNumber}"]`);
    if (!page) return null;
    let layer = page.querySelector<HTMLElement>(`:scope > .${OVERLAY_CLASS}`);
    if (!layer) {
      layer = doc.createElement('div');
      layer.className = OVERLAY_CLASS;
      page.appendChild(layer);
    }
    return layer;
  };

  const draw = () => {
    if (destroyed) return;
    doc.querySelectorAll(`.${OVERLAY_CLASS}`).forEach(el => el.remove());
    for (const ann of anns) {
      const view = viewer.getPageView(ann.page - 1);
      const layer = overlayFor(ann.page);
      if (!view || !layer) continue;
      for (const q of ann.quads) {
        const [x1, y1] = view.viewport.convertToViewportPoint(q.x0, q.y0);
        const [x2, y2] = view.viewport.convertToViewportPoint(q.x1, q.y1);
        const el = doc.createElement('div');
        el.className = HIGHLIGHT_CLASS;
        el.dataset.annId = ann.id;
        el.style.left = `${Math.min(x1, x2)}px`;
        el.style.top = `${Math.min(y1, y2)}px`;
        el.style.width = `${Math.abs(x2 - x1)}px`;
        el.style.height = `${Math.abs(y2 - y1)}px`;
        el.style.background = opts.colorOf(ann.color);
        el.title = ann.comment || ann.quote;
        el.addEventListener('click', ev => {
          ev.stopPropagation();
          opts.onAnnotationClick(ann.id);
        });
        layer.appendChild(el);
      }
    }
  };

  const redraw = () => draw();
  const bus = viewer.eventBus;
  bus.on('pagerendered', redraw);
  bus.on('scalechanging', redraw);
  bus.on('rotationchanging', redraw);
  // Guard rail: the stock editor must never engage (NONE = 0).
  const forceNone = () => {
    try {
      if (viewer.annotationEditorMode !== 0) viewer.annotationEditorMode = { mode: 0 };
    } catch { /* editor UIManager absent: nothing to guard */ }
  };
  bus.on('annotationeditormodechanged', forceNone);
  forceNone();

  const onUp = () => opts.onSelectionChange();
  doc.addEventListener('mouseup', onUp);

  const getSelection = (): PdfSelection | null => {
    const sel = doc.getSelection();
    if (!sel || sel.isCollapsed || sel.rangeCount === 0) return null;
    const quote = sel.toString().trim().replace(/\s+/g, ' ').slice(0, 2000);
    if (!quote) return null;
    const range = sel.getRangeAt(0);
    const anchor = range.commonAncestorContainer instanceof Element
      ? range.commonAncestorContainer
      : range.commonAncestorContainer.parentElement;
    const pageEl = anchor?.closest?.('.page[data-page-number]') as HTMLElement | null;
    if (!pageEl?.dataset.pageNumber) return null;
    const page = Number(pageEl.dataset.pageNumber);
    if (!Number.isInteger(page) || page < 1) return null;
    // Single-page selections only (v1): every rect must sit on this page.
    const pageBox = pageEl.getBoundingClientRect();
    const view = viewer.getPageView(page - 1);
    if (!view) return null;
    const quads: PdfQuad[] = [];
    const rects = range.getClientRects();
    if (rects.length === 0 || rects.length > 64) return null;
    for (const r of rects) {
      if (r.width < 1 || r.height < 1) continue;
      const [x0, y1] = view.viewport.convertToPdfPoint(r.left - pageBox.left, r.top - pageBox.top);
      const [x1, y0] = view.viewport.convertToPdfPoint(r.right - pageBox.left, r.bottom - pageBox.top);
      if (![x0, y0, x1, y1].every(Number.isFinite)) return null;
      quads.push({ x0: Math.min(x0, x1), y0: Math.min(y0, y1), x1: Math.max(x0, x1), y1: Math.max(y0, y1) });
    }
    if (!quads.length) return null;
    // Reject cross-page drags (rects outside this page's box).
    for (const r of rects) {
      if (r.bottom < pageBox.top || r.top > pageBox.bottom) return null;
    }
    return { page, quads, quote };
  };

  return {
    setAnnotations(next: PdfAnn[]) {
      anns = next;
      draw();
    },
    gotoAnnotation(ann: PdfAnn) {
      viewer.scrollPageIntoView({ pageNumber: ann.page });
      window.setTimeout(() => {
        if (destroyed) return;
        doc.querySelectorAll(`.${HIGHLIGHT_CLASS}[data-ann-id="${ann.id}"]`).forEach(el => {
          el.classList.remove('pg-ann-flash');
          void (el as HTMLElement).offsetWidth;
          el.classList.add('pg-ann-flash');
        });
      }, 350);
    },
    getSelection,
    clearSelection() {
      doc.getSelection()?.removeAllRanges();
    },
    destroy() {
      destroyed = true;
      try {
        bus.off('pagerendered', redraw);
        bus.off('scalechanging', redraw);
        bus.off('rotationchanging', redraw);
        bus.off('annotationeditormodechanged', forceNone);
      } catch { /* viewer already torn down */ }
      doc.removeEventListener('mouseup', onUp);
      doc.querySelectorAll(`.${OVERLAY_CLASS}`).forEach(el => el.remove());
      cssLink.remove();
    },
  };
}
