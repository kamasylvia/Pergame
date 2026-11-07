//! Shared helpers.

/// HTML sanitization matching Go `utils.EscapeHTML` (bluemonday UGCPolicy):
/// nil/empty input → empty string; otherwise strip everything outside the UGC
/// allow-list. Rust side uses ammonia's default builder, which covers the same
/// UGC tag set. Known delta vs bluemonday: `data-*` attributes are stripped
/// (bluemonday allowed them); revisit if the frontend relies on embedded
/// data attributes in compiled content.
pub fn sanitize_html(input: Option<&str>) -> String {
    match input {
        Some(s) if !s.is_empty() => ammonia::clean(s),
        _ => String::new(),
    }
}
