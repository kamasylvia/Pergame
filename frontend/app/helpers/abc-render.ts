/**
 * ABC notation rendering (Joplin parity): ` ```abc ` blocks hydrate to
 * engraved sheet music via abcjs (mermaid post-mount twin). Options come
 * from the `abcjsOptions` preference (JSON5-ish object, Joplin-style).
 */
export async function renderAbcBlocks(root: HTMLElement): Promise<void> {
  const elements = Array.from(root.querySelectorAll<HTMLElement>('pre.abc-notation:not([data-abc-done])'));
  if (!elements.length) return;

  // Lazy import: abcjs (~400KB) loads only on pages containing ABC blocks,
  // same as mermaid's loadMermaid (static import would bloat every page).
  const abcjs = await import('abcjs');
  let options: Record<string, unknown> = {};
  try {
    const prefs = usePreferencesStore();
    const raw = (prefs.preferences as Record<string, unknown>).abcjsOptions;
    if (typeof raw === 'string' && raw.trim()) options = JSON.parse(raw) as Record<string, unknown>;
    else if (raw && typeof raw === 'object') options = raw as Record<string, unknown>;
  } catch {
    // Defaults below.
  }

  for (const el of elements) {
    const source = el.dataset.abcSource ?? el.textContent ?? '';
    el.dataset.abcSource ??= source;
    el.dataset.abcDone = '1';
    try {
      abcjs.renderAbc(el, source.trim(), {
        responsive: 'resize',
        ...options,
      });
      el.classList.remove('abc-error');
    } catch {
      el.classList.add('abc-error');
    }
  }
}
