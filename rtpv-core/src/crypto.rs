// Crypto backends — age (native) and GPG (subprocess compat).
//
// The `Backend` trait is the single interface both backends implement.
// All higher-level code talks to `Backend`, never to age/gpg directly.
//
// # Age backend
// Uses the `age` crate for pure-Rust age encryption.
// Keys are stored at `~/.config/rtpv/identity.age` (X25519).
// Recipients file at `~/.config/rtpv/recipients.txt` (one pubkey/line).
//
// # GPG backend
// Wraps the `gpg` binary via `std::process::Command`.
// Reads recipient IDs from `~/.password-store/.gpg-id` (pass compatible).
// Used for migrating existing pass stores.
use age::secrecy::ExposeSecret;
use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{bail, Context, Result};

// ─────────────────────────────────────────────────────────────────────────────
// Backend trait
// ─────────────────────────────────────────────────────────────────────────────
pub trait Backend: Send + Sync {
    /// Encrypt `plaintext` to all configured recipients. Returns ciphertext.
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>>;

    /// Decrypt `ciphertext` using the local identity/key. Returns plaintext.
    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>>;

    /// File extension used for encrypted files (e.g. "age" or "gpg")
    fn extension(&self) -> &'static str;
}

// ─────────────────────────────────────────────────────────────────────────────
// Factory — build the right backend from config
// ─────────────────────────────────────────────────────────────────────────────
pub fn build_backend(cfg: &crate::config::Config) -> Result<Box<dyn Backend>> {
    match cfg.crypto_backend {
        crate::config::CryptoBackend::Age => {
            let b = AgeBackend::new(&cfg.age_identity, &cfg.config_dir)?;
            Ok(Box::new(b))
        }
        crate::config::CryptoBackend::Gpg => {
            let recipients_file = cfg.store_dir.join(".gpg-id");
            let b = GpgBackend::new(recipients_file)?;
            Ok(Box::new(b))
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Age backend
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(feature = "age-native")]
pub struct AgeBackend {
    identity_path:   PathBuf,
    recipients_path: PathBuf,
}

#[cfg(feature = "age-native")]
impl AgeBackend {
    pub fn new(identity_path: &Path, config_dir: &Path) -> Result<Self> {
        Ok(Self {
            identity_path:   identity_path.to_path_buf(),
            recipients_path: config_dir.join("recipients.txt"),
        })
    }

    /// Generate a new X25519 identity and save it (first-time setup).
    pub fn init(identity_path: &Path, config_dir: &Path) -> Result<String> {
        use age::x25519;
        std::fs::create_dir_all(config_dir)?;

        let secret = x25519::Identity::generate();
        let pubkey = secret.to_public();

        // Write identity (private)
        let id_text = format!(
            "# rtpv age identity — keep this private!\n# Created: {}\n{}\n",
            chrono::Local::now().format("%Y-%m-%d"),
            secret.to_string().expose_secret().to_string()
        );
        // Restrict permissions on Unix
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = std::fs::OpenOptions::new()
                .write(true).create(true).truncate(true)
                .mode(0o600)
                .open(identity_path)?;
            f.write_all(id_text.as_bytes())?;
        }
        #[cfg(not(unix))]
        std::fs::write(identity_path, &id_text)?;

        // Write recipients (public key)
        let recipients_path = config_dir.join("recipients.txt");
        std::fs::write(&recipients_path, format!("{}\n", pubkey))?;

        Ok(pubkey.to_string())
    }

    fn load_identity(&self) -> Result<age::x25519::Identity> {
        let text = std::fs::read_to_string(&self.identity_path)
            .with_context(|| format!(
                "Cannot read age identity at {:?} — run: rtpv init",
                self.identity_path
            ))?;
        // Filter comment lines, parse first identity
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            return line.parse::<age::x25519::Identity>()
                .map_err(|e| anyhow::anyhow!("Bad age identity: {}", e));
        }
        bail!("No identity found in {:?}", self.identity_path)
    }

    fn load_recipients(&self) -> Result<Vec<age::x25519::Recipient>> {
        let text = std::fs::read_to_string(&self.recipients_path)
            .with_context(|| format!(
                "Cannot read recipients at {:?} — run: rtpv recipients add <pubkey>",
                self.recipients_path
            ))?;
        text.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| l.parse::<age::x25519::Recipient>()
                .map_err(|e| anyhow::anyhow!("Bad recipient key '{}': {}", l, e)))
            .collect()
    }
}

#[cfg(feature = "age-native")]
impl Backend for AgeBackend {
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        use age::Encryptor;
        use std::io::Write;

