//! `rtpv gpg` — store and restore GPG keypairs in the password store.
//!
//! Private keys are stored under `gpg/<keyid>` with the armored key as
//! the password field. The public key goes in `pub:` field, fingerprint in `fp:`.

use std::collections::HashMap;
use std::process::Command;
use anyhow::{bail, Context, Result};
use crate::config::Config;
use crate::store::Store;

const GPG_PREFIX: &str = "gpg/";

// ─────────────────────────────────────────────────────────────────────────────
// List
// ─────────────────────────────────────────────────────────────────────────────
pub fn list(cfg: &Config) -> Result<Vec<String>> {
    let store = Store::new(cfg)?;
    store.list_filtered(|p| p.starts_with(GPG_PREFIX))
}

// ─────────────────────────────────────────────────────────────────────────────
// Add — export from keyring and store
// ─────────────────────────────────────────────────────────────────────────────
pub struct AddResult {
    pub path:        String,
    pub fingerprint: String,
}

pub fn add(cfg: &Config, key_id: &str, name: Option<&str>) -> Result<AddResult> {
    // Export private key
    let priv_out = Command::new("gpg")
        .args(["--armor", "--export-secret-keys", "--", key_id])
        .output()
        .context("gpg not found — install gnupg")?;
    if !priv_out.status.success() || priv_out.stdout.is_empty() {
        bail!("gpg export failed for key '{}': {}",
            key_id, String::from_utf8_lossy(&priv_out.stderr));
    }
    let private_key = String::from_utf8_lossy(&priv_out.stdout).to_string();

    // Export public key
    let pub_out = Command::new("gpg")
        .args(["--armor", "--export", "--", key_id])
        .output()?;
    let public_key = String::from_utf8_lossy(&pub_out.stdout).to_string();

    // Get fingerprint
    let fp_out = Command::new("gpg")
        .args(["--fingerprint", "--with-colons", "--", key_id])
        .output()?;
    let fingerprint = String::from_utf8_lossy(&fp_out.stdout)
        .lines()
        .find(|l| l.starts_with("fpr:"))
        .and_then(|l| l.split(':').nth(9))
        .unwrap_or(key_id)
        .to_string();

    let entry_name = name.unwrap_or_else(|| {
        // Use last 8 chars of fingerprint as default name
        if fingerprint.len() >= 8 { &fingerprint[fingerprint.len()-8..] } else { key_id }
    });
    let path = format!("{}{}", GPG_PREFIX, entry_name);

    let store = Store::new(cfg)?;
    if store.exists(&path) {
        bail!("GPG key already stored at '{}' — delete it first", path);
    }

    let mut fields = HashMap::new();
    fields.insert("fp".to_string(),  fingerprint.clone());
    fields.insert("pub".to_string(), public_key.trim().to_string());

    store.add(&path, private_key.trim(), fields)?;

    if let Ok(git) = crate::git::StoreGit::open(&cfg.store_dir) {
        let _ = git.commit(&format!("Store GPG key: {}", entry_name));
    }
    Ok(AddResult { path, fingerprint })
}

// ─────────────────────────────────────────────────────────────────────────────
// Restore — import back into keyring
// ─────────────────────────────────────────────────────────────────────────────
pub fn restore(cfg: &Config, name: &str) -> Result<String> {
    let store = Store::new(cfg)?;
    let path = if name.starts_with(GPG_PREFIX) { name.to_string() }
               else { format!("{}{}", GPG_PREFIX, name) };
    let entry = store.read(&path)?;

    // Import private key
    use std::io::Write;
    let mut child = Command::new("gpg")
        .args(["--import"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    child.stdin.as_mut()
        .ok_or_else(|| anyhow::anyhow!("gpg stdin unavailable"))?
        .write_all(entry.password.as_bytes())?;
    drop(child.stdin.take());
    let out = child.wait_with_output()?;

    if !out.status.success() {
        bail!("gpg import failed: {}", String::from_utf8_lossy(&out.stderr));
    }

    let fp = entry.get_field("fp").unwrap_or("unknown").to_string();
    Ok(fp)
}

// ─────────────────────────────────────────────────────────────────────────────
// Show fingerprint
// ─────────────────────────────────────────────────────────────────────────────
pub fn fingerprint(cfg: &Config, name: &str) -> Result<String> {
    let store = Store::new(cfg)?;
    let path = if name.starts_with(GPG_PREFIX) { name.to_string() }
               else { format!("{}{}", GPG_PREFIX, name) };
    let entry = store.read(&path)?;
    entry.get_field("fp")
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("No fingerprint stored for '{}'", name))
}

// ─────────────────────────────────────────────────────────────────────────────
// Copy public key to clipboard
// ─────────────────────────────────────────────────────────────────────────────
pub fn copy_pubkey(cfg: &Config, name: &str) -> Result<()> {
    let store = Store::new(cfg)?;
    let path = if name.starts_with(GPG_PREFIX) { name.to_string() }
               else { format!("{}{}", GPG_PREFIX, name) };
    let entry = store.read(&path)?;
    let pub_key = entry.get_field("pub")
        .ok_or_else(|| anyhow::anyhow!("No public key stored for '{}'", name))?;
    crate::clipboard::copy(pub_key)?;
    Ok(())
}
