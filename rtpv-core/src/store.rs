//! Password store — entry read/write, field parsing, directory listing.
//!
//! Entry format (same as pass):
//! ```text
//! <password>
//! username: alice
//! email: alice@example.com
//! url: https://example.com
//! otp: otpauth://totp/...
//! notes: anything here
//! ```
//!
//! The first line is always the password.
//! All subsequent lines are `key: value`.
//! Field names are case-insensitive.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use crate::config::Config;
use crate::crypto::Backend;

// ─────────────────────────────────────────────────────────────────────────────
// Entry struct
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// Relative path from store root, without extension (e.g. "github" or "work/slack")
    pub path:     String,
    /// The first line — the password
    pub password: String,
    /// Key→value fields from lines 2+
    pub fields:   HashMap<String, String>,
    /// Last modification time (from filesystem)
    pub modified: Option<DateTime<Utc>>,
}

impl Entry {
    /// Parse decrypted plaintext into an Entry.
    pub fn parse(path: &str, plaintext: &str) -> Self {
        let mut lines = plaintext.lines();
        let password = lines.next().unwrap_or("").to_string();
        let mut fields = HashMap::new();
        for line in lines {
            if let Some((k, v)) = line.split_once(':') {
                fields.insert(k.trim().to_lowercase(), v.trim().to_string());
            }
        }
        Self { path: path.to_string(), password, fields, modified: None }
    }

    /// Serialize back to the on-disk text format.
    pub fn to_plaintext(&self) -> String {
        let mut out = self.password.clone();
        out.push('\n');
        // Write fields in a stable order
        let mut keys: Vec<&String> = self.fields.keys().collect();
        keys.sort();
        for k in keys {
            out.push_str(&format!("{}: {}\n", k, self.fields[k]));
        }
        out
    }

    pub fn username(&self) -> Option<&str> {
        self.fields.get("username").map(String::as_str)
            .or_else(|| self.fields.get("user").map(String::as_str))
            .or_else(|| self.fields.get("login").map(String::as_str))
    }

    pub fn email(&self)    -> Option<&str> { self.fields.get("email").map(String::as_str) }
    pub fn url(&self)      -> Option<&str> { self.fields.get("url").map(String::as_str) }
    pub fn otp_uri(&self)  -> Option<&str> { self.fields.get("otp").map(String::as_str) }
    pub fn token(&self)    -> Option<&str> { self.fields.get("token").map(String::as_str) }
    pub fn notes(&self)    -> Option<&str> { self.fields.get("notes").map(String::as_str) }

    pub fn get_field(&self, field: &str) -> Option<&str> {
        self.fields.get(&field.to_lowercase()).map(String::as_str)
    }

    pub fn set_field(&mut self, key: &str, value: &str) {
        self.fields.insert(key.to_lowercase(), value.to_string());
    }

    pub fn has_otp(&self) -> bool {
        self.otp_uri().is_some()
    }

