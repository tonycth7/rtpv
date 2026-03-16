// vault/store.rs — read/write entries from ~/.password-vault/

use age::x25519::{Identity, Recipient};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

use super::crypto;
use super::entry::Entry;

const VAULT_DIR_NAME: &str = ".rtpv";
const IDENTITY_FILE: &str = "identity.age";
const RECIPIENTS_FILE: &str = "recipients.txt";
const ENTRY_EXT: &str = "age";

// ── Vault ────────────────────────────────────────────────────────────────────

pub struct Vault {
    pub root: PathBuf,
    identity: Identity,
    recipients: Vec<Recipient>,
    meta_dir: PathBuf,
}

impl Vault {
    /// Open an existing vault. Errors if not initialized.
    pub fn open(root: &Path) -> Result<Self> {
        let meta_dir = root.join(VAULT_DIR_NAME);
        let identity_path = meta_dir.join(IDENTITY_FILE);
        let rec_path = meta_dir.join(RECIPIENTS_FILE);

        if !meta_dir.exists() {
            bail!("No vault found at {}\nRun: rtpv init", root.display());
        }

        let identity = crypto::load_identity(&identity_path)?;
        let recipients = crypto::load_recipients(&rec_path)?;

        Ok(Self {
            root: root.to_path_buf(),
            identity,
            recipients,
            meta_dir,
        })
    }

    /// Initialize a new vault at `root`. Generates identity + recipients file.
    pub fn init(root: &Path) -> Result<String> {
        let meta_dir = root.join(VAULT_DIR_NAME);
        let identity_path = meta_dir.join(IDENTITY_FILE);
        let rec_path = meta_dir.join(RECIPIENTS_FILE);

        std::fs::create_dir_all(&meta_dir)?;
        std::fs::create_dir_all(root)?;

        if identity_path.exists() {
            bail!("Vault already initialized at {}\nUse --force to reinitialize (WARNING: loses access to existing entries)", root.display());
        }

        let pubkey = crypto::generate_identity(&identity_path)?;
        crypto::add_recipient(&rec_path, &pubkey)?;

        // Restrict meta dir permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&meta_dir, std::fs::Permissions::from_mode(0o700))?;
        }

