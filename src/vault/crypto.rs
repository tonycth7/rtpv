// vault/crypto.rs — age encryption backend
// Native Rust, no subprocess, no GPG dependency.
use age::secrecy::ExposeSecret;
use age::x25519::{Identity, Recipient};
use age::{Decryptor, Encryptor};
use anyhow::{bail, Context, Result};
use std::io::{Read, Write};
use std::path::Path;

// ── Key management ───────────────────────────────────────────────────────────

/// Generate a new X25519 identity, write it to `path` (mode 0600 on Unix).
/// Returns the public key string.
pub fn generate_identity(path: &Path) -> Result<String> {
    let identity = Identity::generate();
    let pubkey = identity.to_public().to_string();
    // age Identity implements Display via its to_string() which gives the Bech32 key
    let id_str = identity.to_string();
    let content = format!(
        "# rtpv age identity — KEEP THIS PRIVATE\n# Public key: {}\n{}\n",
        pubkey,
        id_str.expose_secret()
    );
    write_secret_file(path, &content)?;
    Ok(pubkey)
}

/// Load identity from file, return first non-comment line parsed as Identity.
pub fn load_identity(path: &Path) -> Result<Identity> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("Cannot read identity at {}\nRun: rtpv init", path.display()))?;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        return line
            .parse::<Identity>()
            .map_err(|e| anyhow::anyhow!("Bad identity key: {}", e));
    }
    bail!("No identity found in {}", path.display())
}

/// Load all recipients from recipients.txt — one pubkey per line.
pub fn load_recipients(path: &Path) -> Result<Vec<Recipient>> {
    let text = std::fs::read_to_string(path).with_context(|| {
        format!(
            "Cannot read recipients at {}\nRun: rtpv recipients add <pubkey>",
            path.display()
        )
    })?;
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            l.parse::<Recipient>()
                .map_err(|e| anyhow::anyhow!("Bad recipient key '{}': {}", l, e))
        })
        .collect()
}

/// Add a public key to recipients.txt. No duplicates.
pub fn add_recipient(recipients_path: &Path, pubkey: &str) -> Result<()> {
    if !pubkey.starts_with("age1") {
        bail!("Invalid age public key — must start with 'age1'");
    }
    let existing = if recipients_path.exists() {
        std::fs::read_to_string(recipients_path)?
    } else {
        String::new()
    };
    if existing.lines().any(|l| l.trim() == pubkey) {
        bail!("Recipient already in recipients.txt");
    }
    let mut content = existing;
    if !content.ends_with('\n') && !content.is_empty() {
        content.push('\n');
    }
    content.push_str(pubkey);
    content.push('\n');
    std::fs::write(recipients_path, content)?;
    Ok(())
}

// ── Encrypt / Decrypt ────────────────────────────────────────────────────────

pub fn encrypt(plaintext: &[u8], recipients: &[Recipient]) -> Result<Vec<u8>> {
    use age::Recipient;
    use std::io::Write;

    // Convert recipients into boxed trait objects required by age
    let recipients: Vec<Box<dyn age::Recipient + Send>> = recipients
        .iter()
        .cloned()
        .map(|r| Box::new(r) as Box<dyn age::Recipient + Send>)
        .collect();

    let encryptor = Encryptor::with_recipients(recipients)
        .ok_or_else(|| anyhow::anyhow!("age encryption init failed"))?;

    let mut ciphertext = vec![];

    let mut writer = encryptor
        .wrap_output(&mut ciphertext)
        .map_err(|e| anyhow::anyhow!("age wrap_output failed: {:?}", e))?;

    writer.write_all(plaintext)?;

    writer
        .finish()
        .map_err(|e| anyhow::anyhow!("age encrypt finish failed: {:?}", e))?;

    Ok(ciphertext)
}

pub fn decrypt(ciphertext: &[u8], identity: &Identity) -> Result<Vec<u8>> {
    use std::io::{Cursor, Read};

    let decryptor = Decryptor::new(Cursor::new(ciphertext))?;

    let mut reader = match decryptor {
        Decryptor::Recipients(d) => d.decrypt(std::iter::once(identity as &dyn age::Identity))?,
        _ => {
            return Err(anyhow::anyhow!("Unsupported age format"));
        }
    };

    let mut plaintext = Vec::new();
    reader.read_to_end(&mut plaintext)?;

    Ok(plaintext)
}

/// Encrypt a UTF-8 string and return ciphertext bytes.
pub fn encrypt_str(plaintext: &str, recipients: &[Recipient]) -> Result<Vec<u8>> {
    encrypt(plaintext.as_bytes(), recipients)
}

/// Decrypt ciphertext and return UTF-8 string.
pub fn decrypt_str(ciphertext: &[u8], identity: &Identity) -> Result<String> {
    let bytes = decrypt(ciphertext, identity)?;
    String::from_utf8(bytes).context("Decrypted data is not valid UTF-8")
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Write file with 0600 permissions on Unix.
fn write_secret_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        f.write_all(content.as_bytes())?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, content)?;
    }
    Ok(())
}
