import { createApp, type App } from 'vue';
import PDFViewer from '~/components/PDFViewer.vue';

// Mounted apps per placeholder, so re-renders unmount before re-mounting
// (no leaked canvases / pdf.js workers on content updates).
const mounted = new WeakMap<HTMLElement, App>();

/**
 * Mounts PDFViewer into every `.pdf-embed[data-src]` placeholder under root
 * (same post-mount pattern as mermaid diagrams: markdown-it emits the
 * placeholder, the compiled view hydrates it after render).
 */
export function renderPdfEmbeds(root: HTMLElement): void {
  const placeholders = Array.from(root.querySelectorAll<HTMLElement>('.pdf-embed[data-src]'));
  for (const el of placeholders) {
    const prev = mounted.get(el);
    if (prev) {
      prev.unmount();
      mounted.delete(el);
    }
    const src = el.dataset.src ?? '';
    if (!src) continue;
    el.textContent = '';
    const app = createApp(PDFViewer, { src });
    app.mount(el);
    mounted.set(el, app);
  }
}

/** Unmounts everything this module mounted (on component teardown). */
export function unmountPdfEmbeds(root: HTMLElement): void {
  const placeholders = Array.from(root.querySelectorAll<HTMLElement>('.pdf-embed[data-src]'));
  for (const el of placeholders) {
    mounted.get(el)?.unmount();
    mounted.delete(el);
  }
}
