/**
 * Plugin host contract (PS-A, spec `.work/plugin-system/DESIGN.md`).
 *
 * This file is the public surface plugins program against. It stays free of
 * Nuxt/Vue imports so the kernel can be loaded and tested outside the app;
 * the `pergame` module handed to a plugin is built from these types.
 *
 * Versioning: `HOST_API_VERSION` is the contract major. A plugin declaring a
 * different major is refused at load time (design §0: no silent breakage).
 */

/** Host contract major version. Bump only on breaking changes. */
export const HOST_API_VERSION = 1;

/** Capabilities a manifest may request. `net:<domain>` / `net:*` are open-ended. */
export const STATIC_CAPABILITIES = ['nodes:read', 'nodes:write', 'ui', 'markdown:render', 'settings'] as const;

/** One declared capability (`nodes:read`, `net:example.com`, …). */
export type PluginCapability = string;

/** Parsed manifest.json. */
export interface PluginManifest {
  /** Slug: `[a-z0-9]` and dashes, 1-64 chars. */
  id: string;
  name: string;
  version: string;
  /** Host API major the plugin was written against (string, e.g. "1"). */
  apiVersion: string;
  /** ESM entry, relative to the plugin root. */
  entry: string;
  capabilities: PluginCapability[];
  /** Optional semver floor for the host app version. */
  minHostVersion?: string;
  /** Optional author line shown in the settings list. */
  author?: string;
  description?: string;
}

/** Cleanup handle returned by every registration (design §3: unload is total). */
export interface Disposable {
  dispose(): void;
}

/** Minimal settings store handed to a plugin (its own namespace only). */
export interface KVStore {
  get<T = unknown>(key: string): Promise<T | undefined>;
  set(key: string, value: unknown): Promise<void>;
  delete(key: string): Promise<void>;
}

/** One node as the plugin data client returns it (wire shape, trimmed). */
export interface PluginNode {
  id: string;
  parent_id?: string;
  name: string;
  description?: string;
  tags?: string;
  role: number;
  content?: string;
  content_compiled?: string;
  metadata?: Record<string, unknown>;
  created_timestamp: number;
  updated_timestamp: number;
}

/** L1 data client (`ctx.nodes`). Permissions are the caller's — never more. */
export interface NodeClient {
  get(id: string): Promise<PluginNode>;
  listChildren(parentId?: string): Promise<PluginNode[]>;
  create(input: { name: string; parent_id?: string; content?: string; role?: number }): Promise<PluginNode>;
  update(id: string, patch: { name?: string; content?: string; description?: string; tags?: string }): Promise<PluginNode>;
  remove(id: string): Promise<void>;
}

/** L1 uploads (`ctx.resources`). */
export interface ResourceClient {
  upload(file: File | Blob, parentId?: string): Promise<PluginNode>;
}

/** L1 search (`ctx.search`). */
export interface SearchClient {
  fulltext(query: string, opts?: { content?: boolean; limit?: number }): Promise<PluginNode[]>;
  query(dql: string): Promise<PluginNode[]>;
}

/** UI surface: notices (PS-A), commands and ribbons (PS-B). */
export interface UiClient {
  notice(message: string, opts?: { type?: 'info' | 'success' | 'warning' | 'error'; timeout?: number }): void;
  /** Register a command; it appears in the command palette. */
  command(id: string, title: string, run: () => void): Disposable;
  /** Register a sidebar dock button. */
  ribbon(icon: string, title: string, run: () => void): Disposable;
  /** Ask the user for input; resolves to the field values or `null`. */
  modal(spec: ModalSpec): Promise<ModalResult>;
}

/** Network facade (design §3: capability-gated, method/size limited). */
export interface NetClient {
  fetch(url: string | URL, init?: RequestInit): Promise<Response>;
  /**
   * Server-side fetch for hosts the browser cannot read (CORS). Same domain
   * scope as `fetch`, plus SSRF guards on the backend; no browser cookies are
   * borrowed — pass credentials explicitly when a site needs them.
   */
  proxy(url: string, init?: { method?: 'GET' | 'POST'; body?: string; headers?: Record<string, string> }): Promise<{
    status: number;
    contentType: string;
    body: string;
  }>;
  /** Server-side GET of bytes (images, fonts) as a base64 payload. */
  proxyBinary(url: string): Promise<{ contentType: string; base64: string }>;
}

/** One field of a plugin dialog. */
export interface ModalField {
  key: string;
  label: string;
  type: 'text' | 'textarea' | 'select' | 'checkbox';
  value?: string | boolean;
  placeholder?: string;
  /** Options for `select`. */
  choices?: Array<{ value: string; label: string }>;
  required?: boolean;
}

/** Declarative dialog spec; the host renders and returns the values. */
export interface ModalSpec {
  title: string;
  description?: string;
  fields: ModalField[];
  submitLabel?: string;
  cancelLabel?: string;
}

/** Values keyed by field; `null` when the user cancels. */
export type ModalResult = Record<string, string | boolean> | null;

/** Markdown render hooks (PS-C). */
export interface MarkdownClient {
  registerCodeBlock(lang: string, render: CodeBlockRenderer): Disposable;
  registerContainer(name: string, render: ContainerRenderer): Disposable;
  registerPostProcessor(fn: (html: string) => string): Disposable;
}

/** Editor hooks (PS-C): CM6 extensions appended to the editor state. */
export interface EditorClient {
  registerCmExtension(extension: unknown): Disposable;
}

/** Renderer context for a fenced block. */
export interface CodeBlockContext {
  content: string;
  info: string;
  escapeHtml(text: string): string;
}

/** Renderer context for a `:::name` container. */
export interface ContainerContext extends CodeBlockContext {
  name: string;
}

/** Renders a fence; `null` falls through to the host renderer. */
export type CodeBlockRenderer = (ctx: CodeBlockContext) => string | null;

/** Renders a container; `null` falls through to the host renderer. */
export type ContainerRenderer = (ctx: ContainerContext) => string | null;

/** The object plugins receive in `onload(ctx)`. */
export interface PluginContext {
  api: {
    version: number;
    host: { name: string; version: string; client: string };
  };
  manifest: PluginManifest;
  nodes: NodeClient;
  resources: ResourceClient;
  search: SearchClient;
  settings: KVStore;
  ui: UiClient;
  net: NetClient;
  markdown: MarkdownClient;
  editor: EditorClient;
}

/** Shape a plugin module's default export must provide. */
export interface PluginInstance {
  onload(ctx: PluginContext): void | Promise<void>;
  onunload?(): void | Promise<void>;
}

/** Default export of `main.mjs`. */
export type PluginConstructor = new () => PluginInstance;

/** Result of one load attempt, surfaced to the settings UI later (PS-B). */
export interface LoadedPlugin {
  manifest: PluginManifest;
  instance: PluginInstance;
  dispose: () => Promise<void>;
}
