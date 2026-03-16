use notify_rust;
// `rtpv otp` — TOTP/HOTP commands.
//
//   rtpv otp [name]         copy current code for entry
//   rtpv otp show [name]    print code + countdown
//   rtpv otp list           list all entries with OTP configured
//   rtpv otp import <uri>   import an otpauth:// URI into an entry

use anyhow::{bail, Result};
use crate::config::Config;
use crate::store::Store;
use crate::otp::{self, OtpState};

// ─────────────────────────────────────────────────────────────────────────────
// Copy current code to clipboard
// ─────────────────────────────────────────────────────────────────────────────
pub fn copy_code(cfg: &Config, name: &str) -> Result<OtpState> {
    let store = Store::new(cfg)?;
    let entry = store.read(name)?;

    let uri = entry.otp_uri()
        .ok_or_else(|| anyhow::anyhow!("No OTP configured for '{}'", name))?;

    let state = otp::code_for_uri(uri)?;
    crate::clipboard::copy_with_clear(&state.code, cfg.clip_timeout)?;

    if cfg.notify {
        let _ = notify_rust::Notification::new()
            .summary("rtpv OTP")
            .body(&format!(
                "Copied OTP for {} ({} seconds remaining)",
                name, state.remaining
            ))
            .timeout(3000)
            .show();
    }

    Ok(state)
}

// ─────────────────────────────────────────────────────────────────────────────
// List all OTP entries
// ─────────────────────────────────────────────────────────────────────────────
pub struct OtpEntry {
    pub path:  String,
    pub state: OtpState,
}

/// Returns all entries that have an OTP field, with live codes.
/// We decrypt each entry — this is intentionally lazy and only used for
/// the picker, so we skip entries that fail to decrypt.
pub fn list_all(cfg: &Config) -> Result<Vec<OtpEntry>> {
    let store = Store::new(cfg)?;
    let entries = store.list()?;
    let mut out = Vec::new();

    for path in entries {
        if let Ok(entry) = store.read(&path) {
            if let Some(uri) = entry.otp_uri() {
                if let Ok(state) = otp::code_for_uri(uri) {
                    out.push(OtpEntry { path, state });
                }
            }
        }
    }
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────
// Import otpauth:// URI into an existing entry
// ─────────────────────────────────────────────────────────────────────────────
pub fn import_uri(cfg: &Config, name: &str, uri: &str) -> Result<()> {
    // Validate the URI first
    otp::from_uri(uri)?;

    let store = Store::new(cfg)?;
    if !store.exists(name) {
        bail!("Entry not found: {}  (create it first with: rtpv add {})", name, name);
    }
    store.set_field(name, "otp", uri)?;

    if let Ok(git) = crate::git::StoreGit::open(&cfg.store_dir) {
        let _ = git.commit(&format!("Add OTP for {}", name));
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Import from Google Authenticator migration QR
// ─────────────────────────────────────────────────────────────────────────────
pub struct MigrationImportResult {
    pub imported: usize,
    pub failed:   usize,
    pub entries:  Vec<(String, String)>, // (name, uri)
}

pub fn import_migration_qr(cfg: &Config, payload: &str, prefix: &str) -> Result<MigrationImportResult> {
    let pairs = otp::parse_google_migration_qr(payload)?;
    let store = Store::new(cfg)?;
    let mut imported = 0;
    let mut failed = 0;
    let mut entries = Vec::new();

    for (label, uri) in &pairs {
        // Sanitize label into a store path
        let path = format!("{}/{}", prefix.trim_end_matches('/'),
            label.replace(['/', '\\', ':', ' '], "_").to_lowercase());

        if store.exists(&path) {
            // Just add the OTP field to the existing entry
            if store.set_field(&path, "otp", uri).is_ok() {
                imported += 1;
                entries.push((path, uri.clone()));
            } else {
                failed += 1;
            }
        } else {
            // Create a minimal entry with just the OTP
            let mut fields = std::collections::HashMap::new();
            fields.insert("otp".to_string(), uri.clone());
            if store.add(&path, "", fields).is_ok() {
                imported += 1;
                entries.push((path, uri.clone()));
            } else {
                failed += 1;
            }
        }
    }

    if imported > 0 {
        if let Ok(git) = crate::git::StoreGit::open(&cfg.store_dir) {
            let _ = git.commit(&format!("Import {} OTP entries from migration QR", imported));
        }
    }

    Ok(MigrationImportResult { imported, failed, entries })
}
