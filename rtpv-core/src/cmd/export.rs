//! `rtpv export` — export the store to external formats.
//!
//!   rtpv export bitwarden   → Bitwarden-compatible JSON
//!   rtpv export backup      → age-encrypted tar archive

use anyhow::Result;
use serde::Serialize;
use crate::config::Config;
use crate::store::Store;

// ─────────────────────────────────────────────────────────────────────────────
// Bitwarden JSON export
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Serialize)]
struct BwExport {
    encrypted: bool,
    items:     Vec<BwItem>,
}

#[derive(Serialize)]
struct BwItem {
    #[serde(rename = "type")]
    kind:     u8,
    name:     String,
    login:    BwLogin,
    notes:    Option<String>,
}

#[derive(Serialize)]
struct BwLogin {
    username: Option<String>,
    password: String,
    uris:     Vec<BwUri>,
    totp:     Option<String>,
}

#[derive(Serialize)]
struct BwUri { uri: String }

pub fn export_bitwarden(cfg: &Config) -> Result<String> {
    let store = Store::new(cfg)?;
    let paths = store.list()?;
    let mut items = Vec::new();

    for path in &paths {
        if let Ok(entry) = store.read(path) {
            items.push(BwItem {
                kind:  1,
                name:  entry.path.clone(),
                login: BwLogin {
                    username: entry.username().map(String::from),
                    password: entry.password.clone(),
                    uris:     entry.url().map(|u| vec![BwUri { uri: u.to_string() }])
                               .unwrap_or_default(),
                    totp:     entry.otp_uri().map(String::from),
                },
                notes: entry.notes().map(String::from),
            });
        }
    }

    let export = BwExport { encrypted: false, items };
    Ok(serde_json::to_string_pretty(&export)?)
}

// ─────────────────────────────────────────────────────────────────────────────
// Encrypted backup archive
// ─────────────────────────────────────────────────────────────────────────────
pub fn export_backup(cfg: &Config, output_path: &str) -> Result<usize> {
    // Create a tar of all encrypted files, then age-encrypt the tar
    let mut tar_data = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_data);
        builder.append_dir_all(".", &cfg.store_dir)?;
        builder.finish()?;
    }

    let backend = crate::crypto::build_backend(cfg)?;
    let encrypted = backend.encrypt(&tar_data)?;
    std::fs::write(output_path, &encrypted)?;

    Ok(tar_data.len())
}
