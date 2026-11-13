//! TOTP domain logic (Go `services/auth.service.go` TOTP section parity):
//! secret generation (SHA1 / 6 digits / 30s step / skew 1 — pquerna/otp
//! defaults), 8-digit backup codes (bcrypt-hashed), code validation that
//! consumes a backup code on match, enable/disable flows.

use toasty::Db;
use totp_rs::{Algorithm, Secret, Totp};

use crate::error::ApiError;
use crate::models::{RecoveryCode, User};
use uuid::Uuid;

/// Wall-clock unix millis (TOTP/backup-code timestamps).
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Builds the TOTP instance for a base32 secret (Go `totp.Validate` parity:
/// SHA1, 6 digits, 30s step, ±1 step skew).
fn totp_for_secret(base32_secret: &str) -> Result<Totp, ApiError> {
    let raw = Secret::try_from_base32(base32_secret)
        .map_err(|_| ApiError::BadRequest("invalid TOTP secret".into()))?;

    totp_rs::Builder::new()
        .with_algorithm(Algorithm::SHA1)
        .with_digits(6)
        .with_skew(1)
        .with_step_duration(30)
        .with_secret(raw)
        .with_account_name("pergame")
        .build()
        .map_err(|e| {
            tracing::error!(error = %e, "totp build failed");
            ApiError::BadRequest("invalid TOTP secret".into())
        })
}

/// Generates a new secret + otpauth URL for enrollment (issuer "Pergame").
pub fn generate_secret(username: &str) -> Result<(String, String), ApiError> {
    let totp = totp_rs::Builder::new()
        .with_algorithm(Algorithm::SHA1)
        .with_digits(6)
        .with_skew(1)
        .with_step_duration(30)
        .with_secret(Secret::generate())
        .with_account_name(username)
        .with_issuer(Some("Pergame"))
        .build()
        .map_err(|e| {
            tracing::error!(error = %e, "totp generate failed");
            ApiError::Internal(Box::new(std::io::Error::other(
                "failed to generate TOTP key",
            )))
        })?;
    let url = totp.to_url().map_err(|e| {
        tracing::error!(error = %e, "totp url failed");
        ApiError::Internal(Box::new(std::io::Error::other("failed to build TOTP url")))
    })?;
    Ok((totp.secret().to_base32(), url))
}

/// 8 backup codes: 8-digit numbers in [10000000, 100000000), Go parity.
fn generate_backup_codes() -> Vec<String> {
    use rand::distr::{Distribution, Uniform};
    let mut rng = rand::rng();
    let dist = Uniform::new(10_000_000u32, 100_000_000).expect("uniform range");
    (0..8)
        .map(|_| format!("{:08}", dist.sample(&mut rng)))
        .collect()
}

/// Validates a code against the user: 6 digits = TOTP, 8 digits = backup code
/// (bcrypt match consumes the code, Go parity).
pub async fn validate_code(db: &mut Db, user: &User, code: &str) -> bool {
    if code.len() == 6 {
        let Some(secret) = &user.totp_secret else {
            return false;
        };
        return match totp_for_secret(secret) {
            Ok(totp) => totp.check_current(code).is_some(),
            Err(_) => false,
        };
    }
    if code.len() == 8 {
        let Ok(codes) = RecoveryCode::filter_by_user_id(user.id).exec(db).await else {
            return false;
        };
        for rc in codes {
            if rc.used {
                continue;
            }
            if bcrypt::verify(code, &rc.code).unwrap_or(false) {
                let _ = RecoveryCode::filter_by_id(rc.id)
                    .update()
                    .used(true)
                    .exec(db)
                    .await;
                return true;
            }
        }
    }
    false
}

/// Enables TOTP: verifies the enrollment code, stores 8 bcrypt-hashed backup
/// codes, marks the user enabled with the secret. Returns the plain codes.
pub async fn enable_totp(
    db: &mut Db,
    user_id: Uuid,
    secret: &str,
    code: &str,
) -> Result<Vec<String>, ApiError> {
    let totp = totp_for_secret(secret)?;
    if totp.check_current(code).is_none() {
        return Err(ApiError::BadRequest("invalid verification code".into()));
    }

    let plain_codes = generate_backup_codes();
    let now = now_ms();
    for plain in &plain_codes {
        let hash = bcrypt::hash(plain, bcrypt::DEFAULT_COST)
            .map_err(|_| ApiError::BadRequest("failed to create backup codes".into()))?;
        RecoveryCode::create()
            .id(crate::types::new_id())
            .user_id(user_id)
            .code(hash)
            .used(false)
            .created_timestamp(now)
            .exec(db)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "backup code insert failed");
                ApiError::BadRequest("failed to create backup codes".into())
            })?;
    }

    User::filter_by_id(user_id)
        .update()
        .totp_secret(Some(secret.to_string()))
        .totp_enabled(true)
        .exec(db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "totp enable update failed");
            ApiError::BadRequest("failed to enable TOTP".into())
        })?;

    Ok(plain_codes)
}

