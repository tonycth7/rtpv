//! `rtpv share` — share an encrypted entry with another person.
//!
//! Re-encrypts the entry's plaintext to a recipient's age public key
//! and writes the result to a file that can be sent safely.
//!
//! The recipient decrypts with:
//!   age -d shared.age
//!
//! Usage:
//!   rtpv share github age1xxxxxxxxx           # share to age pubkey
//!   rtpv share github --out github.age        # custom output file

use std::io::Write;
use std::path::PathBuf;
use anyhow::{bail, Context, Result};
use crate::config::Config;
use crate::store::Store;

pub struct ShareResult {
    pub output_path: PathBuf,
    pub recipient:   String,
    pub entry_path:  String,
}

pub fn run(cfg: &Config, entry_name: &str, recipient_key: &str, output: Option<&str>) -> Result<ShareResult> {
    // Validate recipient key format
    if !recipient_key.starts_with("age1") {
        bail!(
            "Invalid age public key '{}' — age public keys start with 'age1'",
            recipient_key
        );
    }

    let store = Store::new(cfg)?;
    let entry = store.read(entry_name)?;
    let plaintext = entry.to_plaintext();

    // Parse recipient
    #[cfg(feature = "age-native")]
    let encrypted = {
        use age::{Encryptor, x25519};
        let recipient: x25519::Recipient = recipient_key.parse()
            .map_err(|e| anyhow::anyhow!("Bad age public key: {:?}", e))?;
       let rec_refs: Vec<&dyn age::Recipient> = vec![&recipient as &dyn age::Recipient];
       let mut ciphertext = Vec::new();
       let encryptor = Encryptor::with_recipients(rec_refs.into_iter())
          .map_err(|e| anyhow::anyhow!("age encrypt init: {:?}", e))?;
       let mut writer = encryptor.wrap_output(&mut ciphertext)
          .map_err(|e| anyhow::anyhow!("age wrap: {:?}", e))?;
       writer.write_all(plaintext.as_bytes())?;
       writer.finish()
          .map_err(|e| anyhow::anyhow!("age finish: {:?}", e))?;
       ciphertext
    };

    #[cfg(not(feature = "age-native"))]
    let encrypted = bail!("rtpv share requires the age-native feature");

    // Determine output path
    let default_name = format!(
        "{}-shared.age",
        entry_name.split('/').last().unwrap_or(entry_name)
    );
    let output_path = output
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default().join(default_name));

    std::fs::write(&output_path, &encrypted)
        .with_context(|| format!("Cannot write to {:?}", output_path))?;

    Ok(ShareResult {
        output_path,
        recipient: recipient_key.to_string(),
        entry_path: entry_name.to_string(),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Recipients management — add/list/remove recipients for the store
// ─────────────────────────────────────────────────────────────────────────────

/// List all configured recipients (public keys).
pub fn list_recipients(cfg: &Config) -> Result<Vec<String>> {
    let rec_path = cfg.config_dir.join("recipients.txt");
    if !rec_path.exists() { return Ok(vec![]); }
    Ok(std::fs::read_to_string(&rec_path)?
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect())
}

/// Add a recipient public key. Re-encryption of the store is NOT automatic
/// — run `rtpv reencrypt` afterward.
pub fn add_recipient(cfg: &Config, key: &str) -> Result<()> {
    if !key.starts_with("age1") {
        bail!("Invalid age public key — must start with 'age1'");
    }
    let rec_path = cfg.config_dir.join("recipients.txt");
    let current = if rec_path.exists() {
        std::fs::read_to_string(&rec_path)?
    } else {
        String::new()
    };
     if current.lines().any(|l: &str| l.trim() == key) {
        bail!("Recipient '{}' is already in recipients.txt", key);
    }
    std::fs::create_dir_all(&cfg.config_dir)?;
    let updated = format!("{}{}\n", current, key);
    std::fs::write(&rec_path, updated)?;
    Ok(())
}

/// Remove a recipient by public key prefix or full key.
pub fn remove_recipient(cfg: &Config, key_or_prefix: &str) -> Result<bool> {
    let rec_path = cfg.config_dir.join("recipients.txt");
    if !rec_path.exists() { return Ok(false); }
    let content = std::fs::read_to_string(&rec_path)?;
    let (kept, removed): (Vec<&str>, Vec<&str>) = content.lines()
        .partition(|l| !l.trim().starts_with(key_or_prefix));
    if removed.is_empty() { return Ok(false); }
    let updated = kept.join("\n") + "\n";
    std::fs::write(&rec_path, updated)?;
    Ok(true)
}

/// Re-encrypt every entry in the store to the current recipients list.
/// This is needed after adding or removing a recipient.
pub fn reencrypt_store(cfg: &Config) -> Result<(usize, usize)> {
    let store = Store::new(cfg)?;
    let paths = store.list()?;
    let mut ok = 0usize;
    let mut fail = 0usize;
    for path in &paths {
        match store.read(path) {
            Ok(entry) => match store.write(&entry) {
                Ok(_)  => ok   += 1,
                Err(_) => fail += 1,
            },
            Err(_) => fail += 1,
        }
    }
    if ok > 0 {
        if let Ok(git) = crate::git::StoreGit::open(&cfg.store_dir) {
            let _ = git.commit("Re-encrypt store (recipients changed)");
        }
    }
    Ok((ok, fail))
}
