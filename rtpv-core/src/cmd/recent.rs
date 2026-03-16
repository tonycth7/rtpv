//! `rtpv recent` — list recently modified entries.
//!
//! Reads mtime from the filesystem — no decryption needed.

use std::path::PathBuf;
use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::config::Config;

pub struct RecentEntry {
    pub path:     String,
    pub modified: DateTime<Utc>,
}

pub fn run(cfg: &Config, count: usize) -> Result<Vec<RecentEntry>> {
    let mut entries: Vec<(PathBuf, DateTime<Utc>)> = Vec::new();

    for entry in walkdir::WalkDir::new(&cfg.store_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let ext = path.extension().and_then(|s| s.to_str());
        if !matches!(ext, Some("age") | Some("gpg")) { continue; }

        if let Ok(meta) = path.metadata() {
            if let Ok(modified) = meta.modified() {
                entries.push((path.to_path_buf(), DateTime::from(modified)));
            }
        }
    }

    // Sort newest first
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries.truncate(count);

    let results = entries.into_iter().filter_map(|(path, modified)| {
        let rel = path.strip_prefix(&cfg.store_dir).ok()?;
        let name = rel.with_extension("").to_string_lossy()
            .replace('\\', "/");
        Some(RecentEntry { path: name, modified })
    }).collect();

    Ok(results)
}
