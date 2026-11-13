/**
 * Browser-local attachment cache helpers (client side of the `pergame-
 * attachments` Cache Storage bucket managed by app/sw.ts).
 */

/** Tell the worker to drop every cached range of one resource URL. */
export function invalidateAttachmentCache(url: string): void {
  if (!('serviceWorker' in navigator)) return;
  navigator.serviceWorker.controller?.postMessage({
    type: 'ATTACHMENT_CACHE_INVALIDATE',
    url,
  });
}

/** Absolute /api URL for a resource id (matches the SW route matcher). */
export function resourceFileUrl(id: string): string {
  return `${location.origin}/api/resources/${id}/file`;
}
