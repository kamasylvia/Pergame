/** Content kinds the clipper recognises, in detection priority order. */
export type ContentType =
  | 'zhihu'
  | 'youtube'
  | 'youtube-channel'
  | 'bilibili'
  | 'vimeo'
  | 'tiktok'
  | 'twitter'
  | 'bluesky'
  | 'mastodon'
  | 'stackexchange'
  | 'github'
  | 'wikipedia'
  | 'pinterest'
  | 'website'
  | 'text';

/** Everything a parser can hand to the note builder. */
export interface ParsedContent {
  type: ContentType;
  title: string;
  /** Markdown body, already image-rewritten. */
  content: string;
  description?: string;
  author?: string;
  published?: string;
  url?: string;
  tags?: string[];
  /** Attachments stored while clipping. */
  images?: number;
  /** Extra template variables (video chapters, vote counts, …). */
  extra?: Record<string, unknown>;
}

/** Clip result the UI reports. */
export interface ClipOutcome {
  type: ContentType;
  title: string;
  noteId: string;
  images: number;
}

/** Parser contract: `null` means "this parser does not handle the URL". */
export interface ParserContext {
  /** Server-side GET (CORS-free) with optional safe headers. */
  proxy(url: string, init?: { headers?: Record<string, string> }): Promise<{ status: number; contentType: string; body: string }>;
  /** Server-side GET of bytes, base64 encoded. */
  proxyBinary(url: string): Promise<{ contentType: string; base64: string }>;
  /** Upload bytes as an attachment; returns the stored URL. */
  upload(bytes: Uint8Array, filename: string, contentType: string): Promise<string>;
  /** Plugin settings (cookie per host, template overrides, …). */
  setting<T = string>(key: string): Promise<T | undefined>;
}
