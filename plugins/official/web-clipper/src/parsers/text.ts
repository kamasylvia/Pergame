/** Fallback: non-URL clipboard text is saved verbatim. */
import type { ParsedContent } from '../types';

/** Wrap plain text as a note body. */
export function parseText(text: string): ParsedContent {
  const firstLine = text.trim().split('\n')[0] ?? '';
  return {
    type: 'text',
    title: firstLine.slice(0, 60) || 'Clipboard',
    content: `${text}\n`,
    description: '',
    extra: { chars: text.length },
  };
}
