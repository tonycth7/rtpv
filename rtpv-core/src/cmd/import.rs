//! `rtpv import-*` — import from external password managers.
//!
//!   rtpv import bitwarden  export.json
//!   rtpv import firefox    logins.csv
//!   rtpv import chrome     passwords.csv
//!   rtpv import keepass    passwords.csv    (KeePass CSV export)
//!   rtpv import csv        generic.csv      (custom column mapping)

use std::collections::HashMap;
use anyhow::{Context, Result};
use serde::Deserialize;
use crate::config::Config;
use crate::store::Store;

pub struct ImportResult {
    pub imported: usize,
    pub skipped:  usize,
    pub errors:   Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Bitwarden JSON export
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Deserialize)]
struct BwExport {
    items: Vec<BwItem>,
}

#[derive(Deserialize)]
struct BwItem {
    name:     Option<String>,
    #[serde(rename = "type")]
    kind:     u8,
    login:    Option<BwLogin>,
    notes:    Option<String>,
    #[serde(rename = "folderId")]
    folder_id: Option<String>,
}

#[derive(Deserialize)]
struct BwLogin {
    username: Option<String>,
    password: Option<String>,
    uris:     Option<Vec<BwUri>>,
    totp:     Option<String>,
}

#[derive(Deserialize)]
struct BwUri { uri: Option<String> }

pub fn import_bitwarden(cfg: &Config, path: &str) -> Result<ImportResult> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("Cannot read {}", path))?;
    let export: BwExport = serde_json::from_str(&text)
        .context("Invalid Bitwarden JSON export")?;

    let store = Store::new(cfg)?;
    let mut result = ImportResult { imported: 0, skipped: 0, errors: Vec::new() };

    for item in export.items {
        // Only import logins (type 1)
        if item.kind != 1 { result.skipped += 1; continue; }

        let name = match item.name {
            Some(n) if !n.is_empty() => sanitize_path(&n),
            _ => { result.skipped += 1; continue; }
        };

        let login = match item.login {
            Some(l) => l,
            None    => { result.skipped += 1; continue; }
        };

        let password = login.password.unwrap_or_default();
        if password.is_empty() { result.skipped += 1; continue; }

        // Avoid collisions
        let path = unique_path(&store, &name);

        let mut fields = HashMap::new();
        if let Some(u) = login.username.filter(|s| !s.is_empty()) {
            fields.insert("username".to_string(), u);
        }
        if let Some(uri) = login.uris.and_then(|v| v.into_iter().find_map(|u| u.uri)) {
            fields.insert("url".to_string(), uri);
        }
        if let Some(n) = item.notes.filter(|s| !s.is_empty()) {
            fields.insert("notes".to_string(), n);
        }
        if let Some(t) = login.totp.filter(|s| !s.is_empty()) {
            let uri = if t.starts_with("otpauth://") { t }
                      else { format!("otpauth://totp/{}?secret={}", name, t) };
            fields.insert("otp".to_string(), uri);
        }

        match store.add(&path, &password, fields) {
            Ok(_)  => result.imported += 1,
            Err(e) => result.errors.push(format!("{}: {}", path, e)),
        }
    }

    if result.imported > 0 {
        if let Ok(git) = crate::git::StoreGit::open(&cfg.store_dir) {
            let _ = git.commit(&format!("Import {} entries from Bitwarden", result.imported));
        }
    }
    Ok(result)
}

