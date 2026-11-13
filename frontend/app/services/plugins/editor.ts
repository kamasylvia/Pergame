/**
 * CodeMirror extensions contributed by plugins (PS-C).
 *
 * The host hands plugins no editor instance: they append an extension to a
 * registry that the editor reads when it builds its state. Extensions are
 * plain CM6 values, so the host stays on the same version and a plugin cannot
 * fork the editor.
 */
import type { Extension } from '@codemirror/state';
import type { Disposable } from '../../types/plugin';

/** Registry state read by the editor factory. */
interface EditorHookState {
  version: number;
  extensions: Extension[];
}

/** Same page-wide identity rule as the markdown registry. */
const EDITOR_KEY = '__pergamePluginEditorHooks';

export const editorHooks: EditorHookState = (() => {
  const holder = globalThis as unknown as Record<string, EditorHookState | undefined>;
  holder[EDITOR_KEY] ??= { version: 0, extensions: [] };
  return holder[EDITOR_KEY];
})();

/** The API handed to a plugin for editor extensions. */
export interface PluginEditorClient {
  registerCmExtension(extension: Extension): Disposable;
}

/** Build the editor client for one plugin. */
export function createEditorClient(track: (disposable: Disposable) => void): PluginEditorClient {
  return {
    registerCmExtension(extension) {
      editorHooks.extensions.push(extension);
      editorHooks.version += 1;
      const disposable: Disposable = {
        dispose: () => {
          const index = editorHooks.extensions.indexOf(extension);
          if (index !== -1) editorHooks.extensions.splice(index, 1);
          editorHooks.version += 1;
        },
      };
      track(disposable);
      return disposable;
    },
  };
}
