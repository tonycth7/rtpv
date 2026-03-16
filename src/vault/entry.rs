// vault/entry.rs — typed entry definitions
// Each entry is age-encrypted TOML on disk.
// Unknown fields are preserved in `extra` so we never lose data.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Entry type enum ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    #[default]
    Login,
    Note,
    Ssh,
    Gpg,
    Card,
    Env,
    Wifi,
    Database,
    License,
}

impl EntryKind {
    pub fn display(&self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::Note => "note",
            Self::Ssh => "ssh",
            Self::Gpg => "gpg",
            Self::Card => "card",
            Self::Env => "env / api key",
            Self::Wifi => "wifi",
            Self::Database => "database",
            Self::License => "license",
        }
    }

    pub fn all_names() -> &'static [&'static str] {
        &[
            "login", "note", "ssh", "gpg", "card", "env", "wifi", "database", "license",
        ]
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "login" | "web" | "web-login" => Self::Login,
            "note" | "text" => Self::Note,
            "ssh" => Self::Ssh,
            "gpg" => Self::Gpg,
            "card" | "credit-card" => Self::Card,
            "env" | "api" | "api-key" => Self::Env,
            "wifi" | "network" => Self::Wifi,
            "db" | "database" => Self::Database,
            "license" | "software" => Self::License,
            _ => Self::Login,
        }
    }
}

// ── Meta section ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryMeta {
    pub kind: EntryKind,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    /// Optional tags for filtering
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Optional favourite flag
    #[serde(default)]
    pub starred: bool,
}

impl Default for EntryMeta {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            kind: EntryKind::Login,
            created: now,
            modified: now,
            tags: vec![],
            starred: false,
        }
    }
}

// ── Fields section ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EntryFields {
    // Universal
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub otp: Option<String>, // otpauth:// URI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>, // API key / access token

    // SSH
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pub_key: Option<String>, // public key armored
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pvt_key: Option<String>, // private key armored
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<String>,

    // Card
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvv: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub holder: Option<String>,

    // Database
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,

    // Wifi
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<String>,

    // License
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    // Catch-all for unknown fields (forward compatibility)
    #[serde(flatten)]
    pub extra: HashMap<String, String>,
}

impl EntryFields {
    /// Get any field by name — checks known fields first, then extra
    pub fn get(&self, name: &str) -> Option<&str> {
        match name {
            "password" => self.password.as_deref(),
            "username" | "user" | "login" => self.username.as_deref(),
            "email" => self.email.as_deref(),
            "url" => self.url.as_deref(),
            "notes" => self.notes.as_deref(),
            "otp" => self.otp.as_deref(),
            "token" => self.token.as_deref(),
            "pub_key" | "pub" => self.pub_key.as_deref(),
            "pvt_key" | "pvt" => self.pvt_key.as_deref(),
            "host" => self.host.as_deref(),
            "port" => self.port.as_deref(),
            "number" => self.number.as_deref(),
            "expiry" => self.expiry.as_deref(),
            "cvv" => self.cvv.as_deref(),
            "holder" => self.holder.as_deref(),
            "database" | "db" => self.database.as_deref(),
            "ssid" => self.ssid.as_deref(),
            "security" => self.security.as_deref(),
            "product" => self.product.as_deref(),
            "key" => self.key.as_deref(),
            other => self.extra.get(other).map(String::as_str),
        }
    }

    /// Set a field by name
    pub fn set(&mut self, name: &str, value: String) {
        match name {
            "password" => self.password = Some(value),
            "username" | "user" => self.username = Some(value),
            "email" => self.email = Some(value),
            "url" => self.url = Some(value),
            "notes" => self.notes = Some(value),
            "otp" => self.otp = Some(value),
            "token" => self.token = Some(value),
            "pub_key" | "pub" => self.pub_key = Some(value),
            "pvt_key" | "pvt" => self.pvt_key = Some(value),
            "host" => self.host = Some(value),
            "port" => self.port = Some(value),
            "number" => self.number = Some(value),
            "expiry" => self.expiry = Some(value),
            "cvv" => self.cvv = Some(value),
            "holder" => self.holder = Some(value),
            "database" | "db" => self.database = Some(value),
            "ssid" => self.ssid = Some(value),
            "security" => self.security = Some(value),
            "product" => self.product = Some(value),
            "key" => self.key = Some(value),
            other => {
                self.extra.insert(other.to_string(), value);
            }
        }
    }

