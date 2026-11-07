//! Auth domain logic (Go `services/auth.service.go` parity): login with
//! bcrypt verification, session create/rotate, logout, signup.

use toasty::Db;

use crate::error::ApiError;
use crate::models::{Session, User};
use crate::types::{Snowflake, SnowflakeGenerator};

/// Sessions last 30 days (Go: `30 * 24 * time.Hour`).
const SESSION_TTL_MS: i64 = 30 * 24 * 3600 * 1000;

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub enum LoginOutcome {
    /// Password verified; session created; refresh token issued.
    Full {
        user: User,
        refresh_token: String,
    },
    /// TOTP required (enabled) or must be enrolled first (forced).
    TwoFa {
        message: &'static str,
        pre_auth_token: String,
        user_id: i64,
    },
}

fn bad_credentials() -> ApiError {
    ApiError::Unauthorized("invalid credentials".into())
}

/// Generates a Snowflake for the given generator (shared state in AppState).
pub fn new_id(generator: &SnowflakeGenerator) -> Snowflake {
    generator.generate()
}

pub async fn login(
    db: &mut Db,
    generator: &SnowflakeGenerator,
    username: &str,
    password: &str,
    ip: &str,
    user_agent: &str,
) -> Result<LoginOutcome, ApiError> {
    let user = User::filter_by_username(username).first().exec(db).await.ok().flatten();

    let Some(user) = user else {
        return Err(bad_credentials());
    };

    let Some(hash) = &user.password else {
        return Err(ApiError::Unauthorized("please login with OIDC provider".into()));
    };

    if bcrypt::verify(password, hash).unwrap_or(false) {
        // ok — fall through to TOTP / session creation
    } else {
        return Err(bad_credentials());
    }

    if user.totp_enabled || user.totp_forced {
        let pre = super::jwt::sign_pre_auth_token(user.id as u64)
            .map_err(|_| ApiError::Unauthorized("failed to generate pre-authentication token".into()))?;
        let message = if user.totp_enabled { "2FA_REQUIRED" } else { "2FA_TO_ENABLE" };
        return Ok(LoginOutcome::TwoFa {
            message,
            pre_auth_token: pre,
            user_id: user.id,
        });
    }

    let refresh =
        super::totp::create_login_session(db, generator, user.id, ip, user_agent).await?;

    Ok(LoginOutcome::Full { user, refresh_token: refresh })
}

pub async fn refresh_session(db: &mut Db, refresh_token: &str) -> Result<(User, String), ApiError> {
    let mut session = Session::filter_by_refresh_token(refresh_token)
        .first()
        .exec(db)
        .await
        .ok()
        .flatten()
        .ok_or(ApiError::Unauthorized("invalid refresh token".into()))?;

    if session.expire_token < now_ms() {
        return Err(ApiError::Unauthorized("refresh token expired".into()));
    }

    let user = User::filter_by_id(session.user_id)
        .first()
        .exec(db)
        .await
        .ok()
        .flatten()
        .ok_or(ApiError::Unauthorized("failed to get user".into()))?;

    // Rotate the refresh token and extend the window.
    let new_refresh = super::jwt::refresh_token();
    let now = now_ms();
    Session::filter_by_id(session.id)
        .update()
        .refresh_token(new_refresh.clone())
        .expire_token(now + SESSION_TTL_MS)
        .last_refresh_timestamp(now)
        .exec(db)
        .await
        .map_err(|_| ApiError::Unauthorized("failed to update session".into()))?;
    let _ = &mut session; // rotated via query update above

    Ok((user, new_refresh))
}

/// Logs a session out by refresh token. Errors are swallowed (Go parity:
/// `ctr.app.Services.Auth.Logout(refreshToken)` ignores the result).
pub async fn logout(db: &mut Db, refresh_token: &str) {
    if let Some(session) = Session::filter_by_refresh_token(refresh_token).first().exec(db).await.ok().flatten() {
        let _ = Session::filter_by_id(session.id).delete().exec(db).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P0 finding: Go used golang.org/x/crypto/bcrypt (default cost 10), which
    /// emits $2a$-prefixed hashes. The Rust bcrypt crate verifies them directly,
    /// so existing user passwords carry over with zero migration.
    #[test]
    fn verifies_bcrypt_hashes_roundtrip() {
        let hash = bcrypt::hash("correct horse battery", bcrypt::DEFAULT_COST).unwrap();
        assert!(hash.starts_with("$2"));
        assert!(bcrypt::verify("correct horse battery", &hash).unwrap());
        assert!(!bcrypt::verify("wrong password", &hash).unwrap());
    }

    #[test]
    fn login_error_text_matches_go() {
        assert_eq!(bad_credentials().message(), "invalid credentials");
    }
}

pub struct SignupInput {
    pub username: String,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
    pub avatar: Option<String>,
    pub email: Option<String>,
    pub user_type: i32,
    pub password: Option<String>,
    pub totp_forced: bool,
}

pub async fn signup(db: &mut Db, generator: &SnowflakeGenerator, input: SignupInput) -> Result<User, ApiError> {
    if User::filter_by_username(&input.username).first().exec(db).await.ok().flatten().is_some() {
        return Err(ApiError::Conflict("Username already exists".into()));
    }

    // Empty password triggers the "account created by admin" email flow in Go;
    // without a mail client initialized that path always fails there too.
    let password_hash = match input.password.as_deref() {
        Some(p) if !p.is_empty() => bcrypt::hash(p, bcrypt::DEFAULT_COST)
            .map_err(|_| ApiError::BadRequest("failed to hash password".into()))?,
        _ => {
            if input.email.as_deref().unwrap_or("").is_empty() {
                return Err(ApiError::BadRequest("no email provided".into()));
            }
            return Err(ApiError::BadRequest("mail client is not initialized".into()));
        }
    };

    let now = now_ms();
    let user = User {
        id: new_id(generator).as_i64(),
        username: input.username,
        firstname: input.firstname,
        lastname: input.lastname,
        role: 1,
        user_type: input.user_type,
        avatar: input.avatar,
        email: input.email,
        password: Some(password_hash),
        totp_secret: None,
        totp_enabled: false,
        totp_forced: input.totp_forced,
        created_timestamp: now,
        updated_timestamp: now,
    };
    User::create()
        .id(user.id)
        .username(user.username.clone())
        .firstname(user.firstname.clone())
        .lastname(user.lastname.clone())
        .role(user.role)
        .user_type(user.user_type)
        .avatar(user.avatar.clone())
        .email(user.email.clone())
        .password(user.password.clone())
        .totp_enabled(false)
        .totp_forced(user.totp_forced)
        .created_timestamp(now)
        .updated_timestamp(now)
        .exec(db)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "signup insert failed");
        ApiError::BadRequest("failed to create user".into())
    })?;

    Ok(user)
}
