//! SMTP mail delivery for Klarbog (DK-EMAIL-DELIVERY-001).
//! Faithful port of ai-gateway `email.rs` with Klarbog-specific from defaults.

use lettre::message::header::ContentType;
use lettre::message::{Attachment, Mailbox, Message, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};

const DEFAULT_FROM_EMAIL: &str = "info+klarbog@pellucidsoftware.com";
const DEFAULT_FROM_NAME: &str = "Klarbog";

fn env_or(a: &str, b: &str, default: &str) -> String {
    std::env::var(a)
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var(b).ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| default.to_string())
}

pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub secure: bool,
    pub user: String,
    pub pass: String,
    pub from_name: String,
    pub from_email: String,
}

impl SmtpConfig {
    pub fn from_env() -> Self {
        let from_email = env_or("KLARBOG_SMTP_FROM", "SMTP_SYSTEM_FROM", DEFAULT_FROM_EMAIL);
        let from_name = env_or(
            "KLARBOG_SMTP_FROM_NAME",
            "SMTP_SYSTEM_FROM_NAME",
            DEFAULT_FROM_NAME,
        );
        SmtpConfig {
            host: env_or("SMTP_SYSTEM_HOST", "SMTP_HOST", "send.one.com"),
            port: env_or("SMTP_SYSTEM_PORT", "SMTP_PORT", "465")
                .parse()
                .unwrap_or(465),
            secure: env_or("SMTP_SYSTEM_SECURE", "SMTP_SECURE", "true") == "true",
            user: env_or("SMTP_SYSTEM_USER", "SMTP_USER", ""),
            pass: env_or("SMTP_SYSTEM_PASS", "SMTP_PASS", ""),
            from_name,
            from_email,
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.user.is_empty() && !self.pass.is_empty()
    }

    pub fn transport(&self) -> Option<AsyncSmtpTransport<Tokio1Executor>> {
        if !self.is_configured() {
            return None;
        }
        let creds = Credentials::new(self.user.clone(), self.pass.clone());
        let builder = if self.secure {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&self.host).ok()?
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.host).ok()?
        };
        Some(builder.port(self.port).credentials(creds).build())
    }
}

pub fn email_dry_run_from_env() -> bool {
    std::env::var("KLARBOG_EMAIL_DRY_RUN")
        .ok()
        .is_some_and(|v| matches!(v.trim(), "1" | "true" | "yes"))
}

/// Send plain-text email with one attachment. When `dry_run` is true or SMTP
/// is not configured, records success without a network call.
#[allow(clippy::too_many_arguments)]
pub async fn send_mail_with_attachment(
    cfg: &SmtpConfig,
    to: &str,
    subject: &str,
    text: &str,
    filename: &str,
    mime: &str,
    bytes: &[u8],
    dry_run: bool,
) -> Result<(), String> {
    if dry_run || !cfg.is_configured() {
        return Ok(());
    }
    let transport = cfg
        .transport()
        .ok_or_else(|| "smtp_not_configured".to_string())?;
    let from: Mailbox = format!("{} <{}>", cfg.from_name, cfg.from_email)
        .parse()
        .map_err(|e| format!("bad from: {e}"))?;
    let to_mb: Mailbox = to.parse().map_err(|e| format!("bad to: {e}"))?;

    let attachment = Attachment::new(filename.to_string()).body(
        bytes.to_vec(),
        ContentType::parse(mime).map_err(|e| e.to_string())?,
    );

    let msg = Message::builder()
        .from(from)
        .to(to_mb)
        .subject(subject)
        .multipart(
            MultiPart::mixed()
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_PLAIN)
                        .body(text.to_string()),
                )
                .singlepart(attachment),
        )
        .map_err(|e| e.to_string())?;

    transport
        .send(msg)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_and_disabled_without_creds() {
        std::env::remove_var("SMTP_SYSTEM_USER");
        std::env::remove_var("SMTP_USER");
        std::env::remove_var("SMTP_SYSTEM_PASS");
        std::env::remove_var("SMTP_PASS");
        std::env::remove_var("KLARBOG_SMTP_FROM");
        std::env::remove_var("SMTP_SYSTEM_FROM");
        let c = SmtpConfig::from_env();
        assert_eq!(c.host, "send.one.com");
        assert_eq!(c.port, 465);
        assert!(c.secure);
        assert_eq!(c.from_email, DEFAULT_FROM_EMAIL);
        assert_eq!(c.from_name, DEFAULT_FROM_NAME);
        assert!(!c.is_configured());
        assert!(c.transport().is_none());
    }

    #[tokio::test]
    async fn dry_run_skips_network() {
        let cfg = SmtpConfig::from_env();
        send_mail_with_attachment(
            &cfg,
            "test@example.com",
            "subject",
            "body",
            "inv.json",
            "application/json",
            b"{}",
            true,
        )
        .await
        .unwrap();
    }
}
