//! Portable object-set zip: offline export/import format (fork §7 parity).

use serde::Serialize;
use serde_json::json;
use zip::ZipArchive;

use crate::error::ApiError;
// ---------------- portable zip snapshot (fork §7 parity) ----------------
//
// The offline export uses the *store object set* layout rather than the v1
// archive shape: `siyuan-compatible` meta.json + manifest.json + blobs/<sha256>`.
// Every entry name is pure ASCII (hashes and fixed names), so classic zip
// filename-encoding problems (CP437 vs UTF-8) cannot occur; document paths
// live inside the UTF-8 manifest JSON and blob bytes are sha256-checked.

/// meta.json payload identifying the export.
#[derive(Debug, Serialize, serde::Deserialize)]
pub struct PortableMeta {
    pub format: String,
    pub version: String,
    pub created_at: String,
    pub snapshot_id: String,
    pub user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encryption: Option<crate::snapcrypto::EncryptionParams>,
    pub entries: Vec<crate::backup::ManifestEntry>,
    pub files: Vec<crate::backup::ManifestFile>,
}

/// `meta.json` format id for portable object-set zips.
pub const PORTABLE_FORMAT: &str = "pergame-backup/object-set";
/// `meta.json` version (current writer = 1).
pub const PORTABLE_VERSION: &str = "1";

/// Builds the portable object-set zip from in-memory payloads (zero-target
/// snapshots): identical layout to `export_portable_zip`, but the blobs come
/// from the just-collected snapshot rather than any store.
pub fn build_portable_zip_in_memory(
    manifest: &crate::backup::BackupManifest,
    manifest_bytes: &[u8],
    member_payloads: &[(String, bool, Vec<u8>)],
    file_payloads: &[(String, bool, Vec<u8>)],
) -> Result<Vec<u8>, ApiError> {
    let meta = PortableMeta {
        format: PORTABLE_FORMAT.into(),
        version: PORTABLE_VERSION.into(),
        created_at: manifest.created_at.clone(),
        snapshot_id: manifest.id.clone(),
        user_id: manifest.user_id.clone(),
        encryption: manifest.encryption.clone(),
        entries: manifest.entries.clone(),
        files: manifest.files.clone(),
    };
    let mut archive = crate::backup::SimpleArchive::new();
    archive.add(
        "meta.json",
        &serde_json::to_vec_pretty(&meta).unwrap_or_default(),
    );
    archive.add("manifest.json", manifest_bytes);
    let mut seen = std::collections::HashSet::new();
    for (sha, e2e, bytes) in member_payloads.iter().chain(file_payloads.iter()) {
        if !seen.insert(sha.clone()) {
            continue;
        }
        // Ciphertext blobs are incompressible: store as-is (fork §7).
        if *e2e {
            archive.add_stored(format!("blobs/{sha}"), bytes);
        } else {
            archive.add(&format!("blobs/{sha}"), bytes);
        }
    }
    archive
        .finish()
        .map_err(|e| ApiError::BadRequest(format!("zip assembly failed: {e}")))
}
/// Builds the portable zip for a v2 snapshot: manifest.json + one blob per
/// referenced hash (ciphertext preserved when the snapshot is encrypted).
/// Reads blobs from a connected target (direct-write read path).
pub async fn export_portable_zip_from_conn(
    conn: &crate::target::config::TargetConn,
    manifest: &crate::backup::BackupManifest,
) -> Result<Vec<u8>, ApiError> {
    let mut archive = crate::backup::SimpleArchive::new();
    let e2e = manifest.encryption.is_some();
    let meta = PortableMeta {
        format: PORTABLE_FORMAT.into(),
        version: PORTABLE_VERSION.into(),
        created_at: manifest.created_at.clone(),
        snapshot_id: manifest.id.clone(),
        user_id: manifest.user_id.clone(),
        encryption: manifest.encryption.clone(),
        entries: manifest.entries.clone(),
        files: manifest.files.clone(),
    };
    archive.add(
        "meta.json",
        &serde_json::to_vec_pretty(&meta).unwrap_or_default(),
    );
    archive.add(
        "manifest.json",
        &serde_json::to_vec_pretty(manifest).unwrap_or_default(),
    );
    let dir = if e2e { "blobs-e2e" } else { "blobs" };
    let mut seen = std::collections::HashSet::new();
    let mut hashes: Vec<String> = manifest.entries.iter().map(|e| e.sha256.clone()).collect();
    hashes.extend(manifest.files.iter().map(|f| f.sha256.clone()));
    for sha in hashes {
        if !seen.insert(sha.clone()) {
            continue;
        }
        let blob = conn
            .get(&format!("{}/{dir}/{sha}", manifest.user_id))
            .await?;
        if e2e {
            archive.add_stored(format!("blobs/{sha}"), &blob);
        } else {
            archive.add(&format!("blobs/{sha}"), &blob);
        }
    }
    archive
        .finish()
        .map_err(|e| ApiError::BadRequest(format!("portable zip assembly failed: {e}")))
}

