//! Plugin manifest parsing and validation (PS-D).
//!
//! The server is the authority: an uploaded archive only reaches storage once
//! its `manifest.json` passes these rules. The web client repeats the checks
//! before `import()`, but a hostile upload never gets that far.
//!
//! Rules mirror `frontend/app/services/plugins/manifest.ts` (PS-A):
//! slug id, semver, matching contract major, capability allow-list, an
//! entry that stays inside the plugin root (optional for themes-only
//! plugins), and validated `themes` declarations.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Host contract major version.
pub const HOST_API_VERSION: u32 = 1;

/// Capabilities the host knows how to enforce.
const STATIC_CAPABILITIES: [&str; 6] = [
    "nodes:read",
    "nodes:write",
    "ui",
    "markdown:render",
    "settings",
    "themes",
];

/// One manifest-declared app theme (pure CSS, no JS needed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginTheme {
    /// Theme slug, unique within the plugin (host qualifies it as `plugin:theme`).
    pub id: String,
    /// Display name in the theme list.
    pub name: String,
    /// Light-mode declaration file, relative to the plugin root.
    pub file: String,
    /// Optional dark-mode declaration file (light applies to both when absent).
    #[serde(rename = "darkFile", default, skip_serializing_if = "Option::is_none")]
    pub dark_file: Option<String>,
}

/// The 18 `--code-*` slots a palette must fill (mirrors code-hljs.scss).
pub const CODE_VAR_NAMES: [&str; 18] = [
    "bg", "fg", "comment", "keyword", "type", "literal", "attr", "number", "string", "function",
    "class", "variable", "operator", "section", "addition", "deletion", "meta", "tag",
];

/// One manifest-declared code palette (inline vars, no files needed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCodeTheme {
    /// Palette slug, unique within the plugin (host qualifies it as `plugin:code`).
    pub id: String,
    /// Display name in the palette list.
    pub name: String,
    /// All 19 code vars (`bg` → `--code-bg`, …).
    pub vars: std::collections::HashMap<String, String>,
}

/// Parsed, validated manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    /// Empty for themes-only plugins (no JS entry).
    pub entry: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Manifest-declared app themes (served via the asset route).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub themes: Vec<PluginTheme>,
    /// Manifest-declared code palettes (inline vars, no files).
    #[serde(rename = "codeThemes", default, skip_serializing_if = "Vec::is_empty")]
    pub code_themes: Vec<PluginCodeTheme>,
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
    /// Dock/list icon: sprite name, emoji, or image URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Declarative settings the UI renders; opaque to the server (the plugin
    /// reads the values back through its own KV namespace).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<serde_json::Value>,
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

/// A relative path with the given extension that stays inside the plugin root.
fn is_safe_asset(path: &str, ext: &str) -> bool {
    if !path.ends_with(ext) || path.contains('\\') {
        return false;
    }
    path.split('/').all(|segment| {
        !segment.is_empty()
            && segment != "."
            && segment != ".."
            && segment
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    })
}

