/**
 * Resource/File utilities
 * Helpers for working with uploaded files and CDN resources
 */
import type { Node } from '~/stores';

/** Convert bytes to human-readable size (e.g., "1.5 MB") */
const readableFileSize = (bytes: number): string => {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
};

/** Check if a filetype MIME string represents an image */
const isImageFile = (filetype?: string): boolean => filetype?.startsWith('image/') ?? false;

/** Check if a filetype MIME string represents a video */
const isVideoFile = (filetype: string): boolean => filetype.startsWith('video/');

/** Check if a filetype MIME string represents an audio */
const isAudioFile = (filetype: string): boolean => filetype.startsWith('audio/');

/** Check if a filetype MIME string represents a PDF */
const isPdfFile = (filetype: string): boolean => filetype.startsWith('application/pdf');

/** Source-code / text extensions rendered as code blocks when embedded.
 * Backend mirror: `backend-rs/src/utils.rs` (`CODE_EXTS`) — keep in sync. */
const CODE_EXTS = ['py', 'rs', 'go', 'js', 'mjs', 'cjs', 'ts', 'tsx', 'jsx', 'java', 'c', 'h', 'cpp', 'hpp', 'cs', 'rb', 'php', 'sh', 'bash', 'zsh', 'fish', 'sql', 'r', 'swift', 'kt', 'scala', 'pl', 'lua', 'toml', 'yaml', 'yml', 'json', 'xml', 'xsd', 'ini', 'md', 'markdown', 'txt', 'csv', 'diff', 'log', 'dockerfile', 'makefile', 'bat', 'ps1', 'vim', 'el'];

/** Lowercased extension of a filename (without the dot), '' when none. */
const fileExt = (filename: string): string => {
  const name = filename.split(/[\\/]/).pop() || filename;
  const idx = name.lastIndexOf('.');
  if (idx > 0 && idx < name.length - 1) return name.slice(idx + 1).toLowerCase();
  return CODE_EXTS.includes(name.toLowerCase()) ? name.toLowerCase() : '';
};

/** True when the filename is a code/text file (embed renders as code block). */
const isCodeFile = (filename: string): boolean => CODE_EXTS.includes(fileExt(filename));

/** Map a code-file extension to a highlight.js language hint (best effort). */
const codeLanguage = (filename: string): string => {
  const ext = fileExt(filename);
  const map: Record<string, string> = {
    py: 'python', rs: 'rust', go: 'go', js: 'javascript', mjs: 'javascript', cjs: 'javascript',
    ts: 'typescript', tsx: 'typescript', jsx: 'javascript', java: 'java', cs: 'csharp',
    rb: 'ruby', php: 'php', sh: 'bash', bash: 'bash', zsh: 'bash', fish: 'bash', bat: 'batch',
    ps1: 'powershell', kt: 'kotlin', swift: 'swift', scala: 'scala', pl: 'perl', r: 'r',
    md: 'markdown', markdown: 'markdown', yml: 'yaml', toml: 'toml', ini: 'ini', xsd: 'xml',
    dockerfile: 'dockerfile', makefile: 'makefile', h: 'c', hpp: 'cpp', cpp: 'cpp',
  };
  return map[ext] || ext;
};

/** Embed URL for a resource: code files use the named route so the markdown
 * renderer can dispatch on the extension; others use the bare file URL. */
const resourceEmbedUrl = (node: Pick<Node, 'id'>, filename: string): string => {
  const { resourceURL } = useApi();
  return isCodeFile(filename) ? `${resourceURL(node as Node)}/${encodeURIComponent(filename)}` : resourceURL(node as Node);
};

/** Get the CDN preview URL for a resource node */
const resolvePreviewUrl = (node: Node): string => {
  const { resourceURL } = useApi();
  if (isImageFile((node.metadata?.filetype as string) || '')) {
    return resourceURL(node);
  }
  if (isPdfFile((node.metadata?.filetype as string) || '')) {
    return '/placeholder/pdf.webp';
  }
  return '/placeholder/file.webp';
};

/** Resolve the appropriate icon for a file based on its role and properties */
const resolveFileIcon = (mimeType: string) => {
  if (mimeType.startsWith('image/')) return 'image';
  if (mimeType.startsWith('video/')) return 'video';
  if (mimeType.startsWith('audio/')) return 'sidebar/music';
  if (mimeType.includes('pdf')) return 'sidebar/pdf';
  return 'sidebar/attachment';
};

/** Get human-readable type name for a file based on its MIME type */
const resolveFileType = (mimeType: string) => {
  if (mimeType.startsWith('image/')) return 'Image';
  if (mimeType.startsWith('video/')) return 'Video';
  if (mimeType.startsWith('audio/')) return 'Audio';
  if (mimeType.includes('pdf')) return 'PDF';
  if (mimeType.includes('zip') || mimeType.includes('archive')) return 'Archive';
  return 'File';
};

export { readableFileSize, resolvePreviewUrl, isImageFile, resolveFileIcon, resolveFileType, isPdfFile, isVideoFile, isAudioFile, CODE_EXTS, fileExt, isCodeFile, codeLanguage, resourceEmbedUrl };
