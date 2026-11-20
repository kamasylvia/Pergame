/**
 * Declarative plugin dialogs (PS-B tail).
 *
 * A plugin describes fields; the host renders them in its own modal. Plugins
 * never mount Vue components, so the dialog keeps the app's look and the
 * plugin cannot reach into the DOM.
 */
import { shallowReactive } from 'vue';
import type { ModalResult, ModalSpec } from '../../types/plugin';

/** One dialog waiting for the user. */
export interface PendingModal {
  id: number;
  spec: ModalSpec;
  resolve(result: ModalResult): void;
}

interface ModalQueueState {
  current: PendingModal | null;
}

/** Same page-wide identity rule as the other registries. */
const MODAL_KEY = '__pergamePluginModalQueue';

// The *reactive* object is the singleton: wrapping a raw object and mutating
// the raw one leaves Vue blind to the change (the component reads the proxy).
export const pluginModalState: ModalQueueState = (() => {
  const holder = globalThis as unknown as Record<string, ModalQueueState | undefined>;
  holder[MODAL_KEY] ??= shallowReactive<ModalQueueState>({ current: null });
  return holder[MODAL_KEY];
})();

const state = pluginModalState;
let nextId = 1;

/** Open a dialog; resolves `null` when the user cancels. */
export function openPluginModal(spec: ModalSpec): Promise<ModalResult> {
  // One at a time: a second request replaces the first, which resolves null so
  // a plugin awaiting it is not stuck forever.
  state.current?.resolve(null);
  const { promise, resolve } = Promise.withResolvers<ModalResult>();
  state.current = { id: nextId++, spec, resolve };
  return promise;
}

/** Close the active dialog and hand the values back. */
export function settlePluginModal(result: ModalResult): void {
  const pending = state.current;
  if (!pending) return;
  state.current = null;
  pending.resolve(result);
}

/**
 * Replace one field's `choices` on the dialog that is already open.
 *
 * `state.current` is shallow-reactive, so the spec object itself must be
 * swapped for the host to re-render: mutate a copy, then assign a new spec.
 * Used by plugins that open the dialog first and fill slow lists later.
 */
export function patchPluginModalChoices(fieldKey: string, choices: Array<{ value: string; label: string }>): void {
  const pending = state.current;
  if (!pending) return;
  state.current = {
    ...pending,
    spec: {
      ...pending.spec,
      fields: pending.spec.fields.map(f => (f.key === fieldKey ? { ...f, choices } : f)),
    },
  };
}
