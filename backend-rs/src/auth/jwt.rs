//! JWT signing/verification (HS256), mirroring the Go backend claims.
//!
//! Access token:  `{sub: "<id>", iss: "pergame", exp, iat, role: "<role>"}`
//! Pre-auth token (TOTP flow): same shape + `pre_auth: true`, 3-minute expiry.
//! Refresh tokens are opaque random hex (see [`refresh_token`]), not JWTs.

use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

pub const JWT_ISSUER: &str = "pergame";
const PRE_AUTH_EXPIRY_SECS: i64 = 3 * 60;

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessClaims {
    pub sub: String,
    pub iss: String,
    pub exp: i64,
    pub iat: i64,
    pub role: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PreAuthClaims {
    pub sub: String,
    pub iss: String,
    pub exp: i64,
    pub iat: i64,
    pub pre_auth: bool,
}

fn secret() -> Vec<u8> {
    match std::env::var("JWT_SECRET") {
        Ok(s) if !s.is_empty() => s.into_bytes(),
        _ => {
            tracing::warn!(
                "JWT_SECRET not set; using development default (do not use in production)"
            );
            b"development-jwt-secret".to_vec()
        }
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn sign_access_token(
    user_id: uuid::Uuid,
    role: i32,
    expiry_secs: i64,
) -> Result<String, jsonwebtoken::errors::Error> {
    let claims = AccessClaims {
        sub: user_id.to_string(),
        iss: JWT_ISSUER.into(),
        exp: now() + expiry_secs,
        iat: now(),
        role: role.to_string(),
    };
    encode(
        &Header::new(jsonwebtoken::Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(&secret()),
    )
}

pub fn sign_pre_auth_token(user_id: uuid::Uuid) -> Result<String, jsonwebtoken::errors::Error> {
    let claims = PreAuthClaims {
        sub: user_id.to_string(),
        iss: JWT_ISSUER.into(),
        exp: now() + PRE_AUTH_EXPIRY_SECS,
        iat: now(),
        pre_auth: true,
    };
    encode(
        &Header::new(jsonwebtoken::Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(&secret()),
    )
}

/// Verifies an access token (HS256 only, exp validated) and returns (user_id, role).
pub fn verify_access_token(token: &str) -> Result<(uuid::Uuid, i32), jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.validate_exp = true;
    validation.validate_aud = false;
    // Go parity: the issuer is not checked on read.
    validation.iss = None;
    validation.required_spec_claims.clear(); // Go treats missing exp as valid
    let data = decode::<AccessClaims>(token, &DecodingKey::from_secret(&secret()), &validation)?;
    let user_id: uuid::Uuid = data.claims.sub.parse().map_err(|_| {
        jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::InvalidClaimFormat(
            "sub/role".into(),
        ))
    })?;
    let role: i32 = data.claims.role.parse().map_err(|_| {
        jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::InvalidClaimFormat(
            "sub/role".into(),
        ))
    })?;
    Ok((user_id, role))
}

/// Verifies a pre-auth token and returns the user id.
pub fn verify_pre_auth_token(token: &str) -> Result<uuid::Uuid, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.validate_exp = true;
    validation.validate_aud = false;
    validation.iss = None;
    validation.required_spec_claims.clear();
    let data = decode::<PreAuthClaims>(token, &DecodingKey::from_secret(&secret()), &validation)?;
    if !data.claims.pre_auth {
        return Err(jsonwebtoken::errors::Error::from(
            jsonwebtoken::errors::ErrorKind::InvalidClaimFormat("sub/role".into()),
        ));
    }
    data.claims.sub.parse().map_err(|_| {
        jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::InvalidClaimFormat(
            "sub/role".into(),
        ))
    })
}

/// 45 random bytes hex-encoded (90 chars), Go `signRefreshToken` parity.
pub fn refresh_token() -> String {
    use rand::RngExt;
    let mut bytes = [0u8; 45];
    rand::rng().fill(&mut bytes);
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serializes env mutation across tests (set_var is unsafe + racy in parallel).
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_secret<R>(secret: &str, f: impl FnOnce() -> R) -> R {
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: env access is serialized by ENV_LOCK; no other threads read
        // JWT_SECRET concurrently within these tests.
        unsafe {
            std::env::set_var("JWT_SECRET", secret);
        }
        let out = f();
        unsafe {
            std::env::remove_var("JWT_SECRET");
        }
        out
    }

    #[test]
    fn access_token_roundtrip() {
        with_secret("test-secret-for-unit", || {
            let uid = uuid::Uuid::now_v7();
            let token = sign_access_token(uid, 1, 1800).unwrap();
            let (id, role) = verify_access_token(&token).unwrap();
            assert_eq!(id, uid);
            assert_eq!(role, 1);
        });
    }

    #[test]
    fn rejects_wrong_secret() {
        // Never nest with_secret calls: ENV_LOCK is not reentrant.
        let token = with_secret("secret-a", || {
            sign_access_token(uuid::Uuid::now_v7(), 1, 1800).unwrap()
        });
        with_secret("secret-b", || {
            assert!(verify_access_token(&token).is_err());
        });
    }

    #[test]
    fn pre_auth_flow() {
        with_secret("test-secret-for-unit", || {
            let uid = uuid::Uuid::now_v7();
            let token = sign_pre_auth_token(uid).unwrap();
            assert_eq!(verify_pre_auth_token(&token).unwrap(), uid);
            // An access token is not a pre-auth token.
            let access = sign_access_token(uid, 1, 1800).unwrap();
            assert!(verify_pre_auth_token(&access).is_err());
        });
    }

    #[test]
    fn refresh_token_shape() {
        let t = refresh_token();
        assert_eq!(t.len(), 90);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
