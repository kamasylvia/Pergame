//! Generic image-host adapter (CloudFlare-ImgBed / Lsky Pro / Chevereto …).
//!
//! Configured through `[Imgbed]` / `IMGBED_*`; empty endpoint or token means
//! disabled. Uploads are multipart (`Field`, default `file`) with a bearer
//! header; the response is parsed by `ResponsePath` (dot path, array indexes
//! allowed) or accepted as plain text. `UrlTemplate` maps the returned `src`
//! to the public URL (`{src}` / `{endpoint}` placeholders); when empty, an
//! absolute `src` is used as-is and a leading `/` is joined to the endpoint
//! origin.

use serde_json::Value;

use crate::error::ApiError;

#[derive(Debug, Clone)]
pub struct Imgbed {
    pub endpoint: String,
    pub token: String,
    pub field: String,
    pub auth_header: String,
    pub auth_prefix: String,
    pub response_path: String,
    pub url_template: String,
    client: reqwest::Client,
}

impl Imgbed {
    /// Build from the merged config; `None` when disabled or incomplete.
    pub fn from_config() -> Option<Self> {
        let cfg = &crate::config::global().imgbed;
        if !cfg.enabled || cfg.endpoint.trim().is_empty() || cfg.token.trim().is_empty() {
            return None;
        }
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .ok()?;
        Some(Self {
            endpoint: cfg.endpoint.clone(),
            token: cfg.token.clone(),
            field: cfg.field.clone(),
            auth_header: cfg.auth_header.clone(),
            auth_prefix: cfg.auth_prefix.clone(),
            response_path: cfg.response_path.clone(),
            url_template: cfg.url_template.clone(),
            client,
        })
    }

    /// Upload one file, returning the resolved public URL.
    pub async fn upload(
        &self,
        bytes: &[u8],
        filename: &str,
        mime: &str,
    ) -> Result<String, ApiError> {
        let part = reqwest::multipart::Part::bytes(bytes.to_vec())
            .file_name(filename.to_string())
            .mime_str(mime)
            .map_err(|e| ApiError::BadRequest(format!("invalid image-host mime: {e}")))?;
        let form = reqwest::multipart::Form::new().part(self.field.clone(), part);
        let mut req = self.client.post(&self.endpoint).multipart(form);
        if !self.token.is_empty() {
            req = req.header(
                &self.auth_header,
                format!("{}{}", self.auth_prefix, self.token),
            );
        }
        let resp = req.send().await.map_err(|e| {
            tracing::warn!(error = %e, "image-host upload request failed");
            ApiError::BadRequest(format!("image-host upload failed: {e}"))
        })?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            tracing::warn!(%status, body = %body, "image-host upload rejected");
            return Err(ApiError::BadRequest(format!(
                "image-host upload rejected: HTTP {status}"
            )));
        }
        let src = parse_src(&body, &self.response_path)
            .ok_or_else(|| ApiError::BadRequest("image-host response has no URL".into()))?;
        Ok(self.resolve(&src))
    }

    /// Fetch an object (Range forwarded when given) for the authed proxy read.
    pub async fn fetch(
        &self,
        url: &str,
        range: Option<&str>,
    ) -> Result<reqwest::Response, ApiError> {
        let mut req = self.client.get(url);
        if let Some(r) = range {
            req = req.header(reqwest::header::RANGE, r);
        }
        if !self.token.is_empty() {
            req = req.header(
                &self.auth_header,
                format!("{}{}", self.auth_prefix, self.token),
            );
        }
        req.send().await.map_err(|e| {
            tracing::warn!(error = %e, url = %url, "image-host fetch failed");
            ApiError::BadRequest(format!("image-host fetch failed: {e}"))
        })
    }

    fn resolve(&self, src: &str) -> String {
        let src = src.trim();
        if src.starts_with("http://") || src.starts_with("https://") {
            return src.to_string();
        }
        if !self.url_template.is_empty() {
            return self
                .url_template
                .replace("{src}", src)
                .replace("{endpoint}", self.endpoint.trim_end_matches('/'));
        }
        if let Some(rest) = src.strip_prefix('/')
            && let Ok(base) = url::Url::parse(&self.endpoint)
            && let Some(host) = base.host_str()
        {
            let port = base.port().map(|p| format!(":{p}")).unwrap_or_default();
            return format!("{}://{}{}/{}", base.scheme(), host, port, rest);
        }
        format!(
            "{}/{}",
            self.endpoint.trim_end_matches('/'),
            src.trim_start_matches('/')
        )
    }
}

/// Parse `path` (e.g. `0.src`, `data.links.url`) out of a JSON body; plain
/// text bodies that look like a URL are accepted too.
fn parse_src(body: &str, path: &str) -> Option<String> {
    let text = body.trim();
    if let Ok(v) = serde_json::from_str::<Value>(text) {
        let candidates: &[&str] = if path.is_empty() {
            &["src", "url", "0.src", "0.url", "data.url", "data.links.url"]
        } else {
            &[path]
        };
        for p in candidates {
            if let Some(v) = json_path(&v, p)
                && let Some(s) = v.as_str()
                && !s.is_empty()
            {
                return Some(s.to_string());
            }
        }
        return None;
    }
    if text.starts_with("http://") || text.starts_with("https://") || text.starts_with('/') {
        return Some(text.to_string());
    }
    None
}

fn json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = value;
    for part in path.split('.') {
        cur = if let Ok(idx) = part.parse::<usize>() {
            cur.get(idx)?
        } else {
            cur.get(part)?
        };
    }
    Some(cur)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_array_and_object_paths() {
        assert_eq!(
            parse_src(r#"[{"src":"/file/a.png"}]"#, "0.src").as_deref(),
            Some("/file/a.png")
        );
        assert_eq!(
            parse_src(r#"{"data":{"links":{"url":"https://x/y.png"}}}"#, "data.links.url")
                .as_deref(),
            Some("https://x/y.png")
        );
        assert_eq!(
            parse_src(r#"{"src":"/file/a.png"}"#, "").as_deref(),
            Some("/file/a.png")
        );
        assert_eq!(parse_src("not json", ""), None);
    }

    #[test]
    fn resolves_src_to_public_url() {
        let mk = |template: &str| Imgbed {
            endpoint: "https://img.example.com/upload".into(),
            token: "t".into(),
            field: "file".into(),
            auth_header: "Authorization".into(),
            auth_prefix: "Bearer ".into(),
            response_path: String::new(),
            url_template: template.into(),
            client: reqwest::Client::new(),
        };
        assert_eq!(
            mk("{endpoint}{src}").resolve("/file/a.png"),
            "https://img.example.com/upload/file/a.png"
        );
        assert_eq!(
            mk("").resolve("/file/a.png"),
            "https://img.example.com/file/a.png"
        );
        assert_eq!(
            mk("").resolve("https://cdn.example/a.png"),
            "https://cdn.example/a.png"
        );
    }
}
