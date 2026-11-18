import { EditorView } from '@codemirror/view';
import type { Node } from '~/stores';
import { resourceEmbedUrl } from '~/helpers/resources';

interface UploadHandlersParams {
  resourcesStore: ReturnType<typeof import('~/stores').useResourcesStore>;
  nodeId?: string;
  insertText: (text: string) => void;
}

export function createUploadsHandlers({ resourcesStore, nodeId, insertText }: UploadHandlersParams) {
  return EditorView.domEventHandlers({
    paste: event => {
      const items = event.clipboardData?.items;
      if (!items) return;
      for (const item of items) {
        if (item.kind === 'file') {
          const file = item.getAsFile();
          if (!file) return;
          const body = new FormData();
          body.append('parent_id', nodeId || '');
          body.append('file', file);
          resourcesStore.post(body).then((result: Node) => {
            insertText(`![${file.name}](${resourceEmbedUrl(result, file.name)})\n`);
          });
        }
      }
    },
    drop: event => {
      const items = event.dataTransfer?.items;
      if (!items) return;
      for (const item of items) {
        if (item.kind === 'file') {
          const file = item.getAsFile();
          if (!file) return;
          const body = new FormData();
          body.append('parent_id', nodeId || '');
          body.append('file', file);
          resourcesStore.post(body).then((result: Node) => {
            insertText(`![${file.name}](${resourceEmbedUrl(result, file.name)})\n`);
          });
        }
      }
    },
  });
}