        Ok(pubkey)
    }

    /// Force re-init (dangerous — only for password change / key rotation).
    pub fn reinit(root: &Path) -> Result<String> {
        let meta_dir = root.join(VAULT_DIR_NAME);
        let identity_path = meta_dir.join(IDENTITY_FILE);
        let rec_path = meta_dir.join(RECIPIENTS_FILE);

        std::fs::create_dir_all(&meta_dir)?;
        let pubkey = crypto::generate_identity(&identity_path)?;
        // Overwrite recipients with new pubkey
        std::fs::write(&rec_path, format!("{}\n", pubkey))?;
        Ok(pubkey)
    }

    pub fn pubkey(&self) -> String {
        self.recipients
            .first()
            .map(|r| r.to_string())
            .unwrap_or_default()
    }

    pub fn identity_path(&self) -> PathBuf {
        self.meta_dir.join(IDENTITY_FILE)
    }
    pub fn recipients_path(&self) -> PathBuf {
        self.meta_dir.join(RECIPIENTS_FILE)
    }

    pub fn add_recipient(&self, pubkey: &str) -> Result<()> {
        crypto::add_recipient(&self.recipients_path(), pubkey)
    }

    // ── Path helpers ─────────────────────────────────────────────────────────

    pub fn entry_path(&self, name: &str) -> PathBuf {
        self.root.join(name).with_extension(ENTRY_EXT)
    }

    pub fn exists(&self, name: &str) -> bool {
        self.entry_path(name).exists()
    }

    // ── List ─────────────────────────────────────────────────────────────────

    pub fn list(&self) -> Result<Vec<String>> {
        let mut entries = Vec::new();
        for e in walkdir::WalkDir::new(&self.root)
            .follow_links(false)
            .sort_by_file_name()
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = e.path();
            // Skip the .rtpv meta directory
            if path.components().any(|c| c.as_os_str() == VAULT_DIR_NAME) {
                continue;
            }
            if path.extension().and_then(|s| s.to_str()) != Some(ENTRY_EXT) {
                continue;
            }
            if let Ok(rel) = path.strip_prefix(&self.root) {
                let name = rel.with_extension("").to_string_lossy().replace('\\', "/");
                entries.push(name);
            }
        }
        Ok(entries)
    }

    pub fn list_filtered(&self, prefix: &str) -> Result<Vec<String>> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|n| n.starts_with(prefix))
            .collect())
    }

    // ── Read ─────────────────────────────────────────────────────────────────

    pub fn read(&self, name: &str) -> Result<Entry> {
        let path = self.entry_path(name);
        if !path.exists() {
            bail!("Entry not found: {}", name);
        }
        let ciphertext =
            std::fs::read(&path).with_context(|| format!("Cannot read {}", path.display()))?;
        let plaintext = crypto::decrypt_str(&ciphertext, &self.identity)
            .with_context(|| format!("Decryption failed for '{}'", name))?;
        Entry::from_toml(&plaintext).with_context(|| format!("Invalid entry format for '{}'", name))
    }

    // ── Write ────────────────────────────────────────────────────────────────

    pub fn write(&self, name: &str, entry: &Entry) -> Result<()> {
        let path = self.entry_path(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let plaintext = entry.to_toml()?;
        let ciphertext = crypto::encrypt_str(&plaintext, &self.recipients)?;
        std::fs::write(&path, ciphertext)?;
        Ok(())
    }

    // ── Delete ───────────────────────────────────────────────────────────────

    pub fn remove(&self, name: &str) -> Result<()> {
        let path = self.entry_path(name);
        if !path.exists() {
            bail!("Entry not found: {}", name);
        }
        std::fs::remove_file(&path)?;
        // Remove empty parent directories (not root)
        if let Some(parent) = path.parent() {
            if parent != self.root {
                let empty = parent
                    .read_dir()
                    .map(|mut d| d.next().is_none())
                    .unwrap_or(false);
                if empty {
                    let _ = std::fs::remove_dir(parent);
                }
            }
        }
        Ok(())
    }

    // ── Rename ───────────────────────────────────────────────────────────────

    pub fn rename(&self, src: &str, dst: &str) -> Result<()> {
        if !self.exists(src) {
            bail!("Entry not found: {}", src);
        }
        if self.exists(dst) {
            bail!("Destination already exists: {}", dst);
        }
        let src_path = self.entry_path(src);
        let dst_path = self.entry_path(dst);
        if let Some(p) = dst_path.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::rename(src_path, dst_path)?;
        Ok(())
    }

    // ── Clone ────────────────────────────────────────────────────────────────

    pub fn clone_entry(&self, src: &str, dst: &str) -> Result<()> {
        if !self.exists(src) {
            bail!("Entry not found: {}", src);
        }
        if self.exists(dst) {
            bail!("Destination already exists: {}", dst);
        }
        let src_path = self.entry_path(src);
        let dst_path = self.entry_path(dst);
        if let Some(p) = dst_path.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::copy(src_path, dst_path)?;
        Ok(())
    }

    // ── Re-encrypt (after adding recipient) ──────────────────────────────────

    pub fn reencrypt_all(&self) -> Result<(usize, usize)> {
        let names = self.list()?;
        let mut ok = 0usize;
        let mut fail = 0usize;
        for name in &names {
            match self.read(name) {
                Ok(entry) => match self.write(name, &entry) {
                    Ok(_) => ok += 1,
                    Err(_) => fail += 1,
                },
                Err(_) => fail += 1,
            }
        }
        Ok((ok, fail))
    }
}
