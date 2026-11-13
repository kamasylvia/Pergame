/**
 * Declarative UI surface for plugins (PS-B, first slice).
 *
 * Plugins never mount components: they register a name and a callback, and
 * the host decides where it appears. Everything registered here is reactive
 * so the Dock and the command palette update without a reload, and disposing
 * the returned handle removes the entry (design §3: unload is total).
 */
import { shallowReactive } from 'vue';
import type { Disposable } from '../../types/plugin';

/** One registered command (runs from the command palette). */
export interface PluginCommand {
  id: string;
  title: string;
  pluginId: string;
  run(): void;
}

/** One registered ribbon button (rendered in the sidebar dock). */
export interface PluginRibbon {
  id: string;
  icon: string;
  title: string;
  pluginId: string;
  run(): void;
}

/** Reactive tables the UI reads; dispose-able registrations write into them. */
export const pluginUiState = {
  commands: shallowReactive<PluginCommand[]>([]),
  ribbons: shallowReactive<PluginRibbon[]>([]),
};

/** Registration API handed to each plugin (bound to its id + bag). */
export interface PluginUiClient {
  command(id: string, title: string, run: () => void): Disposable;
  ribbon(icon: string, title: string, run: () => void): Disposable;
}

/** Remove one entry by id, returning whether it existed. */
function removeById<T extends { id: string }>(rows: T[], id: string): boolean {
  const index = rows.findIndex(row => row.id === id);
  if (index === -1) return false;
  rows.splice(index, 1);
  return true;
}

/**
 * Build the UI client for one plugin. `track` records the registration so the
 * plugin's bag releases it on unload even if the plugin forgets the handle.
 */
export function createUiClient(pluginId: string, track: (disposable: Disposable) => void): PluginUiClient {
  function register<T extends { id: string }>(rows: T[], row: T): Disposable {
    if (rows.some(existing => existing.id === row.id)) {
      throw new Error(`'${row.id}' is already registered`);
    }
    rows.push(row);
    const disposable: Disposable = { dispose: () => void removeById(rows, row.id) };
    track(disposable);
    return disposable;
  }

  return {
    command: (id, title, run) =>
      register(pluginUiState.commands, { id: `${pluginId}:${id}`, pluginId, title, run }),
    ribbon: (icon, title, run) =>
      register(pluginUiState.ribbons, { id: `${pluginId}:${title}`, pluginId, icon, title, run }),
  };
}

/** Test/teardown helper: drop every registration of one plugin. */
export function clearPluginUi(pluginId: string): void {
  for (let i = pluginUiState.commands.length - 1; i >= 0; i--) {
    if (pluginUiState.commands[i]!.pluginId === pluginId) pluginUiState.commands.splice(i, 1);
  }
  for (let i = pluginUiState.ribbons.length - 1; i >= 0; i--) {
    if (pluginUiState.ribbons[i]!.pluginId === pluginId) pluginUiState.ribbons.splice(i, 1);
  }
}

/** Vue-facing accessor (reads the reactive tables). */
export function usePluginUi() {
  return pluginUiState;
}
