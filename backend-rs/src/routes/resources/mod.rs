//! /resources routes — file upload, avatar upload, file update, authed file
//! download (fork: replaces the public-bucket CDN reads; see `download_file`).

mod download;
mod imgbed;
mod orphans;
mod upload;

pub use download::{
    DownloadQuery, download_avatar, download_file, download_file_named, download_legacy,
};
pub use imgbed::{TransferInput, import_external, migrate_local, status as imgbed_status};
pub use orphans::{
    PreviewOrphanQuery, PurgeOrphansInput, list_orphans, preview_orphan, purge_orphans,
};
pub use upload::{UploadedPart, update_file, upload_avatar, upload_file};
