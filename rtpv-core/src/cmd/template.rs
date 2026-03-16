//! `rtpv template` — create entries from structured templates.
//!
//! Templates
//!   web-login      path, username, email, url, password
//!   server         path, host, username, port, password
//!   database       path, host, username, database, port, password
//!   api-key        path, token, url, notes
//!   email-account  path, email, username, server, port, password
//!   credit-card    path, number, expiry, cvv, holder
//!   wifi           path, ssid, password
//!   note           path, notes

use std::collections::HashMap;
use anyhow::Result;
use crate::config::Config;
use crate::store::{Entry, Store};
use crate::gen;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateKind {
    WebLogin,
    Server,
    Database,
    ApiKey,
    EmailAccount,
    CreditCard,
    Wifi,
    Note,
}

impl TemplateKind {
    pub fn all() -> &'static [TemplateKind] {
        &[
            TemplateKind::WebLogin,
            TemplateKind::Server,
            TemplateKind::Database,
            TemplateKind::ApiKey,
            TemplateKind::EmailAccount,
            TemplateKind::CreditCard,
            TemplateKind::Wifi,
            TemplateKind::Note,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            TemplateKind::WebLogin     => "web-login",
            TemplateKind::Server       => "server",
            TemplateKind::Database     => "database",
            TemplateKind::ApiKey       => "api-key",
            TemplateKind::EmailAccount => "email-account",
            TemplateKind::CreditCard   => "credit-card",
            TemplateKind::Wifi         => "wifi",
            TemplateKind::Note         => "note",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            TemplateKind::WebLogin     => "Website login (username, email, url)",
            TemplateKind::Server       => "SSH / remote server (host, user, port)",
            TemplateKind::Database     => "Database credentials (host, db, user, port)",
            TemplateKind::ApiKey       => "API key / token (token, url)",
            TemplateKind::EmailAccount => "Email account (server, port, protocol)",
            TemplateKind::CreditCard   => "Credit card (number, expiry, CVV, holder)",
            TemplateKind::Wifi         => "WiFi network (SSID, password)",
            TemplateKind::Note         => "Secure text note",
        }
    }

    /// Required field prompts: (key, label, is_password)
    pub fn fields(&self) -> &'static [(&'static str, &'static str, bool)] {
        match self {
            TemplateKind::WebLogin => &[
                ("username", "Username", false),
                ("email",    "Email",    false),
                ("url",      "URL",      false),
            ],
            TemplateKind::Server => &[
                ("host",     "Hostname / IP", false),
                ("username", "Username",      false),
                ("port",     "Port",          false),
            ],
            TemplateKind::Database => &[
                ("host",     "Hostname",  false),
                ("username", "Username",  false),
                ("database", "Database",  false),
                ("port",     "Port",      false),
            ],
            TemplateKind::ApiKey => &[
                ("token",    "Token / API key", true),
                ("url",      "API base URL",     false),
                ("notes",    "Notes",            false),
            ],
            TemplateKind::EmailAccount => &[
                ("email",    "Email address", false),
                ("username", "Username",      false),
                ("host",     "IMAP/SMTP server", false),
                ("port",     "Port",          false),
            ],
            TemplateKind::CreditCard => &[
                ("number",   "Card number", true),
                ("expiry",   "Expiry (MM/YY)", false),
                ("cvv",      "CVV",         true),
                ("holder",   "Cardholder name", false),
            ],
            TemplateKind::Wifi => &[
                ("ssid",     "Network name (SSID)", false),
            ],
            TemplateKind::Note => &[
                ("notes",    "Note content", false),
            ],
        }
    }

    /// Whether this template has a password field.
    pub fn has_password(&self) -> bool {
        !matches!(self, TemplateKind::ApiKey | TemplateKind::CreditCard | TemplateKind::Note)
    }
}

pub struct TemplateArgs {
    pub path:     String,
    pub kind:     TemplateKind,
    pub fields:   HashMap<String, String>,
    /// None = generate, Some(pw) = use this password
    pub password: Option<String>,
    pub gen_len:  usize,
}

pub struct TemplateResult {
    pub entry:    Entry,
    pub password: String,
}

pub fn run(cfg: &Config, args: TemplateArgs) -> Result<TemplateResult> {
    let store = Store::new(cfg)?;

    let password = if args.kind.has_password() {
        match args.password {
            Some(pw) => pw,
            None     => gen::random_password(args.gen_len, &cfg.gen_chars)?,
        }
    } else {
        // For API keys, the token IS the secret; password field = empty
        String::new()
    };

    let entry = store.add(&args.path, &password, args.fields)?;

    if let Ok(git) = crate::git::StoreGit::open(&cfg.store_dir) {
        let _ = git.commit(&format!("Add {} entry: {}", args.kind.name(), args.path));
    }
    store.run_hook_with_path(&cfg.hooks_dir, "post-add", &args.path);

    Ok(TemplateResult { entry, password })
}