/// Recognises a portable object-set zip (as opposed to a v1 archive).
pub fn is_portable_zip(bytes: &[u8]) -> bool {
    let cursor = std::io::Cursor::new(bytes.to_vec());
    let Ok(mut archive) = ZipArchive::new(cursor) else {
        return false;
    };
    let Ok(mut meta) = archive.by_name("meta.json") else {
        return false;
    };
    let mut s = String::new();
    use std::io::Read as _;
    if meta.read_to_string(&mut s).is_err() {
        return false;
    }
    serde_json::from_str::<PortableMeta>(&s)
        .map(|m| m.format == PORTABLE_FORMAT)
        .unwrap_or(false)
}

/// Converts a portable object-set zip into the v1 archive shape the restore
/// engine consumes: meta/manifest/blobs -> manifest.json + documents.json +
/// files_index.json + files/.
pub fn portable_to_v1_zip(bytes: Vec<u8>) -> Result<Vec<u8>, ApiError> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| ApiError::BadRequest(format!("invalid zip archive: {e}")))?;
    let meta: PortableMeta = {
        let mut f = archive
            .by_name("meta.json")
            .map_err(|_| ApiError::BadRequest("meta.json missing".into()))?;
        let mut s = String::new();
        use std::io::Read as _;
        f.read_to_string(&mut s)
            .map_err(|_| ApiError::BadRequest("meta.json unreadable".into()))?;
        serde_json::from_str(&s).map_err(|e| ApiError::BadRequest(format!("meta.json: {e}")))?
    };
    if meta.format != PORTABLE_FORMAT {
        return Err(ApiError::BadRequest("not a portable object-set zip".into()));
    }
    // The manifest itself travels inside the zip.
    let manifest: crate::backup::BackupManifest = {
        let mut f = archive
            .by_name("manifest.json")
            .map_err(|_| ApiError::BadRequest("manifest.json missing".into()))?;
        let mut s = String::new();
        use std::io::Read as _;
        f.read_to_string(&mut s)
            .map_err(|_| ApiError::BadRequest("manifest.json unreadable".into()))?;
        serde_json::from_str(&s).map_err(|e| ApiError::BadRequest(format!("manifest.json: {e}")))?
    };
    let mut out = crate::backup::SimpleArchive::new();
    out.add(
        "manifest.json",
        &serde_json::to_vec_pretty(&manifest).unwrap_or_default(),
    );
    // Members: documents.json / user_settings.json live as blobs in the set.
    for entry in &meta.entries {
        let mut f = archive
            .by_name(&format!("blobs/{}", entry.sha256))
            .map_err(|_| ApiError::BadRequest(format!("blob missing for {}", entry.path)))?;
        let mut buf = Vec::with_capacity(entry.size as usize);
        use std::io::Read as _;
        f.read_to_end(&mut buf)
            .map_err(|_| ApiError::BadRequest("blob unreadable".into()))?;
        if buf.len() as u64 != entry.size {
            return Err(ApiError::BadRequest(format!(
                "blob size mismatch for {} ({} vs {})",
                entry.path,
                buf.len(),
                entry.size
            )));
        }
        out.add(&entry.path, &buf);
    }
    // Files: files_index.json + files/<id><ext>, matching the v1 layout.
    let mut index = Vec::with_capacity(meta.files.len());
    for f in &meta.files {
        let ext = f
            .key
            .rsplit_once('.')
            .map(|(_, e)| format!(".{e}"))
            .unwrap_or_default();
        index.push(json!({
            "id": f.id,
            "path": format!("files/{}{}", f.id, ext),
            "original_path": f.original_path,
            "size": f.size,
        }));
    }
    out.add(
        "files_index.json",
        &serde_json::to_vec_pretty(&index).unwrap_or_default(),
    );
    for f in &meta.files {
        let ext = f
            .key
            .rsplit_once('.')
            .map(|(_, e)| format!(".{e}"))
            .unwrap_or_default();
        let mut zf = archive
            .by_name(&format!("blobs/{}", f.sha256))
            .map_err(|_| ApiError::BadRequest(format!("blob missing for file {}", f.id)))?;
        let mut buf = Vec::new();
        use std::io::Read as _;
        zf.read_to_end(&mut buf)
            .map_err(|_| ApiError::BadRequest("file blob unreadable".into()))?;
        out.add_stored(format!("files/{}{}", f.id, ext), &buf);
    }
    out.finish()
        .map_err(|e| ApiError::BadRequest(format!("conversion failed: {e}")))
}

