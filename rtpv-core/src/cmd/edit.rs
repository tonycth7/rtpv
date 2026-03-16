use tempfile;
// `rtpv edit` — decrypt, open in $EDITOR, re-encrypt on save.

use anyhow::{bail, Result};
use std::io::Write;
use crate::config::Config;
use crate::store::Store;

pub fn run(cfg: &Config, name: &str) -> Result<()> {
    let store = Store::new(cfg)?;

    if !store.exists(name) {
        bail!("Entry not found: {}", name);
    }

    let entry = store.read(name)?;
    let plaintext = entry.to_plaintext();

    // Write to a temp file
    let mut tmp = tempfile::NamedTempFile::new()?;
    tmp.write_all(plaintext.as_bytes())?;
    tmp.flush()?;

    let tmp_path = tmp.path().to_path_buf();

    // Open in $EDITOR
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());

    let status = std::process::Command::new(&editor)
        .arg(&tmp_path)
        .status()?;

    if !status.success() {
        bail!("Editor '{}' exited with non-zero status", editor);
    }

    // Read back edited content
    let edited = std::fs::read_to_string(&tmp_path)?;

    // Don't save if unchanged
    if edited.trim() == plaintext.trim() {
        return Ok(());
    }

    // Validate: must have at least one line (the password)
    if edited.trim().is_empty() {
        bail!("Refusing to save empty entry");
    }

    // Re-parse and write
    let updated = crate::store::Entry::parse(name, &edited);
    store.write(&updated)?;

    // Git commit
    if let Ok(git) = crate::git::StoreGit::open(&cfg.store_dir) {
        let _ = git.commit(&format!("Edit password for {}", name));
    }
    store.run_hook_with_path(&cfg.hooks_dir, "post-edit", name);

    Ok(())
}