/// Disables TOTP: validates the code, clears the secret/flag, deletes the
/// user's backup codes.
pub async fn disable_totp(db: &mut Db, user_id: Uuid, code: &str) -> Result<(), ApiError> {
    let user = User::filter_by_id(user_id)
        .first()
        .exec(db)
        .await
        .ok()
        .flatten()
        .ok_or(ApiError::BadRequest("failed to get user".into()))?;

    if !validate_code(db, &user, code).await {
        return Err(ApiError::BadRequest("invalid verification code".into()));
    }

    User::filter_by_id(user_id)
        .update()
        .totp_secret(None)
        .totp_enabled(false)
        .exec(db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to disable TOTP");
            ApiError::BadRequest("failed to disable TOTP".into())
        })?;

    // Delete all the user's backup codes.
    if let Ok(codes) = RecoveryCode::filter_by_user_id(user_id).exec(db).await {
        for rc in codes {
            let _ = RecoveryCode::filter_by_id(rc.id).delete().exec(db).await;
        }
    }
    Ok(())
}

/// Verifies a pre-auth token and completes the login (Go VerifyTOTPAndLogin):
/// returns the user; the caller creates the session.
pub async fn verify_pre_auth_and_user(
    db: &mut Db,
    pre_auth_token: &str,
    code: &str,
) -> Result<User, ApiError> {
    let user_id = super::jwt::verify_pre_auth_token(pre_auth_token)
        .map_err(|_| ApiError::Unauthorized("invalid pre-authentication token".into()))?;

    let user = User::filter_by_id(user_id)
        .first()
        .exec(db)
        .await
        .ok()
        .flatten()
        .ok_or(ApiError::Unauthorized("user not found".into()))?;

    if !validate_code(db, &user, code).await {
        return Err(ApiError::Unauthorized("invalid 2FA code".into()));
    }
    Ok(user)
}

/// Shared session-row creation for login / TOTP-verify / OIDC paths.
pub async fn create_login_session(
    db: &mut Db,
    user_id: Uuid,
    ip: &str,
    user_agent: &str,
) -> Result<String, ApiError> {
    create_login_session_type(db, user_id, ip, user_agent, "login").await
}

/// Session creation with an explicit `type` column value.
pub async fn create_login_session_type(
    db: &mut Db,
    user_id: Uuid,
    ip: &str,
    user_agent: &str,
    session_type: &str,
) -> Result<String, ApiError> {
    use crate::models::Session;
    let refresh = super::jwt::refresh_token();
    let now = now_ms();
    Session::create()
        .id(crate::types::new_id())
        .user_id(user_id)
        .refresh_token(refresh.clone())
        .expire_token(now + 30 * 24 * 3600 * 1000)
        .last_refresh_timestamp(now)
        .active(1)
        .ip_addr(Some(ip.to_string()))
        .user_agent(Some(user_agent.to_string()))
        .location(None)
        .session_type(Some(session_type.to_string()))
        .login_timestamp(now)
        .logout_timestamp(0)
        .exec(db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "session create failed");
            ApiError::Unauthorized("failed to create session".into())
        })?;
    Ok(refresh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_generates_otpauth_url() {
        let (secret, url) = generate_secret("ada").unwrap();
        assert!(!secret.is_empty());
        assert!(url.starts_with("otpauth://totp/"));
        assert!(url.contains("Pergame"));
        // The secret round-trips through the validator's parser.
        assert!(totp_for_secret(&secret).is_ok());
        assert!(totp_for_secret("not-base32!!").is_err());
    }

    #[test]
    fn backup_codes_are_eight_unique_8digit() {
        let codes = generate_backup_codes();
        assert_eq!(codes.len(), 8);
        for c in &codes {
            assert_eq!(c.len(), 8);
            assert!(c.chars().all(|ch| ch.is_ascii_digit()));
        }
        let mut sorted = codes.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 8, "codes must be unique");
    }
}