// ─────────────────────────────────────────────────────────────────────────────
// Firefox CSV export
// ─────────────────────────────────────────────────────────────────────────────
pub fn import_firefox(cfg: &Config, csv_path: &str) -> Result<ImportResult> {
    // Firefox columns: url,username,password,httpRealm,formActionOrigin,guid,timeCreated,...
    import_csv_with_columns(cfg, csv_path, &CsvMapping {
        name_col:     None,  // derive from url
        url_col:      Some("url"),
        username_col: Some("username"),
        password_col: "password",
        notes_col:    None,
        prefix:       "firefox",
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Chrome CSV export
// ─────────────────────────────────────────────────────────────────────────────
pub fn import_chrome(cfg: &Config, csv_path: &str) -> Result<ImportResult> {
    // Chrome columns: name,url,username,password
    import_csv_with_columns(cfg, csv_path, &CsvMapping {
        name_col:     Some("name"),
        url_col:      Some("url"),
        username_col: Some("username"),
        password_col: "password",
        notes_col:    None,
        prefix:       "chrome",
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// KeePass CSV export
// ─────────────────────────────────────────────────────────────────────────────
pub fn import_keepass(cfg: &Config, csv_path: &str) -> Result<ImportResult> {
    // KeePass columns: Group,Title,Username,Password,URL,Notes
    import_csv_with_columns(cfg, csv_path, &CsvMapping {
        name_col:     Some("Title"),
        url_col:      Some("URL"),
        username_col: Some("Username"),
        password_col: "Password",
        notes_col:    Some("Notes"),
        prefix:       "",
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Generic CSV importer
// ─────────────────────────────────────────────────────────────────────────────
struct CsvMapping {
    name_col:     Option<&'static str>,
    url_col:      Option<&'static str>,
    username_col: Option<&'static str>,
    password_col: &'static str,
    notes_col:    Option<&'static str>,
    prefix:       &'static str,
}

fn import_csv_with_columns(cfg: &Config, csv_path: &str, mapping: &CsvMapping) -> Result<ImportResult> {
    let text = std::fs::read_to_string(csv_path)
        .with_context(|| format!("Cannot read {}", csv_path))?;

    let store = Store::new(cfg)?;
    let mut result = ImportResult { imported: 0, skipped: 0, errors: Vec::new() };

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(text.as_bytes());

    let headers = reader.headers()?.clone();

    let col = |name: &str| -> Option<usize> {
        headers.iter().position(|h| h.eq_ignore_ascii_case(name))
    };

    for record in reader.records() {
        let record = match record {
            Ok(r) => r,
            Err(e) => { result.errors.push(e.to_string()); continue; }
        };

        let get = |col_name: &str| -> Option<String> {
            col(col_name).and_then(|i| record.get(i))
                .filter(|v| !v.is_empty())
                .map(String::from)
        };

        let password = match col(mapping.password_col).and_then(|i| record.get(i)) {
            Some(pw) if !pw.is_empty() => pw.to_string(),
            _ => { result.skipped += 1; continue; }
        };

        // Derive entry name from name_col, then url, then index
        let raw_name = mapping.name_col
            .and_then(|c| get(c))
            .or_else(|| mapping.url_col.and_then(|c| get(c))
                .and_then(|u| url_to_name(&u)))
            .unwrap_or_else(|| format!("entry-{}", result.imported + result.skipped));

        let base = if mapping.prefix.is_empty() {
            sanitize_path(&raw_name)
        } else {
            format!("{}/{}", mapping.prefix, sanitize_path(&raw_name))
        };
        let entry_path = unique_path(&store, &base);

        let mut fields = HashMap::new();
        if let Some(c) = mapping.username_col { if let Some(u) = get(c) { fields.insert("username".to_string(), u); } }
        if let Some(c) = mapping.url_col      { if let Some(u) = get(c) { fields.insert("url".to_string(), u); } }
        if let Some(c) = mapping.notes_col    { if let Some(n) = get(c) { fields.insert("notes".to_string(), n); } }

        match store.add(&entry_path, &password, fields) {
            Ok(_)  => result.imported += 1,
            Err(e) => result.errors.push(format!("{}: {}", entry_path, e)),
        }
    }

    if result.imported > 0 {
        if let Ok(git) = crate::git::StoreGit::open(&cfg.store_dir) {
            let _ = git.commit(&format!("Import {} entries from CSV", result.imported));
        }
    }
    Ok(result)
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────
fn sanitize_path(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '/' || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_lowercase()
}

fn url_to_name(url: &str) -> Option<String> {
    url::Url::parse(url).ok()
        .and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string()))
}

fn unique_path(store: &Store, base: &str) -> String {
    if !store.exists(base) { return base.to_string(); }
    let mut i = 2;
    loop {
        let candidate = format!("{}-{}", base, i);
        if !store.exists(&candidate) { return candidate; }
        i += 1;
    }
}
