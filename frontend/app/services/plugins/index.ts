/** Plugin host services (PS-A). Public entry for the app and tests. */
export { DisposableBag } from './registry';
export { validateManifest, netAllows, compareVersions, isKnownCapability, type ManifestResult } from './manifest';
export {
  createMemoryKV,
  createNetClient,
  createPluginContext,
  type PluginHostInfo,
  type PluginRequest,
  type PluginRequestResult,
} from './context';
export { createRemoteKV } from './settings';
export { createMarkdownClient, markdownHooks, runPostProcessors } from './markdown';
export { createEditorClient, editorHooks } from './editor';
export { createPluginHost, type PluginHost, type PluginHostDeps, type PluginHostEntry } from './loader';
export {
  createUiClient,
  usePluginUi,
  clearPluginUi,
  pluginUiState,
  type PluginCommand,
  type PluginRibbon,
  type PluginUiClient,
} from './ui';
