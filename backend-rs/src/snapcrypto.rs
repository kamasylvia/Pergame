//! Snapshot blob encryption (spec backup-restore.md §8): optional
//! end-to-end AES-256-GCM with a scrypt-derived key. Passphrase comes from
//! the deployment (`PERGAME_SNAPSHOT_PASSPHRASE`); empty/absent = plain
//! blobs. The salt lives in each manifest, so every snapshot is
//! self-describing while the passphrase stays deployment-side.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use serde::{Deserialize, Serialize};

const SCRYPT_LOG_N: u8 = 15; // N = 2^15
const SCRYPT_R: u32 = 8;
const SCRYPT_P: u32 = 1;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncryptionParams {
    pub kdf: String, // "scrypt"
    pub salt_b64: String,
    pub n: u32,
    pub r: u32,
    pub p: u32,
}

/// Deployment passphrase for snapshot encryption; None = plain snapshots.
pub fn passphrase_from_env() -> Option<String> {
    match std::env::var("PERGAME_SNAPSHOT_PASSPHRASE") {
        Ok(v) if !v.trim().is_empty() => Some(v),
        _ => None,
    }
}

fn derive_key(passphrase: &str, salt: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    scrypt::scrypt(
        passphrase.as_bytes(),
        salt,
        &scrypt::Params::new(SCRYPT_LOG_N, SCRYPT_R, SCRYPT_P, 32).expect("valid scrypt params"),
        &mut out,
    )
    .expect("scrypt derive");
    out
}

pub fn new_params() -> EncryptionParams {
    let salt: [u8; SALT_LEN] = rand::random();
    EncryptionParams {
        kdf: "scrypt".into(),
        salt_b64: b64_encode(&salt),
        n: 1u32 << SCRYPT_LOG_N,
        r: SCRYPT_R,
        p: SCRYPT_P,
    }
}

/// Encrypt `plain` (nonce prefixed) under params+passphrase.
pub fn encrypt(
    params: &EncryptionParams,
    passphrase: &str,
    plain: &[u8],
) -> Result<Vec<u8>, String> {
    let salt = b64_decode(&params.salt_b64)?;
    let key = derive_key(passphrase, &salt);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("cipher init: {e}"))?;
    let nonce_bytes: [u8; NONCE_LEN] = rand::random();
    let nonce = Nonce::from(nonce_bytes);
    let ct = cipher
        .encrypt(&nonce, plain)
        .map_err(|_| "encrypt failed".to_string())?;
    let mut out = nonce_bytes.to_vec();
    out.extend(ct);
    Ok(out)
}

/// Decrypt a nonce-prefixed blob produced by [`encrypt`].
pub fn decrypt(
    params: &EncryptionParams,
    passphrase: &str,
    blob: &[u8],
) -> Result<Vec<u8>, String> {
    if blob.len() <= NONCE_LEN {
        return Err("blob too short".into());
    }
    let salt = b64_decode(&params.salt_b64)?;
    let key = derive_key(passphrase, &salt);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("cipher init: {e}"))?;
    let (nonce, ct) = blob.split_at(NONCE_LEN);
    let nonce_arr: [u8; NONCE_LEN] = nonce.try_into().map_err(|_| "bad nonce".to_string())?;
    cipher
        .decrypt(&Nonce::from(nonce_arr), ct)
        .map_err(|_| "decrypt failed (wrong passphrase or corrupted blob)".to_string())
}

fn b64_encode(data: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn b64_decode(s: &str) -> Result<Vec<u8>, String> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| format!("bad base64 salt: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_wrong_passphrase() {
        let params = new_params();
        let plain = b"snapshot blob content";
        let blob = encrypt(&params, "correct horse", plain).expect("encrypt");
        assert_ne!(blob[..NONCE_LEN], [0u8; NONCE_LEN]);
        let back = decrypt(&params, "correct horse", &blob).expect("decrypt");
        assert_eq!(back, plain);
        assert!(decrypt(&params, "wrong", &blob).is_err());
    }

    #[test]
    fn salt_unique_per_snapshot() {
        let a = new_params();
        let b = new_params();
        assert_ne!(a.salt_b64, b.salt_b64);
    }
}
