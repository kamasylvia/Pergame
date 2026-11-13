//! OIDC provider manager + service (Go oidc/config.go + services/oidc.service.go
//! parity): env-configured providers (OIDC_1_*..OIDC_10_*), discovery, auth-URL
//! generation with one-time states, code exchange, userinfo, login-or-create.

use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

const PROVIDER_SLOTS: u32 = 10;
const STATE_TTL_MS: i64 = 10 * 60 * 1000;

#[derive(Debug, Clone, Serialize)]
/// One configured identity provider (discovered endpoints + client credentials).
pub struct Provider {
    pub name: String,
    pub client_id: String,
    pub client_secret: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
/// Login-page projection (no secrets).
pub struct PublicProvider {
    pub name: String,
    pub authorization_endpoint: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Discovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    userinfo_endpoint: String,
    #[serde(default)]
    scopes_supported: Vec<String>,
}

#[derive(Debug, Deserialize)]
/// Token-endpoint response (access token + optional OIDC extras).
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub token_type: String,
    #[allow(dead_code)]
    pub expires_in: Option<i64>,
    #[allow(dead_code)]
    pub refresh_token: Option<String>,
    #[allow(dead_code)]
    pub id_token: Option<String>,
    #[allow(dead_code)]
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
/// Standard-claims userinfo subset used for login-or-create.
pub struct UserInfo {
    pub sub: String,
    pub email: Option<String>,
    #[allow(dead_code)]
    pub email_verified: Option<bool>,
    pub preferred_username: Option<String>,
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub picture: Option<String>,
}

#[derive(Debug, Clone)]
/// Pending auth request (one-time state → expiry/link/user).
pub struct StateData {
    pub expiry_ms: i64,
    pub is_link: bool,
    pub user_id: Uuid,
}

/// Provider registry + pending states, built once at startup.
pub struct OidcManager {
    providers: HashMap<String, Provider>,
    states: Mutex<HashMap<String, StateData>>,
    pub http: reqwest::Client,
}

impl OidcManager {
    /// Loads providers from OIDC_N_* env slots, discovering each concurrently.
    pub async fn from_env() -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("http client");
        let mut providers = HashMap::new();

        for i in 1..=PROVIDER_SLOTS {
            let config_url = std::env::var(format!("OIDC_{i}_CONFIG_URL")).unwrap_or_default();
            if config_url.is_empty() {
                continue;
            }
            let name = std::env::var(format!("OIDC_{i}_PROVIDER_NAME"))
                .ok()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| format!("provider_{i}"));
            let client_id = std::env::var(format!("OIDC_{i}_CLIENT_ID")).unwrap_or_default();
            let client_secret =
                std::env::var(format!("OIDC_{i}_CLIENT_SECRET")).unwrap_or_default();
            if client_id.is_empty() || client_secret.is_empty() {
                tracing::error!(%name, "Failed to add OIDC provider: client id/secret required");
                continue;
            }

            match discover(&http, &config_url).await {
                Ok(d) => {
                    let mut scopes = vec!["openid".to_string()];
                    if d.scopes_supported.iter().any(|s| s == "profile") {
                        scopes.push("profile".into());
                    }
                    if d.scopes_supported.iter().any(|s| s == "email") {
                        scopes.push("email".into());
                    }
                    tracing::info!(provider = %name, issuer = %d.issuer, "Added OIDC provider");
                    providers.insert(
                        name.to_lowercase(),
                        Provider {
                            name,
                            client_id,
                            client_secret,
                            authorization_endpoint: d.authorization_endpoint,
                            token_endpoint: d.token_endpoint,
                            userinfo_endpoint: d.userinfo_endpoint,
                            scopes,
                        },
                    );
                }
                Err(e) => tracing::error!(%name, error = %e, "Failed to discover OIDC endpoints"),
            }
        }

