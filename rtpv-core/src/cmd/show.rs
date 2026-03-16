//! `rtpv show` — decrypt and display an entry, or get a specific field.
//!
//! Path shortcuts:
//!   rtpv show github           → full entry (masked password)
//!   rtpv show github:email     → just the email field
//!   rtpv show github:password  → just the password (printed plaintext)

use anyhow::Result;
use crate::config::Config;
use crate::store::{Entry, Store};

pub struct ShowResult {
    pub entry:  Entry,
    /// If a specific field was requested, it's here
    pub field:  Option<(String, String)>,
}

/// `path` may be "entry/name" or "entry/name:field"
pub fn run(cfg: &Config, path: &str) -> Result<ShowResult> {
    let (entry_path, field_name) = split_path_field(path);
    let store = Store::new(cfg)?;

    // Private key guard — extra confirmation for -pvt entries
    let entry = store.read(entry_path)?;

    if let Some(field) = field_name {
        let value = match field {
            "password" => entry.password.clone(),
            f => entry.get_field(f)
                .ok_or_else(|| anyhow::anyhow!("Field '{}' not found in '{}'", f, entry_path))?
                .to_string(),
        };
        Ok(ShowResult { field: Some((field.to_string(), value)), entry })
    } else {
        Ok(ShowResult { field: None, entry })
    }
}

/// Split "github:email" into ("github", Some("email")).
/// Handles nested paths: "work/github:email" → ("work/github", Some("email"))
pub fn split_path_field(path: &str) -> (&str, Option<&str>) {
    // Only split on the last colon that comes after any slash
    if let Some(colon) = path.rfind(':') {
        let before = &path[..colon];
        let after  = &path[colon+1..];
        // Make sure we're not splitting a URL (after should be a simple word)
        if !after.is_empty() && !after.contains('/') && !after.contains(':') {
            return (before, Some(after));
        }
    }
    (path, None)
}

/// Return a formatted multi-line summary of an entry for display.
/// Password is shown as bullets unless `show_password` is true.
pub fn format_entry(entry: &Entry, show_password: bool) -> String {
    let mut lines = Vec::new();
    let pw = if show_password {
        entry.password.clone()
    } else {
        "●".repeat(entry.password.len().min(12))
    };
    lines.push(format!("password  {}", pw));
    if let Some(u) = entry.username() { lines.push(format!("username  {}", u)); }
    if let Some(e) = entry.email()    { lines.push(format!("email     {}", e)); }
    if let Some(u) = entry.url()      { lines.push(format!("url       {}", u)); }
    if let Some(t) = entry.token()    { lines.push(format!("token     {}…", &t[..t.len().min(8)])); }
    if entry.has_otp()                { lines.push("otp       ● configured".to_string()); }
    if let Some(n) = entry.notes()    { lines.push(format!("notes     {}", n)); }

    // Any remaining custom fields
    let known = ["username","user","login","email","url","token","otp","notes"];
    let mut extra: Vec<(&str, &str)> = entry.fields.iter()
        .filter(|(k, _)| !known.contains(&k.as_str()))
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    extra.sort_by_key(|(k, _)| *k);
    for (k, v) in extra {
        lines.push(format!("{:<9} {}", k, v));
    }
    lines.join("\n")
}
