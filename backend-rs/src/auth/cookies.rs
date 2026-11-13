//! Auth cookies: `Authorization` (access JWT) + `RefreshToken`, path `/`,
//! httpOnly, domain from `COOKIE_DOMAIN` (leading dot added for subdomains,
//! Go parity), secure unless `ALLOW_UNSECURE` is true/1.

use cookie::Cookie;

/// Access-JWT cookie name.
pub const ACCESS_COOKIE: &str = "Authorization";
/// Refresh-token cookie name.
pub const REFRESH_COOKIE: &str = "RefreshToken";

/// True unless `ALLOW_UNSECURE=true`/`1` (dev over plain HTTP).
pub fn secure_cookies() -> bool {
    !matches!(
        std::env::var("ALLOW_UNSECURE").as_deref(),
        Ok("true") | Ok("1")
    )
}

/// Sanitized `COOKIE_DOMAIN` value, or None for host-only cookies.
pub fn cookie_domain() -> Option<String> {
    let raw = std::env::var("COOKIE_DOMAIN").ok()?;
    let cleaned = sanitize_cookie_domain(&raw);
    if cleaned.is_empty() {
        return None;
    }
    // Include a leading dot only for real multi-label domains (subdomain
    // sharing, Go parity). A dotted single-label host — `.localhost`, `.::1`,
    // any bare IP — is rejected by cookie-domain matching, so the cookie would
    // be stored and then never sent back: every request 401s.
    Some(if needs_leading_dot(&cleaned) {
        format!(".{cleaned}")
    } else {
        cleaned.trim_start_matches('.').to_string()
    })
}

/// True for domains that can be scoped to subdomains: at least two labels and
/// not an IP literal. Everything else must be sent host-only.
fn needs_leading_dot(domain: &str) -> bool {
    let d = domain.trim_start_matches('.');
    if d.is_empty() || d.starts_with('[') || d.starts_with("::") {
        return false; // IPv6
    }
    // IPv4: all labels numeric.
    if d.split('.')
        .all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_digit()))
    {
        return false;
    }
    // Two or more non-empty labels: "a-." / "a..b" are malformed and stay
    // host-only rather than gaining a scope that never matches.
    let mut non_empty = 0;
    for label in d.split('.') {
        if label.is_empty() {
            return false;
        }
        non_empty += 1;
    }
    non_empty >= 2
}

/// A `Domain` attribute may not carry a scheme or a port: browsers and HTTP
/// clients silently DROP such a cookie, which turns the entire session into
/// 401s with no error anywhere. Strip both instead of trusting the operator
/// to have typed a bare host (`COOKIE_DOMAIN=localhost:8200` was the shipped
/// compose default for a while and broke every authenticated request).
fn sanitize_cookie_domain(raw: &str) -> String {
    let mut d = raw.trim();
    if let Some(rest) = d.split_once("://").map(|(_, r)| r) {
        d = rest; // drop the scheme
    }
    // Drop everything from the first '/', '?' or '#' onwards.
    d = d.split(['/', '?', '#']).next().unwrap_or(d);
    // Drop the port (IPv6 literals keep their brackets: [::1]:8200).
    let without_port = if d.starts_with('[') {
        match d.find(']') {
            Some(end) => &d[..=end],
            None => d,
        }
    } else {
        d.split(':').next().unwrap_or(d)
    };
    without_port.trim().trim_end_matches('.').to_string()
}

fn build(name: &'static str, value: String, max_age_secs: i64) -> Cookie<'static> {
    let mut cookie = Cookie::build((name, value))
        .path("/")
        .http_only(true)
        .secure(secure_cookies());
    if let Some(domain) = cookie_domain() {
        cookie = cookie.domain(domain);
    }
    if max_age_secs >= 0 {
        cookie = cookie.max_age(time::Duration::seconds(max_age_secs));
    } else {
        cookie = cookie.max_age(time::Duration::ZERO);
    }
    cookie.build()
}

/// Access-token cookie with the given lifetime.
pub fn access_cookie(token: &str, max_age_secs: i64) -> Cookie<'static> {
    build(ACCESS_COOKIE, token.to_string(), max_age_secs)
}

/// Refresh-token cookie with the given lifetime.
pub fn refresh_cookie(token: &str, max_age_secs: i64) -> Cookie<'static> {
    build(REFRESH_COOKIE, token.to_string(), max_age_secs)
}

/// Expired access cookie (logout).
pub fn clear_access_cookie() -> Cookie<'static> {
    build(ACCESS_COOKIE, String::new(), -1)
}

/// Expired refresh cookie (logout).
pub fn clear_refresh_cookie() -> Cookie<'static> {
    build(REFRESH_COOKIE, String::new(), -1)
}

#[cfg(test)]
#[allow(unsafe_code)] // Test-only env mutation, serialized by ENV_LOCK.
mod tests {
    use super::*;
    use std::sync::Mutex;