    /// Primary secret for this entry type
    pub fn primary(&self, kind: &EntryKind) -> Option<&str> {
        match kind {
            EntryKind::Card => self.number.as_deref(),
            EntryKind::Ssh => self.pvt_key.as_deref(),
            EntryKind::Gpg => self.pvt_key.as_deref(),
            EntryKind::Env => self.token.as_deref().or(self.password.as_deref()),
            EntryKind::License => self.key.as_deref(),
            _ => self.password.as_deref(),
        }
    }

    /// Display-safe summary line (no secrets)
    pub fn summary(&self, kind: &EntryKind) -> String {
        match kind {
            EntryKind::Login => {
                let u = self.username.as_deref().unwrap_or("");
                let e = self.email.as_deref().unwrap_or("");
                if !u.is_empty() {
                    format!("user: {}", u)
                } else if !e.is_empty() {
                    format!("email: {}", e)
                } else {
                    "no username".to_string()
                }
            }
            EntryKind::Note => self
                .notes
                .as_deref()
                .map(|n| n.lines().next().unwrap_or("").chars().take(60).collect())
                .unwrap_or_default(),
            EntryKind::Ssh | EntryKind::Gpg => {
                let host = self.host.as_deref().unwrap_or("");
                let user = self.username.as_deref().unwrap_or("");
                if !host.is_empty() {
                    format!("{}@{}", user, host)
                } else {
                    "key stored".to_string()
                }
            }
            EntryKind::Card => {
                let last4 = self
                    .number
                    .as_deref()
                    .map(|n| n.chars().filter(|c| c.is_ascii_digit()).collect::<String>())
                    .and_then(|d| {
                        if d.len() >= 4 {
                            Some(d[d.len() - 4..].to_string())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "????".to_string());
                let holder = self.holder.as_deref().unwrap_or("");
                format!("•••• {}  {}", last4, holder)
            }
            EntryKind::Env => {
                let svc = self.extra.get("service").map(String::as_str).unwrap_or("");
                let url = self.url.as_deref().unwrap_or("");
                if !svc.is_empty() {
                    svc.to_string()
                } else if !url.is_empty() {
                    url.to_string()
                } else {
                    "api key".to_string()
                }
            }
            EntryKind::Wifi => self.ssid.as_deref().unwrap_or("no SSID").to_string(),
            EntryKind::Database => {
                let h = self.host.as_deref().unwrap_or("");
                let d = self.database.as_deref().unwrap_or("");
                format!("{}/{}", h, d)
            }
            EntryKind::License => self
                .product
                .as_deref()
                .unwrap_or("software license")
                .to_string(),
        }
    }

    pub fn has_otp(&self) -> bool {
        self.otp.as_deref().map(|s| !s.is_empty()).unwrap_or(false)
    }
}

// ── Full entry ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub meta: EntryMeta,
    pub fields: EntryFields,
}

impl Entry {
    pub fn new(kind: EntryKind) -> Self {
        Self {
            meta: EntryMeta {
                kind,
                ..Default::default()
            },
            fields: Default::default(),
        }
    }

    pub fn touch(&mut self) {
        self.meta.modified = Utc::now();
    }

    pub fn to_toml(&self) -> anyhow::Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    pub fn from_toml(s: &str) -> anyhow::Result<Self> {
        Ok(toml::from_str(s)?)
    }

    /// Try to import a pass-style `key: value` plaintext entry
    pub fn from_pass_plaintext(text: &str) -> Self {
        let mut entry = Entry::new(EntryKind::Login);
        let mut lines = text.lines();

        // First line = password
        if let Some(first) = lines.next() {
            let first = first.trim();
            if !first.is_empty() {
                entry.fields.password = Some(first.to_string());
            }
        }

        // Remaining lines = key: value
        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some((k, v)) = line.split_once(':') {
                entry.fields.set(k.trim(), v.trim().to_string());
            }
        }

        // Detect type from fields
        if entry.fields.pub_key.is_some() || entry.fields.pvt_key.is_some() {
            entry.meta.kind = EntryKind::Ssh;
        } else if entry.fields.number.is_some() {
            entry.meta.kind = EntryKind::Card;
        } else if entry.fields.token.is_some() && entry.fields.password.is_none() {
            entry.meta.kind = EntryKind::Env;
        } else if entry.fields.ssid.is_some() {
            entry.meta.kind = EntryKind::Wifi;
        } else if entry.fields.database.is_some() {
            entry.meta.kind = EntryKind::Database;
        }

        entry
    }
}
