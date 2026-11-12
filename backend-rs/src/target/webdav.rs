//! WebDAV fan-out target: minimal DAV client over reqwest (PUT/GET/DELETE/
//! PROPFIND depth-1/MKCOL). Paths live under the configured prefix; missing
//! collections are MKCOL'd on first write.

use super::{Fingerprint, ObjectRow, WebdavStoreParams, internal, join_prefix};

pub struct WebdavTarget {
    url: String, // base collection URL, no trailing slash
    auth: String,
    prefix: String,
    http: reqwest::Client,
}

/// Extracts the percent-decoded last path segment of a DAV href.
fn href_tail(href: &str) -> String {
    let decoded = percent_decode(href);
    let trimmed = decoded.trim_end_matches('/');
    trimmed.rsplit('/').next().unwrap_or("").to_string()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() + 1 && i + 2 <= bytes.len() {
            let hex = &s[i + 1..i + 3.min(s.len())];
            if hex.len() == 2
                && let Ok(v) = u8::from_str_radix(hex, 16)
            {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

impl WebdavTarget {
    pub fn new(p: &WebdavStoreParams) -> Self {
        let url = p.url.trim().trim_end_matches('/').to_string();
        Self {
            url,
            auth: format!(
                "Basic {}",
                base64_encode(format!("{}:{}", p.username, p.password).as_bytes())
            ),
            prefix: p.prefix.trim().trim_matches('/').to_string(),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("dav http client"),
        }
    }

    fn path(&self, path: &str) -> String {
        join_prefix(&self.prefix, path)
    }

    fn url_for(&self, path: &str) -> String {
        format!("{}/{}", self.url, self.path(path))
    }

    fn request(&self, method: reqwest::Method, url: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, url)
            .header("Authorization", &self.auth)
    }

    /// Ensures every collection along `path` exists (sequential MKCOL, errors
    /// on already-exists are ignored).
    async fn mkcol_deep(&self, path: &str) -> Result<(), crate::error::ApiError> {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let mut acc = String::new();
        for part in &parts {
            acc = format!("{acc}/{part}");
            let url = format!("{}/{}", self.url, acc.trim_start_matches('/'));
            let resp = self
                .request(
                    reqwest::Method::from_bytes(b"MKCOL").expect("valid method"),
                    &url,
                )
                .send()
                .await
                .map_err(|e| internal(format!("webdav mkcol {acc}: {e}")))?;
            // 2xx = created; 405/409 = already exists (server-dependent);
            // anything else (401/403/507/…) is a real failure.
            let code = resp.status().as_u16();
            if !(resp.status().is_success() || code == 405 || code == 409) {
                return Err(internal(format!("webdav mkcol {acc}: status {code}")));
            }
        }
        Ok(())
    }

    /// PROPFIND depth-1 collection existence/contents probe. Parses only the
    /// href segments — the backup object keys are pure ASCII by design.
    pub async fn list(&self, path: &str) -> Result<Vec<ObjectRow>, crate::error::ApiError> {
        let url = self.url_for(path);
        let resp = self
            .request(reqwest::Method::from_bytes(b"PROPFIND").expect("valid method"), &url)
            .header("Depth", "1")
            .header("Content-Type", "application/xml")
            .body(r#"<?xml version="1.0"?><d:propfind xmlns:d="DAV:"><d:prop><d:getcontentlength/><d:getlastmodified/><d:resourcetype/></d:prop></d:propfind>"#)
            .send()
            .await
            .map_err(|e| internal(format!("webdav propfind {path}: {e}")))?;
        if resp.status().as_u16() == 404 {
            return Ok(Vec::new());
        }
        if !resp.status().is_success() {
            return Err(internal(format!(
                "webdav propfind {path}: status {}",
                resp.status()
            )));
        }
        let body = resp.text().await.map_err(internal)?;
        let mut out = Vec::new();
        for chunk in body.split('<').skip(1) {
            let Some((tag, rest)) = chunk.split_once('>') else {
                continue;
            };
            let tag = tag.rsplit(':').next().unwrap_or(tag);
            if tag != "href" {
                continue;
            }
            let href = rest.split('<').next().unwrap_or("");
            let tail = href_tail(href);
            if tail.is_empty() || tail == path.rsplit('/').next().unwrap_or("") {
                continue; // the collection itself
            }
            // Size/mtime are parsed leniently: 0 when the server omits them.
            let size = rest
                .split('<')
                .find_map(|seg| seg.strip_prefix("getcontentlength"))
                .and_then(|seg| seg.split_once('>'))
                .and_then(|(_, v)| v.split('<').next().and_then(|v| v.parse::<i64>().ok()))
                .unwrap_or(0);
            out.push((tail, size, 0));
        }
        Ok(out)
    }

    /// Fingerprint anchor content: a marker object under the prefix whose
    /// bytes hash to the instance identity (written once at first fan-out).
    pub async fn has_marker(&self, path: &str) -> Result<bool, crate::error::ApiError> {
        self.has(path).await
    }

    pub async fn has(&self, path: &str) -> Result<bool, crate::error::ApiError> {
        let url = self.url_for(path);
        let resp = self
            .request(reqwest::Method::HEAD, &url)
            .send()
            .await
            .map_err(internal)?;
        Ok(resp.status().is_success())
    }

    pub async fn put(&self, path: &str, bytes: Vec<u8>) -> Result<(), crate::error::ApiError> {
        // ensure parent collections (mkdir -p semantics)
        let dir = self.path(path);
        let dir = match dir.rfind('/') {
            Some(i) => &dir[..i],
            None => "",
        };
        if !dir.is_empty() {
            self.mkcol_deep(dir).await?;
        }
        let resp = self
            .request(reqwest::Method::PUT, &self.url_for(path))
            .body(bytes)
            .send()
            .await
            .map_err(|e| internal(format!("webdav put {path}: {e}")))?;
        if !resp.status().is_success() {
            return Err(internal(format!(
                "webdav put {path}: status {}",
                resp.status()
            )));
        }
        Ok(())
    }

    pub async fn get(&self, path: &str) -> Result<Vec<u8>, crate::error::ApiError> {
        let resp = self
            .request(reqwest::Method::GET, &self.url_for(path))
            .send()
            .await
            .map_err(|e| internal(format!("webdav get {path}: {e}")))?;
        if resp.status().as_u16() == 404 {
            return Err(crate::error::ApiError::NotFound("object not found".into()));
        }
        if !resp.status().is_success() {
            return Err(internal(format!(
                "webdav get {path}: status {}",
                resp.status()
            )));
        }
        let bytes = resp.bytes().await.map_err(internal)?;
        Ok(bytes.to_vec())
    }

    pub async fn delete(&self, path: &str) -> Result<bool, crate::error::ApiError> {
        let resp = self
            .request(reqwest::Method::DELETE, &self.url_for(path))
            .send()
            .await
            .map_err(|e| internal(format!("webdav delete {path}: {e}")))?;
        Ok(resp.status().is_success() || resp.status().as_u16() == 404)
    }
}

pub fn fingerprint(p: &WebdavStoreParams, claimed_hash: &str) -> Fingerprint {
    Fingerprint(super::webdav_identity(&p.url, claimed_hash))
}

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}
