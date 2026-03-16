//! `rtpv backup` — create an age-encrypted tar archive of the store.
//!
//! The backup is a single `.tar.age` file — decrypt with:
//!   rage -d backup.tar.age | tar xf -

use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::config::Config;

pub struct BackupResult {
    pub path:          PathBuf,
    pub entries:       usize,
    pub size_bytes:    usize,
}

pub fn run(cfg: &Config, output: Option<&str>) -> Result<BackupResult> {
    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let filename  = format!("rtpv-backup-{}.tar.age", timestamp);
    let dest = output
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default().join(&filename));

    // Count entries
    let mut entry_count = 0usize;

    // Build tar in memory
    let mut tar_data = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_data);
        // Add all encrypted files
        for entry in walkdir::WalkDir::new(&cfg.store_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            let ext = path.extension().and_then(|s| s.to_str());
            if !matches!(ext, Some("age") | Some("gpg")) { continue; }

            let rel = path.strip_prefix(&cfg.store_dir)
                .context("strip_prefix failed")?;
            builder.append_path_with_name(path, rel)?;
            entry_count += 1;
        }
        // Also include .gpg-id if present
        let gpg_id = cfg.store_dir.join(".gpg-id");
        if gpg_id.exists() {
            builder.append_path_with_name(&gpg_id, ".gpg-id")?;
        }
        builder.finish()?;
    }

    let raw_size = tar_data.len();

    // Encrypt the tar with age
    let backend = crate::crypto::build_backend(cfg)?;
    let encrypted = backend.encrypt(&tar_data)
        .context("Failed to encrypt backup")?;

    std::fs::write(&dest, &encrypted)
        .with_context(|| format!("Failed to write backup to {:?}", dest))?;

    Ok(BackupResult {
        path:       dest,
        entries:    entry_count,
        size_bytes: raw_size,
    })
}

/// Restore from a backup archive.
/// Decrypts and extracts to `target_dir` (default: current directory).
pub fn restore(cfg: &Config, backup_path: &str, target_dir: Option<&str>) -> Result<usize> {
    let dest = target_dir
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let ciphertext = std::fs::read(backup_path)
        .with_context(|| format!("Cannot read backup: {}", backup_path))?;

    let backend = crate::crypto::build_backend(cfg)?;
    let tar_data = backend.decrypt(&ciphertext)
        .context("Failed to decrypt backup")?;

    let mut archive = tar::Archive::new(std::io::Cursor::new(tar_data));
    let mut count = 0usize;
    for entry in archive.entries()? {
        let mut entry = entry?;
        entry.unpack_in(&dest)?;
        count += 1;
    }
    Ok(count)
}
