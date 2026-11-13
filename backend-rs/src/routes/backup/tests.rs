//! Unit tests (split out: the 600-line cap counts production code + tests together).

use super::*;

#[test]
fn options_trims_and_caps_memo() {
    let body = BackupBody {
        include_documents: true,
        include_files: false,
        include_metadata: true,
        include_settings: false,
        memo: Some("  hi  ".into()),
        ..Default::default()
    };
    let opts = options_from(body);
    assert!(opts.include_documents);
    assert!(!opts.include_files);
    assert_eq!(opts.memo.as_deref(), Some("hi"));
    let blank = options_from(BackupBody {
        memo: Some("   ".into()),
        ..Default::default()
    });
    assert_eq!(blank.memo, None);
    let long = options_from(BackupBody {
        memo: Some("x".repeat(500)),
        ..Default::default()
    });
    assert_eq!(long.memo.as_ref().unwrap().chars().count(), 200);
}