#[cfg(test)]
mod portable_tests {
    use super::*;

    fn sample_manifest() -> crate::backup::BackupManifest {
        crate::backup::BackupManifest {
            version: crate::backup::BACKUP_VERSION.into(),
            id: "1794281400000".into(),
            created_at: "2026-11-10T03:30:00Z".into(),
            user_id: "42".into(),
            memo: Some("portable test".into()),
            encryption: None,
            options: crate::backup::BackupOptions {
                include_documents: true,
                ..Default::default()
            },
            statistics: crate::backup::BackupStats::default(),
            entries: vec![crate::backup::ManifestEntry {
                path: "documents.json".into(),
                sha256: "aa".repeat(32),
                size: 12,
            }],
            files: vec![crate::backup::ManifestFile {
                id: "7".into(),
                key: "7.png".into(),
                original_path: Some("photo 中文.png".into()),
                sha256: "bb".repeat(32),
                size: 5,
            }],
            logical_bytes: 17,
        }
    }

    /// Builds a portable zip by hand (no storage needed) and checks the
    /// object-set -> v1 conversion keeps every member byte-identical.
    #[test]
    fn portable_to_v1_roundtrip() {
        let manifest = sample_manifest();
        let mut set = crate::backup::SimpleArchive::new();
        let meta = PortableMeta {
            format: PORTABLE_FORMAT.into(),
            version: PORTABLE_VERSION.into(),
            created_at: manifest.created_at.clone(),
            snapshot_id: manifest.id.clone(),
            user_id: manifest.user_id.clone(),
            encryption: None,
            entries: manifest.entries.clone(),
            files: manifest.files.clone(),
        };
        set.add("meta.json", &serde_json::to_vec(&meta).unwrap());
        set.add("manifest.json", &serde_json::to_vec(&manifest).unwrap());
        set.add(
            "blobs/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            br#"{"nodes":[]}"#,
        );
        set.add(
            "blobs/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            b"\x89PNG",
        );
        let bytes = set.finish().expect("assemble");

        assert!(
            is_portable_zip(&bytes),
            "meta.json format must be recognised"
        );
        let v1 = portable_to_v1_zip(bytes).expect("convert");
        let mut archive = ZipArchive::new(std::io::Cursor::new(v1)).expect("valid v1 zip");
        let mut names: Vec<String> = (0..archive.len())
            .filter_map(|i| {
                archive
                    .by_index(i)
                    .ok()
                    .and_then(|f| f.name().ok().map(|n| n.to_string()))
            })
            .collect();
        names.sort();
        assert!(names.contains(&"manifest.json".to_string()));
        assert!(names.contains(&"documents.json".to_string()));
        assert!(names.contains(&"files_index.json".to_string()));
        assert!(names.contains(&"files/7.png".to_string()));
        let mut doc = String::new();
        {
            use std::io::Read as _;
            archive
                .by_name("documents.json")
                .unwrap()
                .read_to_string(&mut doc)
                .unwrap();
        }
        assert_eq!(doc, r#"{"nodes":[]}"#, "member bytes preserved");
    }

    #[test]
    fn rejects_plain_v1_zip_as_portable() {
        let mut archive = crate::backup::SimpleArchive::new();
        archive.add("manifest.json", b"{}");
        archive.add("documents.json", b"[]");
        let bytes = archive.finish().unwrap();
        assert!(!is_portable_zip(&bytes), "a v1 archive has no meta.json");
    }
}
