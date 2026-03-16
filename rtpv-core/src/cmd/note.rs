//! `rtpv note` — create, show, edit, copy secure notes.
//!
//! Notes are regular entries where the `notes:` field holds long-form text.
//! The password field is left empty for pure notes.

use anyhow::{bail, Result};
use std::collections::HashMap;
use crate::config::Config;
use crate::store::{Entry, Store};

pub fn add(cfg: &Config, path: &str, content: &str) -> Result<Entry> {
    let store = Store::new(cfg)?;
    if store.exists(path) {
        bail!("Entry already exists: {}  (use: rtpv note edit {})", path, path);
    }
    let mut fields = HashMap::new();
    fields.insert("notes".to_string(), content.to_string());
    let entry = store.add(path, "", fields)?;
    if let Ok(git) = crate::git::StoreGit::open(&cfg.store_dir) {
        let _ = git.commit(&format!("Add note {}", path));
    }
    Ok(entry)
}

pub fn show(cfg: &Config, path: &str) -> Result<String> {
    let store = Store::new(cfg)?;
    let entry = store.read(path)?;
    Ok(entry.notes().unwrap_or("(no notes field)").to_string())
}

pub fn copy(cfg: &Config, path: &str) -> Result<()> {
    let store = Store::new(cfg)?;
    let entry = store.read(path)?;
    let notes = entry.notes()
        .ok_or_else(|| anyhow::anyhow!("No notes field in '{}'", path))?;
    crate::clipboard::copy_with_clear(notes, cfg.clip_timeout)?;
    Ok(())
}

pub fn edit(cfg: &Config, path: &str) -> Result<()> {
    // Re-use the main edit command — it opens $EDITOR on the full entry
    crate::cmd::edit::run(cfg, path)
}

/// List all entries that have a non-empty `notes:` field.
/// Since we'd need to decrypt everything, this is best-effort:
/// we only decrypt entries whose path hints at notes (contains "note" or "diary").
/// For a full scan pass `deep = true`.
pub fn list_note_entries(cfg: &Config, deep: bool) -> Result<Vec<String>> {
    let store = Store::new(cfg)?;
    let all = store.list()?;
    if !deep {
        return Ok(all.into_iter()
            .filter(|p| p.contains("note") || p.contains("diary") || p.contains("journal"))
            .collect());
    }
    let mut result = Vec::new();
    for path in all {
        if let Ok(entry) = store.read(&path) {
            if entry.notes().is_some() {
                result.push(path);
            }
        }
    }
    Ok(result)
}
