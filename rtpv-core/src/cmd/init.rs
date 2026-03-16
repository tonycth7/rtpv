//! `rtpv init` — initialize a new store (age or gpg backend).

use std::path::Path;
use anyhow::{Result, bail};
use crate::config::{Config, CryptoBackend};

pub struct InitResult {
    pub backend:    CryptoBackend,
    pub store_dir:  std::path::PathBuf,
    pub public_key: String,
    pub created:    bool,
}

pub fn run(cfg: &Config, force: bool) -> Result<InitResult> {
    match &cfg.crypto_backend {
        CryptoBackend::Age => init_age(cfg, force),
        CryptoBackend::Gpg => init_gpg(cfg),
    }
}

fn init_age(cfg: &Config, force: bool) -> Result<InitResult> {
    std::fs::create_dir_all(&cfg.store_dir)?;
    std::fs::create_dir_all(&cfg.config_dir)?;

    let created = if cfg.age_identity.exists() && !force {
        false
    } else {
        #[cfg(feature = "age-native")]
        { crate::crypto::AgeBackend::init(&cfg.age_identity, &cfg.config_dir)?; }
        true
    };

    // Read back the pubkey from recipients.txt
    let rec_path = cfg.config_dir.join("recipients.txt");
    let public_key = if rec_path.exists() {
        std::fs::read_to_string(&rec_path)?
            .lines()
            .find(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .unwrap_or("")
            .to_string()
    } else {
        String::new()
    };

    Ok(InitResult {
        backend:   CryptoBackend::Age,
        store_dir: cfg.store_dir.clone(),
        public_key,
        created,
    })
}

fn init_gpg(cfg: &Config) -> Result<InitResult> {
    let gpg_id = cfg.store_dir.join(".gpg-id");
    if !gpg_id.exists() {
        bail!(
            "No .gpg-id found in {:?}\n  \
             For GPG mode run: rtpv init --backend gpg <GPG_KEY_ID>",
            cfg.store_dir
        );
    }
    let key_id = std::fs::read_to_string(&gpg_id)?
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .to_string();

    Ok(InitResult {
        backend:   CryptoBackend::Gpg,
        store_dir: cfg.store_dir.clone(),
        public_key: key_id,
        created:   false,
    })
}

/// Write a .gpg-id file (used when initializing GPG backend from scratch).
pub fn write_gpg_id(store_dir: &Path, key_id: &str) -> Result<()> {
    std::fs::create_dir_all(store_dir)?;
    let path = store_dir.join(".gpg-id");
    std::fs::write(path, format!("{}\n", key_id))?;
    Ok(())
}