        let count = providers.len();
        if count > 0 {
            tracing::info!(count, "OIDC providers loaded");
        } else {
            tracing::info!("No OIDC providers configured");
        }
        Self {
            providers,
            states: Mutex::new(HashMap::new()),
            http,
        }
    }

    /// True when at least one provider discovered successfully.
    pub fn enabled(&self) -> bool {
        !self.providers.is_empty()
    }

    /// Lookup by name (case-insensitive).
    pub fn get_provider(&self, name: &str) -> Result<Provider, String> {
        self.providers
            .get(&name.to_lowercase())
            .cloned()
            .ok_or_else(|| format!("OIDC provider '{}' not found", name))
    }

    /// Public login-page list.
    pub fn get_providers(&self) -> Vec<PublicProvider> {
        self.providers
            .values()
            .map(|p| PublicProvider {
                name: p.name.clone(),
                authorization_endpoint: p.authorization_endpoint.clone(),
            })
            .collect()
    }

    /// Build an auth URL + one-time state (`Some(user)` = account linking).
    pub async fn generate_auth_url(
        &self,
        provider_name: &str,
        redirect_uri: &str,
        link_user_id: Option<Uuid>,
    ) -> Result<(String, String), String> {
        let provider = self.get_provider(provider_name)?;
        let state_bytes: [u8; 32] = rand::random();
        let state = base64_url_encode(&state_bytes);

        let mut states = self.states.lock().await;
        // Lazy expiry sweep (Go uses a 5-minute ticker; behavior equivalent).
        let now = now_ms();
        states.retain(|_, data| data.expiry_ms > now);
        states.insert(
            state.clone(),
            StateData {
                expiry_ms: now + STATE_TTL_MS,
                is_link: link_user_id.is_some(),
                user_id: link_user_id.unwrap_or_else(Uuid::nil),
            },
        );
        drop(states);

        let url = format!(
            "{}?client_id={}&response_type=code&scope={}&redirect_uri={}&state={}",
            provider.authorization_endpoint,
            urlencode(&provider.client_id),
            urlencode(&provider.scopes.join(" ")),
            urlencode(redirect_uri),
            urlencode(&state),
        );
        Ok((url, state))
    }

    /// Consume a state (single-use; errors when unknown/expired).
    pub async fn validate_state(&self, state: &str) -> Result<StateData, String> {
        let mut states = self.states.lock().await;
        let data = states.remove(state).ok_or("invalid state")?;
        if now_ms() > data.expiry_ms {
            return Err("state expired".into());
        }
        Ok(data)
    }

    /// Exchange an auth code for tokens.
    pub async fn exchange_code(
        &self,
        provider: &Provider,
        code: &str,
        redirect_uri: &str,
    ) -> Result<TokenResponse, String> {
        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", provider.client_id.as_str()),
            ("client_secret", provider.client_secret.as_str()),
        ];
        let resp = self
            .http
            .post(&provider.token_endpoint)
            .header("Accept", "application/json")
            .form(&params)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!(
                "token endpoint returned status {}: {}",
                status, body
            ));
        }
        serde_json::from_str(&body).map_err(|e| format!("failed to decode token response: {e}"))
    }

    /// Fetch userinfo for an access token.
    pub async fn get_user_info(
        &self,
        provider: &Provider,
        access_token: &str,
    ) -> Result<UserInfo, String> {
        let resp = self
            .http
            .get(&provider.userinfo_endpoint)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!(
                "userinfo endpoint returned status {}: {}",
                status, body
            ));
        }
        serde_json::from_str(&body).map_err(|e| format!("failed to decode user info: {e}"))
    }
}

async fn discover(http: &reqwest::Client, config_url: &str) -> Result<Discovery, String> {
    let resp = http
        .get(config_url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    if status != reqwest::StatusCode::OK {
        return Err(format!("discovery endpoint returned status {}", status));
    }
    resp.json::<Discovery>()
        .await
        .map_err(|e| format!("failed to decode discovery document: {e}"))
}

/// Wall-clock unix millis (state expiry math).
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn urlencode(v: &str) -> String {
    let mut out = String::new();
    for b in v.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn base64_url_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Shared manager handle carried in `AppState`.
pub type SharedManager = Arc<OidcManager>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencode_keeps_unreserved() {
        assert_eq!(urlencode("abcXYZ019-_.~"), "abcXYZ019-_.~");
        assert_eq!(urlencode("a b/c?d=e&f"), "a%20b%2Fc%3Fd%3De%26f");
        assert_eq!(urlencode(""), "");
    }

    #[test]
    fn state_encoding_is_url_safe() {
        let s = base64_url_encode(&[0xfb, 0xff, 0x00, 0x01]);
        assert!(!s.contains(['+', '/', '=']));
    }
}
