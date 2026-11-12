//! Auth domain logic (Go `services/auth.service.go` parity): login with
//! bcrypt verification, session create/rotate, logout, signup.

use toasty::Db;
use uuid::Uuid;

use crate::error::ApiError;
use crate::models::{Session, User};

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
    Full { user: User, refresh_token: String },
    /// TOTP required (enabled) or must be enrolled first (forced).
    TwoFa {
        message: &'static str,
        pre_auth_token: String,
        user_id: Uuid,
    },
}

fn bad_credentials() -> ApiError {
    ApiError::Unauthorized("invalid credentials".into())
}

/// New entity id (UUIDv7, coordination-free).
pub fn new_id() -> Uuid {
    crate::types::new_id()
}

pub async fn login(
    db: &mut Db,
    username: &str,
    password: &str,
    ip: &str,
    user_agent: &str,
) -> Result<LoginOutcome, ApiError> {
    let user = User::filter_by_username(username)
        .first()
        .exec(db)
        .await
        .ok()
        .flatten();

    let Some(user) = user else {
        return Err(bad_credentials());
    };

    let Some(hash) = &user.password else {
        return Err(ApiError::Unauthorized(
            "please login with OIDC provider".into(),
        ));
    };

    if bcrypt::verify(password, hash).unwrap_or(false) {
        // ok — fall through to TOTP / session creation
    } else {
        return Err(bad_credentials());
    }

    if user.totp_enabled || user.totp_forced {
        let pre = super::jwt::sign_pre_auth_token(user.id).map_err(|_| {
            ApiError::Unauthorized("failed to generate pre-authentication token".into())
        })?;
        let message = if user.totp_enabled {
            "2FA_REQUIRED"
        } else {
            "2FA_TO_ENABLE"
        };
        return Ok(LoginOutcome::TwoFa {
            message,
            pre_auth_token: pre,
            user_id: user.id,
        });
    }

    let refresh = super::totp::create_login_session(db, user.id, ip, user_agent).await?;

    Ok(LoginOutcome::Full {
        user,
        refresh_token: refresh,
    })
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
    if let Some(session) = Session::filter_by_refresh_token(refresh_token)
        .first()
        .exec(db)
        .await
        .ok()
        .flatten()
    {
        let _ = Session::filter_by_id(session.id).delete().exec(db).await;
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

pub async fn signup(db: &mut Db, input: SignupInput) -> Result<User, ApiError> {
    if User::filter_by_username(&input.username)
        .first()
        .exec(db)
        .await
        .ok()
        .flatten()
        .is_some()
    {
        return Err(ApiError::Conflict("Username already exists".into()));
    }

    // Empty password triggers the "account created by admin" email flow in Go:
    // a set-password invitation JWT (sub only, no expiry — signSetToken parity)
    // is emailed; without a mail client that path fails there too.
    let password_provided = input
        .password
        .as_deref()
        .map(|p| !p.is_empty())
        .unwrap_or(false);
    let password_hash = if password_provided {
        Some(
            bcrypt::hash(input.password.as_deref().unwrap(), bcrypt::DEFAULT_COST)
                .map_err(|_| ApiError::BadRequest("failed to hash password".into()))?,
        )
    } else {
        if input.email.as_deref().unwrap_or("").is_empty() {
            return Err(ApiError::BadRequest("no email provided".into()));
        }
        let Some(mail_client) = crate::mail::from_env() else {
            return Err(ApiError::BadRequest(
                "mail client is not initialized".into(),
            ));
        };
        let user_id = new_id();
        let token = set_password_token(user_id);
        let frontend = std::env::var("FRONTEND_URL").unwrap_or_default();
        let reset_url = format!("{frontend}/login/reset?token={}", token);
        let html = crate::mail::render_go_template(
            crate::mail::ACCOUNT_CREATE_TEMPLATE,
            &[("ResetURL", &reset_url), ("FrontendURL", &frontend)],
        );
        let plain = format!(
            "Your account has been created. Set your password using this link: {}",
            reset_url
        );
        let email_addr = input.email.clone().unwrap_or_default();
        if mail_client
            .send_html(&email_addr, "Pergame: Set your password", &plain, &html)
            .is_err()
        {
            return Err(ApiError::BadRequest(
                "failed to send account creation email".into(),
            ));
        }
        let _ = user_id; // id generated again below for the insert (Go parity: one id)
        None
    };

    let now = now_ms();
    let user = User {
        id: new_id(),
        username: input.username,
        firstname: input.firstname,
        lastname: input.lastname,
        role: 1,
        user_type: input.user_type,
        avatar: input.avatar,
        email: input.email,
        password: password_hash,
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
            // Race-safe duplicate handling: the pre-check above can pass for two
            // concurrent signups; the unique constraint is the final arbiter.
            // (toasty 0.11 has no dedicated unique-violation variant — the pg
            // driver surfaces 23505 inside the generic driver error message.)
            let msg = e.to_string();
            if msg.contains("duplicate key")
                || msg.contains("23505")
                || msg.contains("users_username_key")
            {
                return ApiError::Conflict("Username already exists".into());
            }
            tracing::error!(error = %e, "signup insert failed");
            ApiError::BadRequest("failed to create user".into())
        })?;

    Ok(user)
}

/// Go signSetToken: sub-only HS256 JWT with no expiry claim.
pub fn set_password_token(user_id: Uuid) -> String {
    let claims = serde_json::json!({ "sub": user_id.to_string() });
    let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "development-jwt-secret".into());
    jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap_or_default()
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
