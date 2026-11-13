//! Plugin manifest parsing and validation (PS-D).
//!
//! The server is the authority: an uploaded archive only reaches storage once
//! its `manifest.json` passes these rules. The web client repeats the checks
//! before `import()`, but a hostile upload never gets that far.
//!
//! Rules mirror `frontend/app/services/plugins/manifest.ts` (PS-A):
//! slug id, semver, matching contract major, capability allow-list, and an
//! entry that stays inside the plugin root.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Host contract major version.
pub const HOST_API_VERSION: u32 = 1;

/// Capabilities the host knows how to enforce.
const STATIC_CAPABILITIES: [&str; 5] = [
    "nodes:read",
    "nodes:write",
    "ui",
    "markdown:render",
    "settings",
];

/// Parsed, validated manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub entry: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(
        rename = "minHostVersion",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub min_host_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn is_slug(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.is_empty() || bytes.len() > 64 {
        return false;
    }
    if !bytes[0].is_ascii_lowercase() && !bytes[0].is_ascii_digit() {
        return false;
    }
    bytes
        .iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

fn is_semver(version: &str) -> bool {
    let core = version.split(['-', '+']).next().unwrap_or("");
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// A relative `.mjs` path that stays inside the plugin root.
fn is_safe_entry(entry: &str) -> bool {
    if !entry.ends_with(".mjs") || entry.contains('\\') {
        return false;
    }
    entry.split('/').all(|segment| {
        !segment.is_empty()
            && segment != "."
            && segment != ".."
            && segment
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    })
}

/// `net:<domain>` / `net:*` or one of the static capabilities.
fn is_known_capability(cap: &str) -> bool {
    if STATIC_CAPABILITIES.contains(&cap) {
        return true;
    }
    let Some(scope) = cap.strip_prefix("net:") else {
        return false;
    };
    if scope == "*" {
        return true;
    }
    // IP literals are refused: a granted address can change owner, and the
    // proxy's SSRF guard is easier to reason about with names only.
    if scope.parse::<std::net::IpAddr>().is_ok() {
        return false;
    }
    // Host name: at least two labels, each `[a-z0-9-]` and not starting/ending
    // with a dash. IP literals are rejected on purpose (no pinning to a host
    // that can change owner).
    let labels: Vec<&str> = scope.split('.').collect();
    labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                && !label.starts_with('-')
                && !label.ends_with('-')
        })
}

/// True when any `net:` capability of `capabilities` covers `url`'s host.
///
/// Used by both the browser facade and the server-side proxy: the same rule
/// decides what a plugin may fetch, so the two paths cannot drift.
pub fn net_allows(capabilities: &[String], url: &url::Url) -> bool {
    if url.scheme() != "http" && url.scheme() != "https" {
        return false;
    }
    let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
        return false;
    };
    capabilities.iter().any(|cap| {
        let Some(scope) = cap.strip_prefix("net:") else {
            return false;
        };
        scope == "*" || host == scope || host.ends_with(&format!(".{scope}"))
    })
}

