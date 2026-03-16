//! `rtpv add` — add a new entry.
//!
//! Options
//!   --password / -p [value]   use a manual password (prompt if no value)
//!   --ask / -a                alias for --password without a value
//!   --gen / -g                force-generate (default)
//!   --length / -l <n>         generator length
//!   --words <n>               n-word passphrase
//!   --pin <n>                 numeric PIN of length n
//!   email, username, notes    positional or flag args

use std::collections::HashMap;
use anyhow::{bail, Result};

use crate::config::Config;
use crate::store::{Entry, Store};
use crate::gen;

#[derive(Debug, Default)]
pub struct AddArgs {
    /// Entry path (e.g. "github" or "work/slack")
    pub path:       String,
    pub email:      Option<String>,
    pub username:   Option<String>,
    pub notes:      Option<String>,
    pub url:        Option<String>,

    pub mode:       PasswordMode,
    /// For Manual variant — the actual password (None = prompt)
    pub password:   Option<String>,
    pub length:     Option<usize>,
    pub words:      Option<usize>,
    pub pin:        Option<usize>,

    pub autosync:   bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub enum PasswordMode {
    #[default]
    Generate,
    Manual,
    Words,
    Pin,
}

pub struct AddResult {
    pub entry:    Entry,
    pub password: String,   // plaintext, for display / clipboard
}

/// Run the add command. The caller is responsible for reading the password
/// from stdin if `args.password` is None and mode is Manual.
pub fn run(cfg: &Config, args: AddArgs, ask_password: impl FnOnce() -> Result<String>) -> Result<AddResult> {
    let store = Store::new(cfg)?;

    if store.exists(&args.path) {
        bail!("Entry already exists: {}  (use: rtpv edit {})", args.path, args.path);
    }

    // Resolve the password
    let password = match args.mode {
        PasswordMode::Manual => {
            match args.password {
                Some(pw) => pw,
                None     => ask_password()?,
            }
        }
        PasswordMode::Generate => {
            let length = args.length.unwrap_or(cfg.gen_length);
            gen::random_password(length, &cfg.gen_chars)?
        }
        PasswordMode::Words => {
            let count = args.words.unwrap_or(4);
            gen::passphrase(count, '-')?
        }
        PasswordMode::Pin => {
            let length = args.pin.unwrap_or(6);
            gen::pin(length)?
        }
    };

    // Build fields
    let mut fields = HashMap::new();
    if let Some(e) = args.email    { fields.insert("email".to_string(),    e); }
    if let Some(u) = args.username { fields.insert("username".to_string(), u); }
    if let Some(n) = args.notes    { fields.insert("notes".to_string(),    n); }
    if let Some(u) = args.url      { fields.insert("url".to_string(),      u); }

    let entry = store.add(&args.path, &password, fields)?;

    // Git commit
    if let Ok(git) = crate::git::StoreGit::open(&cfg.store_dir) {
        let _ = git.commit(&format!("Add password for {}", args.path));
    }
    // Hook
    store.run_hook_with_path(&cfg.hooks_dir, "post-add", &args.path);
    // Autosync
    if args.autosync || cfg.autosync {
        if let Ok(git) = crate::git::StoreGit::open(&cfg.store_dir) {
            let _ = git.push("origin");
        }
    }

    Ok(AddResult { entry, password })
}
