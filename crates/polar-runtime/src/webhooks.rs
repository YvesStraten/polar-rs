use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::HashMap;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, PartialEq, Eq)]
pub enum WebhookError {
    MissingHeader(&'static str),
    InvalidSecret,
    NoSignature,
    BadSignature,
}

impl std::fmt::Display for WebhookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebhookError::MissingHeader(h) => write!(f, "missing header: {h}"),
            WebhookError::InvalidSecret => write!(f, "invalid webhook secret"),
            WebhookError::NoSignature => write!(f, "no signature in webhook-signature header"),
            WebhookError::BadSignature => write!(f, "webhook signature verification failed"),
        }
    }
}
impl std::error::Error for WebhookError {}

fn header<'a>(
    headers: &'a HashMap<String, String>,
    name: &'static str,
) -> Result<&'a str, WebhookError> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
        .ok_or(WebhookError::MissingHeader(name))
}

pub fn verify(
    payload: &[u8],
    headers: &HashMap<String, String>,
    secret: &str,
) -> Result<(), WebhookError> {
    let id = header(headers, "webhook-id")?;
    let timestamp = header(headers, "webhook-timestamp")?;
    let sig_header = header(headers, "webhook-signature")?;

    let key_b64 = secret.strip_prefix("whsec_").unwrap_or(secret);
    let key = base64::engine::general_purpose::STANDARD
        .decode(key_b64)
        .map_err(|_| WebhookError::InvalidSecret)?;
    if key.is_empty() {
        return Err(WebhookError::InvalidSecret);
    }

    let mut signed = Vec::with_capacity(id.len() + timestamp.len() + payload.len() + 2);
    signed.extend_from_slice(id.as_bytes());
    signed.push(b'.');
    signed.extend_from_slice(timestamp.as_bytes());
    signed.push(b'.');
    signed.extend_from_slice(payload);

    let mut found = false;
    for token in sig_header.split(' ') {
        let Some((version, sig)) = token.split_once(',') else {
            continue;
        };
        if version != "v1" {
            continue;
        }
        found = true;
        let Ok(sig_bytes) = base64::engine::general_purpose::STANDARD.decode(sig) else {
            continue;
        };
        let mut mac = HmacSha256::new_from_slice(&key).map_err(|_| WebhookError::InvalidSecret)?;
        mac.update(&signed);
        if mac.verify_slice(&sig_bytes).is_ok() {
            return Ok(());
        }
    }
    if found {
        Err(WebhookError::BadSignature)
    } else {
        Err(WebhookError::NoSignature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn headers(sig: &str) -> HashMap<String, String> {
        let mut h = HashMap::new();
        h.insert("webhook-id".into(), "msg_p5jXN8AQM9LWM0D4loKWxJek".into());
        h.insert("webhook-timestamp".into(), "1614265330".into());
        h.insert("webhook-signature".into(), sig.into());
        h
    }

    #[test]
    fn verifies_canonical_standard_webhooks_vector() {
        let secret = "whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw";
        let payload = br#"{"test": 2432232314}"#;
        let h = headers("v1,g0hM9SsE+OTPJTGt/tmIKtSyZlE3uFJELVlNIOLJ1OE=");
        assert!(verify(payload, &h, secret).is_ok());
    }

    #[test]
    fn rejects_tampered_payload() {
        let secret = "whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw";
        let payload = br#"{"test": 9999999999}"#;
        let h = headers("v1,g0hM9SsE+OTPJTGt/tmIKtSyZlE3uFJELVlNIOLJ1OE=");
        assert!(matches!(
            verify(payload, &h, secret),
            Err(WebhookError::BadSignature)
        ));
    }

    #[test]
    fn rejects_empty_key() {
        let mut h = HashMap::new();
        h.insert("webhook-id".into(), "msg_test".into());
        h.insert("webhook-timestamp".into(), "1700000000".into());
        h.insert("webhook-signature".into(), "v1,c2ln".into());
        assert!(matches!(
            verify(b"{}", &h, "whsec_"),
            Err(WebhookError::InvalidSecret)
        ));
    }

    #[test]
    fn rejects_missing_header() {
        let secret = "whsec_x";
        let h = HashMap::new();
        assert!(matches!(
            verify(b"{}", &h, secret),
            Err(WebhookError::MissingHeader(_))
        ));
    }
}
