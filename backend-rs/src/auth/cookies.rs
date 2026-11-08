//! Auth cookies: `Authorization` (access JWT) + `RefreshToken`, path `/`,
//! httpOnly, domain from `COOKIE_DOMAIN` (leading dot added for subdomains,
//! Go parity), secure unless `ALLOW_UNSECURE` is true/1.

use cookie::Cookie;

pub const ACCESS_COOKIE: &str = "Authorization";
pub const REFRESH_COOKIE: &str = "RefreshToken";

pub fn secure_cookies() -> bool {
    !matches!(
        std::env::var("ALLOW_UNSECURE").as_deref(),
        Ok("true") | Ok("1")
    )
}

pub fn cookie_domain() -> Option<String> {
    match std::env::var("COOKIE_DOMAIN") {
        Ok(d) if !d.is_empty() => {
            // Include leading dot for subdomains (Go parity).
            Some(if d.starts_with('.') {
                d
            } else {
                format!(".{d}")
            })
        }
        _ => None,
    }
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

pub fn access_cookie(token: &str, max_age_secs: i64) -> Cookie<'static> {
    build(ACCESS_COOKIE, token.to_string(), max_age_secs)
}

pub fn refresh_cookie(token: &str, max_age_secs: i64) -> Cookie<'static> {
    build(REFRESH_COOKIE, token.to_string(), max_age_secs)
}

pub fn clear_access_cookie() -> Cookie<'static> {
    build(ACCESS_COOKIE, String::new(), -1)
}

pub fn clear_refresh_cookie() -> Cookie<'static> {
    build(REFRESH_COOKIE, String::new(), -1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

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