    pub fn is_private_key(&self) -> bool {
        self.path.ends_with("-pvt")
            || self.password.contains("-----BEGIN")
            || self.fields.values().any(|v| v.contains("-----BEGIN"))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Store
// ─────────────────────────────────────────────────────────────────────────────
pub struct Store {
    pub root: PathBuf,
    pub backend: Box<dyn Backend>,
    /// Extension: "age" or "gpg"
    ext: &'static str,
}

impl Store {
    pub fn new(cfg: &Config) -> Result<Self> {
        let backend = crate::crypto::build_backend(cfg)?;
        let ext = backend.extension();
        Ok(Self { root: cfg.store_dir.clone(), backend, ext })
    }

    // ── paths ────────────────────────────────────────────────────────────────
    fn entry_path(&self, name: &str) -> PathBuf {
        self.root.join(name).with_extension(self.ext)
    }

    pub fn exists(&self, name: &str) -> bool {
        // Also check legacy .gpg extension if using age backend
        let p = self.entry_path(name);
        if p.exists() { return true; }
        self.root.join(name).with_extension("gpg").exists()
    }

    // ── list ─────────────────────────────────────────────────────────────────
    /// List all entry paths relative to store root (no extension).
    pub fn list(&self) -> Result<Vec<String>> {
        let mut entries = Vec::new();
        for entry in walkdir::WalkDir::new(&self.root)
            .follow_links(false)
            .sort_by_file_name()
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            let ext = path.extension().and_then(|s| s.to_str());
            if !matches!(ext, Some("age") | Some("gpg")) { continue; }
            if let Ok(rel) = path.strip_prefix(&self.root) {
                let s = rel.with_extension("").to_string_lossy().to_string();
                // Normalize path separators on Windows
                let s = s.replace('\\', "/");
                entries.push(s);
            }
        }
        Ok(entries)
    }

    /// List entries filtered by a predicate on their names.
    pub fn list_filtered(&self, pred: impl Fn(&str) -> bool) -> Result<Vec<String>> {
        Ok(self.list()?.into_iter().filter(|n| pred(n)).collect())
    }

    // ── read ─────────────────────────────────────────────────────────────────
    pub fn read(&self, name: &str) -> Result<Entry> {
        // Try preferred extension first, then fallback to .gpg
        let path = self.entry_path(name);
        let (ciphertext, _path) = if path.exists() {
            (std::fs::read(&path)?, path)
        } else {
            let gpg = self.root.join(name).with_extension("gpg");
            if gpg.exists() {
                (std::fs::read(&gpg)?, gpg)
            } else {
                bail!("Entry not found: {}", name);
            }
        };

        let plaintext_bytes = self.backend.decrypt(&ciphertext)
            .with_context(|| format!("Decryption failed for '{}'", name))?;
        let plaintext = String::from_utf8_lossy(&plaintext_bytes).into_owned();

        let mut entry = Entry::parse(name, &plaintext);

        // Grab mtime from filesystem
        if let Ok(meta) = std::fs::metadata(self.entry_path(name)) {
            if let Ok(modified) = meta.modified() {
                entry.modified = Some(DateTime::from(modified));
            }
        }

        Ok(entry)
    }

    /// Read only a single field without storing the full entry.
    pub fn read_field(&self, name: &str, field: &str) -> Result<String> {
        let entry = self.read(name)?;
        match field {
            "password" => Ok(entry.password.clone()),
            f => entry.get_field(f)
                .map(String::from)
                .ok_or_else(|| anyhow::anyhow!("Field '{}' not found in '{}'", f, name)),
        }
    }

    // ── write ────────────────────────────────────────────────────────────────
    pub fn write(&self, entry: &Entry) -> Result<()> {
        let plaintext = entry.to_plaintext();
        let ciphertext = self.backend.encrypt(plaintext.as_bytes())
            .with_context(|| format!("Encryption failed for '{}'", entry.path))?;

        let dest = self.entry_path(&entry.path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, &ciphertext)?;
        Ok(())
    }

    // ── add ──────────────────────────────────────────────────────────────────
    pub fn add(&self, name: &str, password: &str, fields: HashMap<String, String>) -> Result<Entry> {
        if self.exists(name) {
            bail!("Entry already exists: {}  (use: rtpv edit {})", name, name);
        }
        let entry = Entry { path: name.to_string(), password: password.to_string(), fields, modified: None };
        self.write(&entry)?;
        Ok(entry)
    }

    // ── delete ───────────────────────────────────────────────────────────────
    pub fn remove(&self, name: &str) -> Result<()> {
        let path = self.entry_path(name);
        if !path.exists() {
            bail!("Entry not found: {}", name);
        }
        std::fs::remove_file(&path)?;
        // Clean up empty parent directories
        if let Some(parent) = path.parent() {
            if parent != self.root && parent.read_dir().map(|mut d| d.next().is_none()).unwrap_or(false) {
                let _ = std::fs::remove_dir(parent);
            }
        }
        Ok(())
    }

    // ── rename ───────────────────────────────────────────────────────────────
    pub fn rename(&self, src: &str, dst: &str) -> Result<()> {
        if self.exists(dst) { bail!("Destination already exists: {}", dst); }
        let src_path = self.entry_path(src);
        let dst_path = self.entry_path(dst);
        if let Some(p) = dst_path.parent() { std::fs::create_dir_all(p)?; }
        std::fs::rename(&src_path, &dst_path)?;
        Ok(())
    }

    // ── clone ────────────────────────────────────────────────────────────────
    pub fn clone_entry(&self, src: &str, dst: &str) -> Result<()> {
        if self.exists(dst) { bail!("Destination already exists: {}", dst); }
        let src_path = self.entry_path(src);
        let dst_path = self.entry_path(dst);
        if let Some(p) = dst_path.parent() { std::fs::create_dir_all(p)?; }
        std::fs::copy(&src_path, &dst_path)?;
        Ok(())
    }

    // ── set field ────────────────────────────────────────────────────────────
    pub fn set_field(&self, name: &str, key: &str, value: &str) -> Result<()> {
        let mut entry = self.read(name)?;
        if key == "password" {
            entry.password = value.to_string();
        } else {
            entry.set_field(key, value);
        }
        self.write(&entry)
    }

    // ── hooks ────────────────────────────────────────────────────────────────
    pub fn run_hook(&self, hooks_dir: &Path, event: &str) {
        crate::hooks::run_bare(hooks_dir, event);
    }

    pub fn run_hook_with_path(&self, hooks_dir: &Path, event: &str, entry_path: &str) {
        crate::hooks::run(hooks_dir, event, entry_path);
    }

    // ── stats ────────────────────────────────────────────────────────────────
    pub fn stats(&self) -> Result<StoreStats> {
        let entries = self.list()?;
        let total = entries.len();
        let otp_count = entries.iter()
            .filter(|n| n.contains("otp") || {
                // Peek at the entry name for known OTP patterns
                false // we'd need to decrypt all to check fields — do lazy
            })
            .count();
        Ok(StoreStats { total, otp_count })
    }
}

#[derive(Debug)]
pub struct StoreStats {
    pub total:     usize,
    pub otp_count: usize,
}