/// A CSS var value that cannot break out of its declaration: short,
/// colors/functions only — no `;{}` (breakout), no `@` (at-rules), no
/// `<>` (markup), no quotes/backslash (string games), no `!` (!important).
fn is_safe_css_value(value: &str) -> bool {
    if value.is_empty() || value.len() > 64 {
        return false;
    }
    value.chars().all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, '#' | '(' | ')' | ',' | '.' | '%' | '/' | '+' | '-' | '*' | '_' | ' ')
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

    let mut themes: Vec<PluginTheme> = Vec::new();
    match raw.get("themes") {
        None | Some(Value::Null) => {}
        Some(Value::Array(items)) => {
            for (i, item) in items.iter().enumerate() {
                let tag = format!("themes[{i}]");
                let tid = string_field(item, "id").unwrap_or_default();
                if tid.is_empty() {
                    errors.push(format!("{tag}.id is required"));
                    continue;
                } else if !is_slug(&tid) {
                    errors.push(format!("{tag}.id must be a lowercase slug"));
                    continue;
                } else if themes.iter().any(|t: &PluginTheme| t.id == tid) {
                    errors.push(format!("{tag}.id '{tid}' is duplicated"));
                    continue;
                }
                let tname = string_field(item, "name").unwrap_or_default();
                if tname.is_empty() {
                    errors.push(format!("{tag}.name is required"));
                    continue;
                }
                let file = string_field(item, "file").unwrap_or_default();
                if file.is_empty() {
                    errors.push(format!("{tag}.file is required"));
                    continue;
                } else if !is_safe_asset(&file, ".css") {
                    errors.push(format!("{tag}.file must be a relative .css path inside the plugin"));
                    continue;
                }
                let dark_file = string_field(item, "darkFile");
                if let Some(d) = &dark_file
                    && !is_safe_asset(d, ".css")
                {
                    errors.push(format!(
                        "{tag}.darkFile must be a relative .css path inside the plugin"
                    ));
                    continue;
                }
                themes.push(PluginTheme {
                    id: tid,
                    name: tname,
                    file,
                    dark_file,
                });
            }
        }
        Some(_) => errors.push("themes must be an array".to_string()),
    }

    let mut code_themes: Vec<PluginCodeTheme> = Vec::new();
    match raw.get("codeThemes") {
        None | Some(Value::Null) => {}
        Some(Value::Array(items)) => {
            for (i, item) in items.iter().enumerate() {
                let tag = format!("codeThemes[{i}]");
                let tid = string_field(item, "id").unwrap_or_default();
                if tid.is_empty() {
                    errors.push(format!("{tag}.id is required"));
                    continue;
                } else if !is_slug(&tid) {
                    errors.push(format!("{tag}.id must be a lowercase slug"));
                    continue;
                } else if code_themes.iter().any(|t: &PluginCodeTheme| t.id == tid) {
                    errors.push(format!("{tag}.id '{tid}' is duplicated"));
                    continue;
                }
                let tname = string_field(item, "name").unwrap_or_default();
                if tname.is_empty() {
                    errors.push(format!("{tag}.name is required"));
                    continue;
                }
                let mut vars = std::collections::HashMap::new();
                let mut vars_ok = true;
                match item.get("vars") {
                    Some(Value::Object(map)) => {
                        for name in CODE_VAR_NAMES {
                            match map.get(name).and_then(Value::as_str).map(str::trim) {
                                Some(v) if !v.is_empty() => {
                                    if !is_safe_css_value(v) {
                                        errors.push(format!("{tag}.vars.{name} is not a safe CSS value"));
                                        vars_ok = false;
                                    } else {
                                        vars.insert(name.to_string(), v.to_string());
                                    }
                                }
                                _ => {
                                    errors.push(format!("{tag}.vars.{name} is required"));
                                    vars_ok = false;
                                }
                            }
                        }
                        for key in map.keys() {
                            if !CODE_VAR_NAMES.contains(&key.as_str()) {
                                errors.push(format!("{tag}.vars.{key} is not a known code var"));
                                vars_ok = false;
                            }
                        }
                    }
                    _ => {
                        errors.push(format!("{tag}.vars must be an object"));
                        vars_ok = false;
                    }
                }
                if !vars_ok {
                    continue;
                }
                code_themes.push(PluginCodeTheme {
                    id: tid,
                    name: tname,
                    vars,
                });
            }
        }
        Some(_) => errors.push("codeThemes must be an array".to_string()),
    }

    // Themes-only plugins ship no JS: entry may be omitted only then.
    let entry = match string_field(raw, "entry") {
        Some(e) => e,
        None if !themes.is_empty() || !code_themes.is_empty() => String::new(),
        None => "main.mjs".to_string(),
    };
    if !entry.is_empty() && !is_safe_asset(&entry, ".mjs") {
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
        themes,
        code_themes,
        min_host_version,
        author: string_field(raw, "author"),
        description: string_field(raw, "description"),
        icon: string_field(raw, "icon"),
        settings: raw.get("settings").filter(|v| !v.is_null()).cloned(),
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

    #[test]
    fn themes_only_manifest_needs_no_entry() {
        let m = validate(&json!({
            "id": "demo", "name": "D", "version": "0.1.0", "apiVersion": "1",
            "capabilities": ["themes"],
            "themes": [{"id": "mid", "name": "Mid", "file": "mid.css", "darkFile": "mid-dark.css"}]
        }))
        .expect("valid");
        assert!(m.entry.is_empty());
        assert_eq!(m.themes.len(), 1);
        assert_eq!(m.themes[0].dark_file.as_deref(), Some("mid-dark.css"));
    }

    #[test]
    fn theme_rules() {
        // missing name
        let err = validate(&json!({
            "id": "demo", "name": "D", "version": "0.1.0", "apiVersion": "1",
            "themes": [{"id": "mid", "file": "mid.css"}]
        }))
        .expect_err("must reject");
        assert!(err.iter().any(|e| e.contains("themes[0].name")), "{err:?}");
        // traversal + wrong extension
        for file in ["../evil.css", "a.css/nested.mjs", "x.js"] {
            let err = validate(&json!({
                "id": "demo", "name": "D", "version": "0.1.0", "apiVersion": "1",
                "themes": [{"id": "mid", "name": "M", "file": file}]
            }))
            .expect_err("must reject");
            assert!(err.iter().any(|e| e.contains("themes[0].file")), "{file}: {err:?}");
        }
        // duplicate theme id
        let err = validate(&json!({
            "id": "demo", "name": "D", "version": "0.1.0", "apiVersion": "1",
            "themes": [
                {"id": "mid", "name": "M", "file": "a.css"},
                {"id": "mid", "name": "M2", "file": "b.css"}
            ]
        }))
        .expect_err("must reject");
        assert!(err.iter().any(|e| e.contains("duplicated")), "{err:?}");
        // non-array
        let err = validate(&json!({
            "id": "demo", "name": "D", "version": "0.1.0", "apiVersion": "1",
            "themes": {"id": "mid"}
        }))
        .expect_err("must reject");
        assert!(err.iter().any(|e| e.contains("themes must be an array")), "{err:?}");
    }

    fn full_code_vars() -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for name in CODE_VAR_NAMES {
            map.insert(name.to_string(), serde_json::Value::String("#112233".to_string()));
        }
        serde_json::Value::Object(map)
    }

    #[test]
    fn code_themes_only_manifest_needs_no_entry() {
        let m = validate(&json!({
            "id": "demo", "name": "D", "version": "0.1.0", "apiVersion": "1",
            "capabilities": ["themes"],
            "codeThemes": [{"id": "rose", "name": "Rose", "vars": full_code_vars()}]
        }))
        .expect("valid");
        assert!(m.entry.is_empty());
        assert_eq!(m.code_themes.len(), 1);
        assert_eq!(m.code_themes[0].vars.len(), 18);
    }

    #[test]
    fn code_theme_rules() {
        // missing var
        let mut vars = full_code_vars();
        vars.as_object_mut().unwrap().remove("bg");
        let err = validate(&json!({
            "id": "demo", "name": "D", "version": "0.1.0", "apiVersion": "1",
            "codeThemes": [{"id": "rose", "name": "R", "vars": vars}]
        }))
        .expect_err("must reject");
        assert!(err.iter().any(|e| e.contains("codeThemes[0].vars.bg is required")), "{err:?}");
        // unknown var
        let mut vars = full_code_vars();
        vars.as_object_mut().unwrap().insert("evil".to_string(), json!("#fff"));
        let err = validate(&json!({
            "id": "demo", "name": "D", "version": "0.1.0", "apiVersion": "1",
            "codeThemes": [{"id": "rose", "name": "R", "vars": vars}]
        }))
        .expect_err("must reject");
        assert!(err.iter().any(|e| e.contains("not a known code var")), "{err:?}");
        // declaration breakout attempts
        for evil in [
            "#fff; --x: red",
            "red } html { display: none",
            "url(http://evil.test/x)",
            "red !important",
        ] {
            let mut vars = full_code_vars();
            vars.as_object_mut().unwrap().insert("fg".to_string(), json!(evil));
            let err = validate(&json!({
                "id": "demo", "name": "D", "version": "0.1.0", "apiVersion": "1",
                "codeThemes": [{"id": "rose", "name": "R", "vars": vars}]
            }))
            .expect_err("must reject");
            assert!(err.iter().any(|e| e.contains("not a safe CSS value")), "{evil}: {err:?}");
        }
        // functional color notations pass
        for ok in ["#abc", "#aabbcc", "#aabbccdd", "rgb(1, 2, 3)", "rgba(1,2,3,0.5)", "hsl(0 0% 0% / 50%)", "red"] {
            let mut vars = full_code_vars();
            vars.as_object_mut().unwrap().insert("fg".to_string(), json!(ok));
            validate(&json!({
                "id": "demo", "name": "D", "version": "0.1.0", "apiVersion": "1",
                "codeThemes": [{"id": "rose", "name": "R", "vars": vars}]
            }))
            .expect("valid");
        }
    }
}
