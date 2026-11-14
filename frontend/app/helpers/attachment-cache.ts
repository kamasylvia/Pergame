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

/**
 * Drop the whole attachment cache (logout, account deletion, or the settings
 * button). Fire-and-forget: the worker owns the storage, and a failure here
 * must never block the caller.
 */
export function clearAttachmentCache(): void {
  if (typeof navigator === 'undefined' || !('serviceWorker' in navigator)) return;
  navigator.serviceWorker.controller?.postMessage({ type: 'ATTACHMENT_CACHE_CLEAR' });
}

/** Absolute /api URL for a resource id (matches the SW route matcher). */
export function resourceFileUrl(id: string): string {
  return `${location.origin}/api/resources/${id}/file`;
}
