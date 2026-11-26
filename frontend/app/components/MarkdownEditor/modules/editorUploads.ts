import { EditorView } from '@codemirror/view';
import type { Node } from '~/stores';
import { resourceEmbedUrl } from '~/helpers/resources';

interface UploadHandlersParams {
  resourcesStore: { post: (body: FormData) => Promise<Node> };
  nodeId?: string;
  insertText: (text: string) => void;
  notifyError: (message: string) => void;
}

/**
 * Files from a paste/drop payload, read from both `items` and `files`
 * (Firefox and Chromium populate them differently; the same file can
 * appear in both, so dedupe by identity + shape).
 */
function payloadFiles(data: DataTransfer | null | undefined): File[] {
  if (!data) return [];
  const out: File[] = [];
  const seen = new Set<string>();
  const push = (file: File | null) => {
    if (!file) return;
    const key = `${file.name}|${file.size}|${file.type}|${file.lastModified}`;
    if (seen.has(key)) return;
    seen.add(key);
    out.push(file);
  };
  if (data.items) {
    for (const item of data.items) {
      if (item.kind === 'file') push(item.getAsFile());
    }
  }
  if (data.files) {
    for (const file of data.files) push(file);
  }
  return out;
}

export function createUploadsHandlers({ resourcesStore, nodeId, insertText, notifyError }: UploadHandlersParams) {
  const upload = (file: File) => {
    const body = new FormData();
    body.append('parent_id', nodeId || '');
    body.append('file', file);
    resourcesStore.post(body).then(
      (result: Node) => {
        insertText(`![${file.name}](${resourceEmbedUrl(result, file.name)})\n`);
      },
      (error: unknown) => {
        // Rejections used to vanish (no catch): every failed paste/drop
        // looked like "nothing happens". Surface the server message.
        notifyError(error instanceof Error ? error.message : String(error));
      },
    );
  };

  return EditorView.domEventHandlers({
    paste: event => {
      const files = payloadFiles(event.clipboardData);
      if (!files.length) return;
      event.preventDefault();
      for (const file of files) upload(file);
    },
    drop: event => {
      const files = payloadFiles(event.dataTransfer);
      if (!files.length) return;
      // Without this the browser navigates away to the dropped file.
      event.preventDefault();
      for (const file of files) upload(file);
    },
  });
}
