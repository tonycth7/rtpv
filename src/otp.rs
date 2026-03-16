use anyhow::{anyhow, Result};
use totp_rs::{Algorithm, TOTP};
use url::Url;

pub struct OtpState {
    pub code: String,
    pub remaining: u64,
    pub period: u64,
    pub progress: f32,
    pub issuer: Option<String>,
    pub account: Option<String>,
}

fn totp_from_uri(uri: &str) -> Result<TOTP> {
    let url = Url::parse(uri)?;

    if url.scheme() != "otpauth" {
        return Err(anyhow!("Invalid OTP URI"));
    }

    // Extract secret
    let secret = url
        .query_pairs()
        .find(|(k, _)| k == "secret")
        .ok_or_else(|| anyhow!("OTP secret missing"))?
        .1
        .to_string();

    // Decode base32 secret
    let mut secret_bytes = base32::decode(base32::Alphabet::RFC4648 { padding: false }, &secret)
        .ok_or_else(|| anyhow!("Invalid base32 secret"))?;

    // GitHub and some services use 80-bit secrets.
    // totp-rs requires ≥128 bits, so we pad the secret.
    if secret_bytes.len() < 16 {
        secret_bytes.resize(16, 0);
    }

    // issuer
    // label from URI path
    let label = url.path().trim_start_matches('/');

    // split "issuer:account"
    let (issuer_from_label, account) = if let Some((iss, acc)) = label.split_once(':') {
        (Some(iss.to_string()), acc.to_string())
    } else {
        (None, label.to_string())
    };

    // issuer from query OR label
    let issuer = url
        .query_pairs()
        .find(|(k, _)| k == "issuer")
        .map(|(_, v)| v.to_string())
        .or(issuer_from_label);

    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret_bytes,
        issuer.clone(),
        account.clone(),
    )?;

    Ok(TOTP {
        issuer,
        account_name: account,
        ..totp
    })
}

pub fn from_uri(uri: &str) -> Result<TOTP> {
    totp_from_uri(uri)
}

pub fn current_state(uri: &str) -> Result<OtpState> {
    let totp = totp_from_uri(uri)?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();

    let period = totp.step;
    let elapsed = now % period;
    let remaining = period - elapsed;
    let progress = elapsed as f32 / period as f32;

    let code = totp.generate(now);

    Ok(OtpState {
        code,
        remaining,
        period,
        progress,
        issuer: totp.issuer.clone(),
        account: {
            let name = totp.account_name.clone();
            if name.is_empty() {
                None
            } else {
                Some(name)
            }
        },
    })
}

pub fn validate_uri(uri: &str) -> bool {
    Url::parse(uri)
        .map(|u| u.scheme() == "otpauth")
        .unwrap_or(false)
}
