//! Request-context extractor: client IP + user agent (Go `c.ClientIP()` /
//! `c.Request.UserAgent()` parity for the auth domain).

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct RequestCtx {
    pub ip: String,
    pub user_agent: String,
}

/// Resolves the client IP: first `X-Forwarded-For` entry, then `X-Real-IP`,
/// then the peer socket address, else `"unknown"`.
pub fn parse_client_ip(
    headers: &axum::http::HeaderMap,
    connect_info: Option<SocketAddr>,
) -> String {
    if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok())
        && let Some(first) = xff.split(',').next() {
            let candidate = first.trim();
            if !candidate.is_empty() {
                return candidate.to_string();
            }
        }
    if let Some(xri) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        let candidate = xri.trim();
        if !candidate.is_empty() {
            return candidate.to_string();
        }
    }
    if let Some(addr) = connect_info {
        return addr.ip().to_string();
    }
    "unknown".into()
}

impl FromRequestParts<crate::AppState> for RequestCtx {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &crate::AppState,
    ) -> Result<Self, Self::Rejection> {
        let user_agent = parts
            .headers
            .get(axum::http::header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let connect_info = parts
            .extensions
            .get::<axum::extract::ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0);
        let ip = parse_client_ip(&parts.headers, connect_info);
        Ok(Self { ip, user_agent })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn xff_first_entry_wins() {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "203.0.113.7, 70.41.3.18".parse().unwrap());
        assert_eq!(parse_client_ip(&h, None), "203.0.113.7");
    }

    #[test]
    fn xff_empty_falls_to_real_ip() {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", " , ".parse().unwrap());
        h.insert("x-real-ip", "198.51.100.2".parse().unwrap());
        assert_eq!(parse_client_ip(&h, None), "198.51.100.2");
    }

    #[test]
    fn falls_back_to_peer_then_unknown() {
        let h = HeaderMap::new();
        assert_eq!(
            parse_client_ip(&h, Some("10.1.2.3:5555".parse().unwrap())),
            "10.1.2.3"
        );
        assert_eq!(parse_client_ip(&h, None), "unknown");
    }
}
