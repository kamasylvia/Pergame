//! SMTP mail client (Go app/mail.go parity: implicit-TLS SMTP, credentials
//! from SMTP_HOST / SMTP_MAIL / SMTP_PASSWORD; from-address falls back to
//! SMTP_MAIL). Optional — password-reset/account flows degrade when unset.

use lettre::message::Mailbox;
use lettre::{SmtpTransport, Transport};

pub struct MailClient {
    transport: SmtpTransport,
    from: String,
}

pub fn from_env() -> Option<MailClient> {
    let host = std::env::var("SMTP_HOST").ok()?;
    let username = std::env::var("SMTP_MAIL").ok()?;
    let password = std::env::var("SMTP_PASSWORD").ok()?;
    if host.is_empty() || username.is_empty() || password.is_empty() {
        return None;
    }
    // Go: WithSSLPort(true) — implicit TLS on 465; lettre::relay does the same.
    let transport = SmtpTransport::relay(&host)
        .ok()?
        .credentials(lettre::transport::smtp::authentication::Credentials::new(
            username.clone(),
            password,
        ))
        .build();
    let from = std::env::var("SMTP_MAIL_FROM")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or(username);
    Some(MailClient { transport, from })
}

impl MailClient {
    /// Sends a templated HTML email with a plain-text fallback (Go parity:
    /// SetBodyString plain + AddAlternative HTML).
    pub fn send_html(
        &self,
        to: &str,
        subject: &str,
        plain: &str,
        html: &str,
    ) -> Result<(), String> {
        let mailbox_to: Mailbox = to.parse().map_err(|e| format!("invalid recipient: {e}"))?;
        let mailbox_from: Mailbox = self
            .from
            .parse()
            .map_err(|e| format!("invalid sender: {e}"))?;

        let email = lettre::Message::builder()
            .from(mailbox_from)
            .to(mailbox_to)
            .subject(subject)
            .multipart(lettre::message::MultiPart::alternative_plain_html(
                plain.to_string(),
                html.to_string(),
            ))
            .map_err(|e| format!("failed to build email: {e}"))?;

        self.transport
            .send(&email)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

/// Renders a Go html/template file that only uses {{.Var}} placeholders by
/// substituting the given key/value pairs (email templates are flat).
pub fn render_go_template(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (key, value) in vars {
        out = out.replace(&format!("{{{{.{}}}}}", key), value);
    }
    out
}

pub const PASSWORD_RESET_TEMPLATE: &str = include_str!("../emails/password-reset.html");
pub const ACCOUNT_CREATE_TEMPLATE: &str = include_str!("../emails/account-create.html");
