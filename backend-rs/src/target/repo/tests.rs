//! Unit tests (split out: the 600-line cap counts production code + tests together).

use super::*;

fn input(over: impl FnOnce(&mut TargetInput)) -> TargetInput {
    let mut t = TargetInput {
        kind: "s3".into(),
        name: "t".into(),
        endpoint: "https://s3.example.com".into(),
        bucket: "bk".into(),
        prefix: "pergame".into(),
        access_key: "AK".into(),
        secret_key: "SK".into(),
        secure: true,
        region: "us-east-1".into(),
        provider: "Other".into(),
        keep_last: 10,
        max_age_days: 0,
        schedule: String::new(),
        enabled: true,
        users: vec![Uuid::nil()],
        namespace: String::new(),
        connection_token: None,
    };
    over(&mut t);
    t
}

#[test]
fn connection_token_ignores_display_and_retention() {
    let base = connection_token(&input(|_| {}));
    // Name / retention / schedule are not connection identity.
    let display_only = connection_token(&input(|t| {
        t.name = "renamed".into();
        t.keep_last = 3;
        t.schedule = "0 3 * * *".into();
        t.users = vec![Uuid::nil(), Uuid::max()];
    }));
    assert_eq!(base, display_only);
    // Connection fields do change it.
    let moved = connection_token(&input(|t| t.endpoint = "https://other.example".into()));
    assert_ne!(base, moved);
}

#[test]
fn connection_token_never_leaks_secrets() {
    let token = connection_token(&input(|t| {
        t.access_key = "SUPER-SECRET-AK".into();
        t.secret_key = "SUPER-SECRET-SK".into();
    }));
    assert!(!token.contains("SUPER"));
    assert_eq!(token.len(), 24, "12 bytes hex");
}

#[test]
fn namespace_is_server_allocated_ulid() {
    use crate::target::owner::new_namespace_id;
    let a = new_namespace_id();
    let b = new_namespace_id();
    assert_eq!(a.len(), 26, "ULID is 26 Crockford Base32 chars");
    assert_eq!(b.len(), 26);
    assert_ne!(a, b, "each target gets its own scope");
    for id in [&a, &b] {
        assert!(
            id.bytes().all(|c| {
                c.is_ascii_digit()
                    || (c.is_ascii_uppercase() && c != b'I' && c != b'L' && c != b'O' && c != b'U')
            }),
            "Crockford Base32 alphabet only: {id}"
        );
    }
    // Time-ordered prefix: two ids generated in sequence share the high
    // chars far more often than random ones would.
    assert_eq!(a[..6], b[..6], "timestamp prefix should be close");
}

#[test]
fn client_supplied_namespace_is_ignored() {
    // The input carries the field only so old clients don't break; the
    // row builder drops it and the caller assigns a ULID instead.
    let row = row_from_input(&input(|t| t.namespace = "user-typed".into()), 0, None);
    assert_eq!(row.namespace, "", "client value must not reach the row");
}

#[test]
fn mask_row_hides_secrets_but_keeps_shape() {
    let row = row_from_input(&input(|_| {}), 1_700_000_000_000, None);
    let masked = mask_row(&row);
    assert_eq!(masked["access_key"], MASK);
    assert_eq!(masked["secret_key"], MASK);
    assert_eq!(masked["endpoint"], "https://s3.example.com");
    assert_eq!(masked["bucket"], "bk");
    // Empty secrets stay empty (not masked): the form shows blank.
    let bare = row_from_input(
        &input(|t| {
            t.access_key = String::new();
            t.secret_key = String::new();
        }),
        0,
        None,
    );
    let masked_bare = mask_row(&bare);
    assert_eq!(masked_bare["access_key"], "");
    assert_eq!(masked_bare["secret_key"], "");
}

#[test]
fn masked_secret_rehydrates_previous() {
    assert_eq!(resolve_secret(MASK, "prev-ak"), "prev-ak");
    assert_eq!(resolve_secret("new-ak", "prev-ak"), "new-ak");
    assert_eq!(resolve_secret("", "prev-ak"), "");
}