/// Validate a raw manifest object. Every problem is reported at once.
pub fn validate(raw: &Value) -> Result<PluginManifest, Vec<String>> {
    let mut errors = Vec::new();

    let id = string_field(raw, "id").unwrap_or_default();
    if id.is_empty() {
        errors.push("id is required".to_string());
    } else if !is_slug(&id) {
        errors.push("id must be a lowercase slug (a-z, 0-9, dashes, max 64)".to_string());
    }

    let name = string_field(raw, "name").unwrap_or_default();
    if name.is_empty() {
        errors.push("name is required".to_string());
    }

    let version = string_field(raw, "version").unwrap_or_default();
    if version.is_empty() {
        errors.push("version is required".to_string());
    } else if !is_semver(&version) {
        errors.push("version must be semver (x.y.z)".to_string());
    }

    let api_version = string_field(raw, "apiVersion").unwrap_or_default();
    if api_version.is_empty() {
        errors.push("apiVersion is required".to_string());
    } else if api_version.parse::<u32>().ok() != Some(HOST_API_VERSION) {
        errors.push(format!(
            "apiVersion {api_version} is not supported (host contract is {HOST_API_VERSION})"
        ));
    }

    let entry = string_field(raw, "entry").unwrap_or_else(|| "main.mjs".to_string());
    if !is_safe_entry(&entry) {
        errors.push("entry must be a relative .mjs path inside the plugin".to_string());
    }

    let mut capabilities: Vec<String> = Vec::new();
    match raw.get("capabilities") {
        None | Some(Value::Null) => {}
        Some(Value::Array(items)) => {
            for item in items {
                let Some(cap) = item.as_str().map(str::trim).filter(|c| !c.is_empty()) else {
                    errors.push("capabilities must contain strings".to_string());
                    continue;
                };
                if !is_known_capability(cap) {
                    errors.push(format!("unknown capability '{cap}'"));
                } else if !capabilities.iter().any(|c| c == cap) {
                    capabilities.push(cap.to_string());
                }
            }
        }
        Some(_) => errors.push("capabilities must be an array".to_string()),
    }

    let min_host_version = string_field(raw, "minHostVersion");
    if let Some(v) = &min_host_version
        && !is_semver(v)
    {
        errors.push("minHostVersion must be semver (x.y.z)".to_string());
    }

    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(PluginManifest {
        id,
        name,
        version,
        api_version,
        entry,
        capabilities,
        min_host_version,
        author: string_field(raw, "author"),
        description: string_field(raw, "description"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_a_minimal_manifest() {
        let m =
            validate(&json!({"id": "demo", "name": "Demo", "version": "0.1.0", "apiVersion": "1"}))
                .expect("valid");
        assert_eq!(m.entry, "main.mjs");
        assert!(m.capabilities.is_empty());
    }

    #[test]
    fn rejects_a_foreign_contract_major() {
        let err =
            validate(&json!({"id": "demo", "name": "D", "version": "0.1.0", "apiVersion": "2"}))
                .expect_err("must reject");
        assert!(err[0].contains("apiVersion 2"));
    }

    #[test]
    fn rejects_path_traversal_in_entry() {
        for entry in [
            "../evil.mjs",
            "a/../../b.mjs",
            "/abs.mjs",
            "a\\b.mjs",
            "main.js",
        ] {
            let err = validate(&json!({
                "id": "demo", "name": "D", "version": "0.1.0", "apiVersion": "1", "entry": entry
            }))
            .expect_err("must reject");
            assert!(err.iter().any(|e| e.contains("entry")), "{entry}: {err:?}");
        }
    }

    #[test]
    fn capability_rules() {
        let ok = validate(&json!({
            "id": "demo", "name": "D", "version": "0.1.0", "apiVersion": "1",
            "capabilities": ["ui", "net:example.com", "net:*", "ui"]
        }))
        .expect("valid");
        assert_eq!(ok.capabilities.len(), 3, "duplicates collapse");

        for cap in [
            "root",
            "net:localhost",
            "net:127.0.0.1",
            "net:-bad.com",
            "net:",
        ] {
            let err = validate(&json!({
                "id": "demo", "name": "D", "version": "0.1.0", "apiVersion": "1",
                "capabilities": [cap]
            }))
            .expect_err("must reject");
            assert!(err[0].contains("unknown capability"), "{cap}");
        }
    }

    #[test]
    fn reports_every_problem() {
        let err = validate(&json!({"id": "Bad Id", "version": "1", "apiVersion": "9"}))
            .expect_err("invalid");
        assert!(err.len() >= 4, "{err:?}");
    }

    #[test]
    fn net_scope_matches_host_and_subdomains() {
        let caps = vec!["net:example.com".to_string()];
        for url in ["https://example.com/a", "https://api.example.com/x"] {
            assert!(net_allows(&caps, &url::Url::parse(url).unwrap()), "{url}");
        }
        for url in [
            "https://evil.test/",
            "https://example.com.evil.test/",
            "file:///etc/passwd",
        ] {
            assert!(!net_allows(&caps, &url::Url::parse(url).unwrap()), "{url}");
        }
        assert!(net_allows(
            &["net:*".to_string()],
            &url::Url::parse("https://anything.test/").unwrap()
        ));
    }

    #[test]
    fn slug_rules() {
        assert!(is_slug("demo"));
        assert!(is_slug("demo-2"));
        assert!(!is_slug("Demo"));
        assert!(!is_slug("-demo"));
        assert!(!is_slug("demo_2"));
        assert!(!is_slug(&"a".repeat(65)));
    }
}
