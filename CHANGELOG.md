# Changelog

All notable changes to Pergame. The fork's own version line starts at 0.0.4 (2026-11-12); earlier tags are upstream Alexandrie history.

## [0.0.17] - 2026-11-28

- README rewritten (fork note, differences, Pergame screenshots) and CHANGELOG started.
- Doc pages show the note title in the navbar; Trash uses a trash-can icon.
- Rust review nits: dead render-task branch made explicit, unused parameter and test helper removed.

## [0.0.16] - 2026-11-27

- Sticky dashboard chrome: page content scrolls inside `#dashboard-scroll` while the navbar stays fixed; TOC tracking, BackToTop and print styles follow the new scroller.
- Sidebar split: fixed header with an independently scrolling file tree on desktop; mobile keeps the legacy whole-sidebar scroll.
- Context-menu delete honors the tree multi-selection (bulk confirm dialog).
- Docs page: All/Recent segmented tabs (sticky, persisted), global recently-viewed list via DQL with viewed dates and order passthrough.
- Categories page: single frame with sticky search; list rows collapse to dividers.
- Built-in themes +3 (Ocean, Rose, Slate, each light/dark); code palettes +4 (Tokyo Night, Nord, One Light, Catppuccin Latte).

## [0.0.15] - 2026-11-26

- Settings: offsite backup targets merged into the Backup page.

## [0.0.14] - 2026-11-26

- PDF: stock viewer with SiYuan-style annotations, vendored pdf.js cmaps/fonts/wasm, in-page preview for plain PDF links, fluid side panel.
- Doc/folder settings move into right-side panels; per-note document size override; settings Other drawer drained.
- Directory tree: copy-to with deep copy, manual refresh, settings gear, MCP binary paging, resource chevron fix.
- Nodes: 50-char title limit lifted; uploads routed through named URLs with legacy resolution.
- Editor: hardened image paste/drop uploads; image-host adapter carries binary MCP uploads.

## [0.0.13] - 2026-11-23

- Web-clipper v0.5.0: per-type templates, browser rendering, plugin settings UI.
- Render channel: camoufox render endpoint, builtin camoufox-cli engine alongside external MCP, zhihu wall defeats and risk model, production stack recovery.
- Image-host adapter for uploads; code preview themes; admin fixes.
- Resources-out-of-tree overhaul; Move-to menu with searchable Clone/Move picker; clip progress pipeline end to end.

## [0.0.12] - 2026-11-21

- Released to production without a tag (rolled into the 0.0.13 notes above): web-clipper render batch, zhihu risk model, camofox network recovery.

## [0.0.11] - 2026-11-12

- Snapshot empty copy, dead DR i18n strings, sidebar spacing fixes.

## [0.0.10] - 2026-11-12

- Token-authed snapshot download links; zero-target zips live in S3 with user-chosen expiry.

## [0.0.9] - 2026-11-12

- Restore-from-zip button icon; WebDAV list prefix/self-skip and MKCOL status fixes.

## [0.0.8] - 2026-11-12

- WebDAV probe lists the store root instead of a double prefix.

## [0.0.7] - 2026-11-12

- S3 404 maps to NotFound so owner claim works on empty stores.

## [0.0.6] - 2026-11-12

- Backup target INSERT placeholder order and UUIDv7 ids repaired.

## [0.0.5] - 2026-11-12

- Full-backup rebuild: all entity ids unified on UUIDv7; snapshots write straight to user targets (fan-out removed); portable object-set zip; restore covers permissions, invitations, trash and profile with a force gate; account deletion with snapshot gate; backup/restore locking; dr-backup sidecar deleted.

## [0.0.4] - 2026-11-12

- Fork baseline: Collectanea→Pergame rename and branding (P mark, favicons), Rust backend on PostgreSQL, plugin system, MCP server, DQL dataview engine, trash bin, snapshot-target backup subsystem, self-contained and external compose models, same-origin proxy, amd64-only GHCR pipeline, first live NAS deployment.