        let recipients = self.load_recipients()?;
        let rec_refs: Vec<&dyn age::Recipient> = recipients
          .iter()
          .map(|r| r as &dyn age::Recipient)
          .collect();
        let encryptor = Encryptor::with_recipients(rec_refs.into_iter())
            .map_err(|e| anyhow::anyhow!("age encrypt init: {:?}", e))?;
        let mut ciphertext = Vec::new();
        let mut writer = encryptor.wrap_output(&mut ciphertext)
            .map_err(|e| anyhow::anyhow!("age wrap_output: {:?}", e))?;
        writer.write_all(plaintext)?;
        writer.finish()
            .map_err(|e| anyhow::anyhow!("age encrypt finish: {:?}", e))?;
        Ok(ciphertext)
    }

    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        use age::Decryptor;
        use std::io::{Read, Cursor};

        let identity = self.load_identity()?;
        let cursor = Cursor::new(ciphertext);
        let decryptor = Decryptor::new(cursor)
            .map_err(|e| anyhow::anyhow!("age decryptor: {:?}", e))?;
        let mut plaintext = Vec::new();
        let mut reader = decryptor
            .decrypt(std::iter::once(&identity as &dyn age::Identity))
            .map_err(|e| anyhow::anyhow!("age decrypt: {:?}", e))?;
        reader.read_to_end(&mut plaintext)?;
        Ok(plaintext)
    }

    fn extension(&self) -> &'static str { "age" }
}

// ─────────────────────────────────────────────────────────────────────────────
// Stub for when age-native feature is disabled
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(not(feature = "age-native"))]
pub struct AgeBackend;

#[cfg(not(feature = "age-native"))]
impl AgeBackend {
    pub fn new(_: &Path, _: &Path) -> Result<Self> {
        bail!("rtpv was compiled without the 'age-native' feature")
    }
}

#[cfg(not(feature = "age-native"))]
impl Backend for AgeBackend {
    fn encrypt(&self, _: &[u8]) -> Result<Vec<u8>> { bail!("age-native not compiled in") }
    fn decrypt(&self, _: &[u8]) -> Result<Vec<u8>> { bail!("age-native not compiled in") }
    fn extension(&self) -> &'static str { "age" }
}

// ─────────────────────────────────────────────────────────────────────────────
// GPG backend (subprocess)
// ─────────────────────────────────────────────────────────────────────────────
pub struct GpgBackend {
    /// Recipient key IDs / fingerprints
    recipients: Vec<String>,
}

impl GpgBackend {
    pub fn new(gpg_id_file: PathBuf) -> Result<Self> {
        let text = std::fs::read_to_string(&gpg_id_file)
            .with_context(|| format!(
                "Cannot read {:?} — is this a pass store? (run: pass init <gpg-id>)",
                gpg_id_file
            ))?;
        let recipients: Vec<String> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(String::from)
            .collect();
        if recipients.is_empty() {
            bail!("No GPG recipient IDs found in {:?}", gpg_id_file);
        }
        Ok(Self { recipients })
    }

    fn gpg() -> Command {
        let mut cmd = Command::new("gpg");
        cmd.arg("--quiet").arg("--batch").arg("--yes");
        // Use loopback pinentry so we work over SSH / headless
        cmd.arg("--pinentry-mode").arg("loopback");
        cmd
    }
}

impl Backend for GpgBackend {
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        use std::io::Write;
        let mut args = vec!["--armor".to_string()];
        for r in &self.recipients {
            args.push("-r".to_string());
            args.push(r.clone());
        }
        args.push("--encrypt".to_string());

        let mut child = GpgBackend::gpg()
            .args(&args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;
        child.stdin.as_mut()
            .ok_or_else(|| anyhow::anyhow!("gpg stdin unavailable"))?
            .write_all(plaintext)?;
        drop(child.stdin.take()); // close stdin so gpg sees EOF
        let out = child.wait_with_output()?;

        if !out.status.success() {
            bail!("gpg encrypt failed: {}", String::from_utf8_lossy(&out.stderr));
        }
        Ok(out.stdout)
    }

    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        use std::io::Write;
        let mut child = GpgBackend::gpg()
            .arg("--decrypt")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;
        child.stdin.as_mut()
            .ok_or_else(|| anyhow::anyhow!("gpg stdin unavailable"))?
            .write_all(ciphertext)?;
        drop(child.stdin.take());
        let out = child.wait_with_output()?;
        if !out.status.success() {
            bail!("gpg decrypt failed: {}", String::from_utf8_lossy(&out.stderr));
        }
        Ok(out.stdout)
    }

    fn extension(&self) -> &'static str { "gpg" }
}

// ─────────────────────────────────────────────────────────────────────────────
// Store migration: .gpg → .age
// ─────────────────────────────────────────────────────────────────────────────
pub fn migrate_store_to_age(
    store_dir: &Path,
    gpg_backend: &dyn Backend,
    age_backend: &dyn Backend,
) -> Result<(usize, usize)> {
    let mut ok = 0usize;
    let mut fail = 0usize;

    for entry in walkdir::WalkDir::new(store_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("gpg") { continue; }

        let ciphertext = std::fs::read(path)?;
        match gpg_backend.decrypt(&ciphertext) {
            Err(e) => {
                eprintln!("  skip {}: {}", path.display(), e);
                fail += 1;
                continue;
            }
            Ok(plaintext) => {
                let new_cipher = age_backend.encrypt(&plaintext)?;
                let new_path = path.with_extension("age");
                std::fs::write(&new_path, &new_cipher)?;
                std::fs::remove_file(path)?;
                ok += 1;
            }
        }
    }
    Ok((ok, fail))
}
