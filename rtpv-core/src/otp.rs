// OTP — TOTP / HOTP via `totp-rs`.
//
// Parses `otpauth://` URIs stored in the `otp:` field.
// Generates live codes, remaining seconds, and progress (for TUI).

use anyhow::{bail, Context, Result};
use totp_rs::{Algorithm, Secret, TOTP};

// ─────────────────────────────────────────────────────────────────────────────
// OTP state
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct OtpState {
    pub code:        String,
    pub remaining:   u64,   // seconds until next rotation
    pub period:      u64,   // total period (default 30s)
    pub progress:    f32,   // 0.0..1.0 (fraction of period elapsed)
    pub issuer:      Option<String>,
    pub account:     Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Build TOTP from an otpauth:// URI
// ─────────────────────────────────────────────────────────────────────────────
pub fn from_uri(uri: &str) -> Result<TOTP> {
    TOTP::from_url(uri)
        .map_err(|e| anyhow::anyhow!("Invalid OTP URI: {:?}", e))
}

/// Build TOTP from a raw base32 secret (fallback when no URI).
pub fn from_secret(secret: &str, digits: u32, period: u64) -> Result<TOTP> {
    let secret_bytes = Secret::Encoded(secret.to_string())
        .to_bytes()
        .map_err(|e| anyhow::anyhow!("Bad OTP secret: {:?}", e))?;
    Ok(TOTP::new(Algorithm::SHA1, digits as usize, 1, period, secret_bytes, None, String::new())?)
}

// ─────────────────────────────────────────────────────────────────────────────
// Current code + timing
// ─────────────────────────────────────────────────────────────────────────────
pub fn current(totp: &TOTP) -> Result<OtpState> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();

    let code = totp.generate(now);
    let period = totp.step;
    let elapsed = now % period;
    let remaining = period - elapsed;
    let progress = elapsed as f32 / period as f32;

    Ok(OtpState {
        code,
        remaining,
        period,
        progress,
        issuer:  totp.issuer.clone(),
        account: Some(totp.account_name.clone()).filter(|s| !s.is_empty()),
    })
}

/// Generate the current code for an otpauth:// URI stored in an entry field.
pub fn code_for_uri(uri: &str) -> Result<OtpState> {
    let totp = from_uri(uri)?;
    current(&totp)
}

// ─────────────────────────────────────────────────────────────────────────────
// Export / import helpers
// ─────────────────────────────────────────────────────────────────────────────
/// Parse a Google Authenticator migration protobuf export (base64 URL).
/// Returns a list of (label, otpauth_uri) pairs.
/// This is a best-effort implementation using manual protobuf parsing.
pub fn parse_google_migration_qr(payload: &str) -> Result<Vec<(String, String)>> {
    // Google QR: otpauth-migration://offline?data=<base64>
    let data = payload
        .trim_start_matches("otpauth-migration://offline?data=")
        .trim_start_matches("otpauth-migration://offline%3Fdata%3D");
    let data = urlencoding::decode(data)
        .unwrap_or(std::borrow::Cow::Borrowed(data));
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let bytes = STANDARD.decode(data.as_ref())
        .context("Invalid base64 in migration QR")?;
    // Minimal protobuf parser for OtpParameters fields
    parse_migration_protobuf(&bytes)
}

fn parse_migration_protobuf(data: &[u8]) -> Result<Vec<(String, String)>> {
    // Simplified — field 1 = OtpParameters repeated
    // Each OtpParameters: field 1 = secret (bytes), field 2 = name, field 3 = issuer,
    //   field 4 = algorithm, field 5 = digits, field 6 = type
    let mut results = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        let (tag, wire, advance) = read_varint(data, pos)?;
        pos += advance;
        if wire == 2 && tag == 1 {
            // OtpParameters length-delimited
            let (len, advance2) = read_raw_varint(data, pos)?;
            pos += advance2;
            let param_data = &data[pos..pos + len as usize];
            pos += len as usize;
            if let Ok(entry) = parse_otp_parameters(param_data) {
                results.push(entry);
            }
        } else {
            // skip unknown field
            match wire {
                0 => { let (_, a) = read_raw_varint(data, pos)?; pos += a; }
                1 => { pos += 8; }
                2 => { let (l, a) = read_raw_varint(data, pos)?; pos += a + l as usize; }
                5 => { pos += 4; }
                _ => break,
            }
        }
    }
    Ok(results)
}

fn parse_otp_parameters(data: &[u8]) -> Result<(String, String)> {
    let mut secret_b32 = String::new();
    let mut name = String::new();
    let mut issuer = String::new();
    let mut pos = 0;

    while pos < data.len() {
        let (tag, wire, advance) = read_varint(data, pos)?;
        pos += advance;
        match (tag, wire) {
            (1, 2) => {
                let (len, a) = read_raw_varint(data, pos)?; pos += a;
                let bytes = &data[pos..pos + len as usize]; pos += len as usize;
                // Encode as base32
                secret_b32 = base32::encode(base32::Alphabet::RFC4648 { padding: false }, bytes);
            }
            (2, 2) => {
                let (len, a) = read_raw_varint(data, pos)?; pos += a;
                name = String::from_utf8_lossy(&data[pos..pos+len as usize]).to_string();
                pos += len as usize;
            }
            (3, 2) => {
                let (len, a) = read_raw_varint(data, pos)?; pos += a;
                issuer = String::from_utf8_lossy(&data[pos..pos+len as usize]).to_string();
                pos += len as usize;
            }
            (_, 0) => { let (_, a) = read_raw_varint(data, pos)?; pos += a; }
            (_, 1) => { pos += 8; }
            (_, 2) => { let (l, a) = read_raw_varint(data, pos)?; pos += a + l as usize; }
            (_, 5) => { pos += 4; }
            _ => break,
        }
    }

    if secret_b32.is_empty() { bail!("No secret in OTP parameters"); }
    let uri = format!(
        "otpauth://totp/{}?secret={}&issuer={}",
        urlencoding::encode(&name),
        secret_b32,
        urlencoding::encode(&issuer)
    );
    Ok((name, uri))
}

fn read_varint(data: &[u8], pos: usize) -> Result<(u64, u64, usize)> {
    let (raw, advance) = read_raw_varint(data, pos)?;
    let tag = raw >> 3;
    let wire = raw & 0x7;
    Ok((tag, wire, advance))
}

fn read_raw_varint(data: &[u8], mut pos: usize) -> Result<(u64, usize)> {
    let mut result = 0u64;
    let mut shift = 0u32;
    let start = pos;
    loop {
        if pos >= data.len() { bail!("Unexpected end of protobuf data"); }
        let b = data[pos]; pos += 1;
        result |= ((b & 0x7F) as u64) << shift;
        if b & 0x80 == 0 { break; }
        shift += 7;
        if shift >= 64 { bail!("Varint too long"); }
    }
    Ok((result, pos - start))
}
