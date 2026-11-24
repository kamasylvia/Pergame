import { Decoration, EditorView, ViewPlugin, WidgetType, type DecorationSet, type ViewUpdate } from '@codemirror/view';
import type { Range } from '@codemirror/state';
import type { CompletionContext, CompletionResult } from '@codemirror/autocomplete';
import type { Node } from '~/stores';
import { INTERNAL_LINK_REGEX } from '~/helpers/markdown/internal-links';

export interface InternalLinkProvider {
  getName: (id: string) => string | undefined;
  getDocuments: () => Node[];
  /** Fetch one row by id (lazy store); the plugin repaints once it lands. */
  fetchName?: (id: string) => Promise<unknown>;
}

class InternalLinkWidget extends WidgetType {
  constructor(
    readonly id: string,
    readonly name: string,
  ) {
    super();
  }

  override eq(other: InternalLinkWidget) {
    return other.id === this.id && other.name === this.name;
  }

  toDOM() {
    const span = document.createElement('span');
    span.className = 'cm-internal-link';
    span.textContent = `#${this.name}`;
    span.title = `#${this.id}`;
    return span;
  }
}

function buildDecorations(view: EditorView, provider: InternalLinkProvider): { set: DecorationSet; missing: string[] } {
  const decorations: Range<Decoration>[] = [];
  const missing: string[] = [];
  const { selection } = view.state;

  for (const { from, to } of view.visibleRanges) {
    const text = view.state.doc.sliceString(from, to);
    for (const match of text.matchAll(INTERNAL_LINK_REGEX)) {
      const start = from + match.index;
      const end = start + match[0].length;

      // Keep the raw id editable when the cursor/selection touches it
      if (selection.ranges.some(range => range.from <= end && range.to >= start)) continue;

      const id = match[1]!;
      const name = provider.getName(id);
      if (!name) {
        missing.push(id);
        continue; // Unknown document: leave the raw id visible
      }

      decorations.push(Decoration.replace({ widget: new InternalLinkWidget(id, name) }).range(start, end));
    }
  }

  return { set: Decoration.set(decorations), missing };
}

export function createInternalLinks(provider: InternalLinkProvider) {
  const plugin = ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      attempted = new Set<string>();
      destroyed = false;

      constructor(view: EditorView) {
        this.decorations = this.build(view);
      }

      build(view: EditorView): DecorationSet {
        const { set, missing } = buildDecorations(view, provider);
        if (provider.fetchName) {
          const fresh = missing.filter(id => !this.attempted.has(id));
          if (fresh.length) {
            for (const id of fresh) this.attempted.add(id);
            Promise.allSettled(fresh.map(id => provider.fetchName!(id))).then(() => {
              // Names may have landed: repaint once. Still-missing ids stay
              // raw (the attempted set stops refetch loops on dead links).
              if (!this.destroyed) view.dispatch({});
            });
          }
        }
        return set;
      }

      update(update: ViewUpdate) {
        if (update.docChanged || update.selectionSet || update.viewportChanged) {
          this.decorations = this.build(update.view);
        }
      }

      destroy() {
        this.destroyed = true;
      }
    },
    {
      decorations: v => v.decorations,
    },
  );

  const theme = EditorView.baseTheme({
    '.cm-internal-link': {
      color: 'var(--primary)',
      background: 'var(--accent-bg)',
      borderRadius: '4px',
      padding: '0 4px',
    },
  });

  // Autocomplete source: type "#" to search and insert a document link
  const completionSource = (context: CompletionContext): CompletionResult | null => {
    const word = context.matchBefore(/#[^\s#]*/);
    if (!word) return null;

    const documents = provider.getDocuments();
    if (!documents.length) return null;

    return {
      from: word.from,
      options: documents.map(doc => ({
        label: `#${doc.name}`,
        type: 'reference',
        detail: 'Document',
        apply: `#${doc.id}`,
      })),
      validFor: /^#[^\s#]*$/,
    };
  };

  return { extension: [plugin, theme], completionSource };
}
