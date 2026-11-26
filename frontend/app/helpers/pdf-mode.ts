import type { Node } from '~/stores';

export type PdfPreviewMode = 'side' | 'inline';

/** Effective PDF mode for a note: per-note metadata wins, else the default. */
export function resolvePdfMode(node: Partial<Node> | undefined, fallback: PdfPreviewMode): PdfPreviewMode {
  const override = node?.metadata?.pdfPreview;
  return override === 'side' || override === 'inline' ? override : fallback;
}

/** Stock viewer URL for a resource file (same-origin, cookie-authed). */
export function pdfViewerUrl(fileUrl: string, page?: number): string {
  const base = `/pdfjs/web/viewer.html?file=${encodeURIComponent(fileUrl)}`;
  return page && page > 1 ? `${base}#page=${page}` : base;
}

/** Annotation palette (8 colors, index = pdf_annotations.color). */
export const PDF_ANNOT_COLORS = [
  '#ffd400', // yellow
  '#a4e388', // green
  '#8fd3ff', // blue
  '#ffb3c7', // pink
  '#ff9d5c', // orange
  '#d3a6ff', // purple
  '#8c8c8c', // gray
  '#ff6b6b', // red
] as const;