    pub(super) static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn set_env(key: &str, value: &str) {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::set_var(key, value);
        }
    }

    fn remove_env(key: &str) {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn domain_gets_leading_dot() {
        set_env("COOKIE_DOMAIN", "example.com");
        let jar_cookie = access_cookie("t", 60);
        // cookie crate 0.18 normalizes the leading dot away (RFC 6265 §5.2.3 —
        // ".example.com" and "example.com" are equivalent for browsers).
        assert_eq!(jar_cookie.domain(), Some("example.com"));
        remove_env("COOKIE_DOMAIN");
    }

    #[test]
    fn secure_follows_allow_unsecure() {
        set_env("ALLOW_UNSECURE", "true");
        assert!(!secure_cookies());
        set_env("ALLOW_UNSECURE", "1");
        assert!(!secure_cookies());
        remove_env("ALLOW_UNSECURE");
        assert!(secure_cookies());
    }

    #[test]
    fn clear_cookie_has_zero_max_age() {
        let c = clear_access_cookie();
        assert_eq!(c.max_age(), Some(time::Duration::ZERO));
        assert_eq!(c.value(), "");
    }
}

#[cfg(test)]
#[allow(unsafe_code)] // Test-only env mutation, serialized by ENV_LOCK.
mod domain_proptests {
    use proptest::prelude::*;

    use super::{cookie_domain, needs_leading_dot, sanitize_cookie_domain};

    proptest! {
        /// Sanitization never emits a scheme, port, path, or trailing dot;
        /// the result is always safe to put in a `Domain` attribute.
        #[test]
        fn sanitized_domain_is_cookie_safe(s in "\\PC{0,128}") {
            let out = sanitize_cookie_domain(&s);
            prop_assert!(!out.contains("://"));
            prop_assert!(!out.contains('/'));
            prop_assert!(!out.contains('?'));
            prop_assert!(!out.contains('#'));
            prop_assert!(!out.ends_with('.'));
            if !out.starts_with('[') {
                prop_assert!(!out.contains(':'));
            }
        }

        /// Leading-dot scoping only ever applies to multi-label DNS names.
        #[test]
        fn leading_dot_needs_two_labels(s in "[a-z0-9.\\-]{1,64}") {
            if needs_leading_dot(&s) {
                let labels: Vec<&str> = s.trim_start_matches('.').split('.').collect();
                prop_assert!(labels.len() >= 2);
                prop_assert!(labels.iter().all(|l| !l.is_empty()));
                prop_assert!(!s.starts_with('['));
            }
        }

        /// `cookie_domain` agrees with the sanitizer: None exactly when the
        /// sanitized value is empty.
        #[test]
        fn domain_none_iff_empty(host in "[a-z0-9.\\-:]{0,32}") {
            // SAFETY: serialized by the sibling tests module's ENV_LOCK; the
            // proptest runner is single-threaded within this case.
            let _guard = super::tests::ENV_LOCK.lock().unwrap();
            unsafe {
                std::env::set_var("COOKIE_DOMAIN", &host);
            }
            let via_fn = cookie_domain();
            unsafe {
                std::env::remove_var("COOKIE_DOMAIN");
            }
            let clean = sanitize_cookie_domain(&host);
            prop_assert_eq!(via_fn.is_none(), clean.is_empty());
        }
    }
}

#[cfg(test)]
mod domain_tests {
    use super::{needs_leading_dot, sanitize_cookie_domain};

    #[test]
    fn strips_scheme_port_path() {
        assert_eq!(sanitize_cookie_domain("localhost:8200"), "localhost");
        assert_eq!(
            sanitize_cookie_domain("http://localhost:8200/"),
            "localhost"
        );
        assert_eq!(
            sanitize_cookie_domain("https://wiki.example.com"),
            "wiki.example.com"
        );
        assert_eq!(sanitize_cookie_domain("  example.com.  "), "example.com");
        assert_eq!(
            sanitize_cookie_domain("example.com:443/path"),
            "example.com"
        );
    }

    #[test]
    fn keeps_ipv6_brackets() {
        assert_eq!(sanitize_cookie_domain("[::1]:8200"), "[::1]");
        assert_eq!(sanitize_cookie_domain("[::1]"), "[::1]");
    }

    #[test]
    fn single_label_and_ips_stay_host_only() {
        assert!(!needs_leading_dot("localhost"));
        assert!(!needs_leading_dot("127.0.0.1"));
        assert!(!needs_leading_dot("[::1]"));
        assert!(needs_leading_dot("example.com"));
        assert!(needs_leading_dot("wiki.example.com"));
    }

    #[test]
    fn empty_stays_empty() {
        assert_eq!(sanitize_cookie_domain(""), "");
        assert_eq!(sanitize_cookie_domain("   "), "");
    }
}
