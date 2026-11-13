/**
 * Markdown hooks for plugins (PS-C).
 *
 * Plugins register *renderers*, never markdown-it instances: the host owns
 * the pipeline, so a plugin cannot replace core rules or leak a mutated
 * parser into the next document. Registrations bump a version; the compiler
 * cache rebuilds on the next render, which is how a newly loaded plugin takes
 * effect without a reload.
 *
 * PS-C deliberately does not migrate the built-in blocks (index, dataview,
 * mermaid, …): those are verified and stay put until a dedicated batch.
 */
import type { Disposable } from '../../types/plugin';

/** What a code-block renderer receives. */
export interface CodeBlockContext {
  /** Raw block content (no fence, no language tag). */
  content: string;
  /** Fence info string after the language (` ```js title=x ` → `title=x`). */
  info: string;
  /** Escape helper for building HTML. */
  escapeHtml(text: string): string;
}

/** Renders a fenced block; return HTML, or `null` to fall through. */
export type CodeBlockRenderer = (ctx: CodeBlockContext) => string | null;

/** What a container renderer receives. */
export interface ContainerContext extends CodeBlockContext {
  /** Container name from the opening marker (`:::name`). */
  name: string;
}

/** Renders `:::name … :::`; return HTML, or `null` to fall through. */
export type ContainerRenderer = (ctx: ContainerContext) => string | null;

/** Post-processor: rewrite rendered HTML once per document. */
export type PostProcessor = (html: string) => string;

/** Registry state read by the markdown compiler. */
interface MarkdownHookState {
  /** Bumped on every registration change; the compiler cache keys on it. */
  version: number;
  codeBlocks: Map<string, CodeBlockRenderer>;
  containers: Map<string, ContainerRenderer>;
  postProcessors: Set<PostProcessor>;
}

/**
 * One registry per page, not per module instance.
 *
 * Bundlers can hand the same file two module identities (alias vs relative
 * import, or a dev-server reload), and two registries silently mean "the
 * plugin registered into a table nobody reads". Anchoring the state on
 * `globalThis` makes the identity irrelevant.
 */
const REGISTRY_KEY = '__pergamePluginMarkdownHooks';

export const markdownHooks: MarkdownHookState = (() => {
  const holder = globalThis as unknown as Record<string, MarkdownHookState | undefined>;
  holder[REGISTRY_KEY] ??= {
    version: 0,
    codeBlocks: new Map(),
    containers: new Map(),
    postProcessors: new Set(),
  };
  return holder[REGISTRY_KEY];
})();

/** The API handed to a plugin for markdown hooks. */
export interface PluginMarkdownClient {
  registerCodeBlock(lang: string, render: CodeBlockRenderer): Disposable;
  registerContainer(name: string, render: ContainerRenderer): Disposable;
  registerPostProcessor(fn: PostProcessor): Disposable;
}

function normalizeName(name: string): string {
  const value = name.trim().toLowerCase();
  if (!/^[a-z0-9][a-z0-9-]{0,31}$/.test(value)) {
    throw new Error(`'${name}' must be a lowercase name (a-z, 0-9, dashes)`);
  }
  return value;
}

/** Build the markdown client for one plugin (registrations are reversible). */
export function createMarkdownClient(track: (disposable: Disposable) => void): PluginMarkdownClient {
  function bump() {
    markdownHooks.version += 1;
  }

  return {
    registerCodeBlock(lang, render) {
      const key = normalizeName(lang);
      if (markdownHooks.codeBlocks.has(key)) {
        throw new Error(`code block '${key}' is already registered`);
      }
      markdownHooks.codeBlocks.set(key, render);
      bump();
      const disposable: Disposable = {
        dispose: () => {
          if (markdownHooks.codeBlocks.get(key) === render) markdownHooks.codeBlocks.delete(key);
          bump();
        },
      };
      track(disposable);
      return disposable;
    },
    registerContainer(name, render) {
      const key = normalizeName(name);
      if (markdownHooks.containers.has(key)) {
        throw new Error(`container '${key}' is already registered`);
      }
      markdownHooks.containers.set(key, render);
      bump();
      const disposable: Disposable = {
        dispose: () => {
          if (markdownHooks.containers.get(key) === render) markdownHooks.containers.delete(key);
          bump();
        },
      };
      track(disposable);
      return disposable;
    },
    registerPostProcessor(fn) {
      markdownHooks.postProcessors.add(fn);
      bump();
      const disposable: Disposable = {
        dispose: () => {
          markdownHooks.postProcessors.delete(fn);
          bump();
        },
      };
      track(disposable);
      return disposable;
    },
  };
}

/** Apply every post-processor, in registration order. */
export function runPostProcessors(html: string): string {
  let out = html;
  for (const fn of markdownHooks.postProcessors) {
    try {
      out = fn(out);
    } catch (err) {
      console.warn('[plugins] markdown post-processor threw', err);
    }
  }
  return out;
}
