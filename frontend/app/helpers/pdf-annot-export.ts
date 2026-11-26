/**
 * "Download with annotations": fetch the clean stored bytes plus our DB
 * rows, burn each annotation in as a standard /Highlight markup annot,
 * and download the copy. The server file is never modified.
 */
import { PDFDocument, PDFHexString, PDFName, PDFNumber } from 'pdf-lib';

import { PDF_ANNOT_COLORS } from './pdf-mode';
import type { PdfAnn } from './pdf-annots';

function hexToRgb(hex: string): [number, number, number] {
  const n = Number.parseInt(hex.replace('#', ''), 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

export async function downloadPdfWithAnnotations(
  fileUrl: string,
  filename: string,
  anns: PdfAnn[],
): Promise<void> {
  const resp = await fetch(fileUrl, { credentials: 'include' });
  if (!resp.ok) throw new Error(`PDF fetch failed: HTTP ${resp.status}`);
  const pdf = await PDFDocument.load(await resp.arrayBuffer(), { ignoreEncryption: true });
  for (const ann of anns) {
    if (ann.page < 1 || ann.page > pdf.getPageCount() || !ann.quads.length) continue;
    const page = pdf.getPage(ann.page - 1);
    const xs = ann.quads.flatMap(q => [q.x0, q.x1]);
    const ys = ann.quads.flatMap(q => [q.y0, q.y1]);
    // Counterclockwise from bottom-left per quad (PDF y-up points).
    const quadPoints = ann.quads.flatMap(q => [q.x0, q.y0, q.x1, q.y0, q.x1, q.y1, q.x0, q.y1]);
    const [r, g, b] = hexToRgb(PDF_ANNOT_COLORS[ann.color] ?? PDF_ANNOT_COLORS[0]!);
    page.node.addAnnot(
      pdf.context.register(
        pdf.context.obj({
          Type: PDFName.of('Annot'),
          Subtype: PDFName.of('Highlight'),
          Rect: pdf.context.obj([Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)].map(PDFNumber.of)),
          QuadPoints: pdf.context.obj(quadPoints.map(PDFNumber.of)),
          Contents: PDFHexString.fromText(ann.comment || ann.quote),
          T: PDFHexString.fromText('Pergame'),
          C: pdf.context.obj([r, g, b].map(PDFNumber.of)),
          F: PDFNumber.of(4),
        }),
      ),
    );
  }
  const out = await pdf.save();
  const copy = new Uint8Array(out.byteLength);
  copy.set(out);
  const blob = new Blob([copy.buffer], { type: 'application/pdf' });
  const base = filename.replace(/\.pdf$/i, '') || 'document';
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = `${base}-annotated.pdf`;
  document.body.appendChild(a);
  a.click();
  window.setTimeout(() => {
    URL.revokeObjectURL(a.href);
    a.remove();
  }, 5000);
}
