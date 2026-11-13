/**
 * Registration bookkeeping (PS-A).
 *
 * Every API surface a plugin touches goes through a registry, so `unload`
 * can reclaim it all — the design's answer to Obsidian's "plugins leak on
 * unload" problem (spec §3). Cleanup runs LIFO: later registrations may
 * depend on earlier ones.
 */
import type { Disposable } from '../../types/plugin';

/** Records disposables and releases them newest-first. */
export class DisposableBag implements Disposable {
  private entries: Disposable[] = [];
  private finished = false;

  /** Track a disposable (or any cleanup callback). */
  add(target: Disposable | (() => void)): Disposable {
    const disposable = typeof target === 'function' ? { dispose: target } : target;
    if (this.finished) {
      // A late registration after unload would leak: release it immediately.
      disposable.dispose();
      return disposable;
    }
    this.entries.push(disposable);
    return disposable;
  }

  /** True once `dispose` ran. */
  get disposed(): boolean {
    return this.finished;
  }

  /** Release everything, newest first. Idempotent; never throws. */
  dispose(): void {
    if (this.finished) return;
    this.finished = true;
    while (this.entries.length) {
      const entry = this.entries.pop()!;
      try {
        entry.dispose();
      } catch (err) {
        console.warn('[plugins] disposable threw during unload', err);
      }
    }
  }
}
