//! `rtpv rotate` — replace the password of an existing entry with a new one.

use anyhow::{bail, Result};
use crate::config::Config;
use crate::store::Store;
use crate::gen;

pub struct RotateResult {
    pub old_len:     usize,
    pub new_password: String,
}

pub fn run(cfg: &Config, name: &str, length: Option<usize>) -> Result<RotateResult> {
    let store = Store::new(cfg)?;

    if !store.exists(name) {
        bail!("Entry not found: {}", name);
    }

    let mut entry = store.read(name)?;
    let old_len = entry.password.len();

    let new_password = gen::random_password(
        length.unwrap_or(cfg.gen_length),
        &cfg.gen_chars,
    )?;

    entry.password = new_password.clone();
    store.write(&entry)?;

    if let Ok(git) = crate::git::StoreGit::open(&cfg.store_dir) {
        let _ = git.commit(&format!("Rotate password for {}", name));
    }
    store.run_hook_with_path(&cfg.hooks_dir, "post-rotate", name);

    if cfg.autosync {
        if let Ok(git) = crate::git::StoreGit::open(&cfg.store_dir) {
            let _ = git.push("origin");
        }
    }

    Ok(RotateResult { old_len, new_password })
}
