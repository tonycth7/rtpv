use dirs;
// `rtpv ssh` — store and restore SSH keypairs.
//
// Keys are stored under `ssh/<name>` in the password store.
// The password field holds the private key; `pub:` field holds the public key.

use std::path::PathBuf;
use anyhow::{bail, Result};
use std::collections::HashMap;
use crate::config::Config;
use crate::store::Store;

const SSH_PREFIX: &str = "ssh/";

pub fn list(cfg: &Config) -> Result<Vec<String>> {
    let store = Store::new(cfg)?;
    store.list_filtered(|p| p.starts_with(SSH_PREFIX))
}

pub fn add(cfg: &Config, name: &str, private_key_path: &str, public_key_path: Option<&str>) -> Result<()> {
    let store = Store::new(cfg)?;
    let path = format!("{}{}", SSH_PREFIX, name);

    if store.exists(&path) {
        bail!("SSH key already stored: {}  (use: rtpv edit {})", path, path);
    }

    let private_key = std::fs::read_to_string(private_key_path)?;

    let mut fields = HashMap::new();
    if let Some(pub_path) = public_key_path {
        let pub_key = std::fs::read_to_string(pub_path)?;
        fields.insert("pub".to_string(), pub_key.trim().to_string());
    } else {
        // Try <private_key_path>.pub
        let pub_path = format!("{}.pub", private_key_path);
        if let Ok(pub_key) = std::fs::read_to_string(&pub_path) {
            fields.insert("pub".to_string(), pub_key.trim().to_string());
        }
    }

    store.add(&path, private_key.trim(), fields)?;

    if let Ok(git) = crate::git::StoreGit::open(&cfg.store_dir) {
        let _ = git.commit(&format!("Store SSH key: {}", name));
    }
    Ok(())
}

pub struct RestoreResult {
    pub private_path: PathBuf,
    pub public_path:  Option<PathBuf>,
}

pub fn restore(cfg: &Config, name: &str, target_dir: Option<&str>) -> Result<RestoreResult> {
    let store = Store::new(cfg)?;
    let path = if name.starts_with(SSH_PREFIX) {
        name.to_string()
    } else {
        format!("{}{}", SSH_PREFIX, name)
    };

    let entry = store.read(&path)?;
    let ssh_dir = target_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".ssh"));

    std::fs::create_dir_all(&ssh_dir)?;

    let key_name = name.split('/').last().unwrap_or(name);
    let private_path = ssh_dir.join(key_name);
    let public_path  = ssh_dir.join(format!("{}.pub", key_name));

    // Write private key with restricted permissions
    std::fs::write(&private_path, &entry.password)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&private_path, std::fs::Permissions::from_mode(0o600))?;
    }

    // Write public key if present
    let public_written = if let Some(pub_key) = entry.get_field("pub") {
        std::fs::write(&public_path, format!("{}\n", pub_key))?;
        true
    } else {
        false
    };

    // Add to ssh-agent if available
    let _ = std::process::Command::new("ssh-add").arg(&private_path).status();

    Ok(RestoreResult {
        private_path,
        public_path: if public_written { Some(public_path) } else { None },
    })
}

pub fn copy_pubkey(cfg: &Config, name: &str) -> Result<()> {
    let store = Store::new(cfg)?;
    let path = if name.starts_with(SSH_PREFIX) { name.to_string() }
               else { format!("{}{}", SSH_PREFIX, name) };

    let entry = store.read(&path)?;
    let pub_key = entry.get_field("pub")
        .ok_or_else(|| anyhow::anyhow!("No public key stored for '{}'", name))?;

    crate::clipboard::copy(pub_key)?;
    Ok(())
}

pub fn copy_id(cfg: &Config, name: &str, user_at_host: &str) -> Result<()> {
    let store = Store::new(cfg)?;
    let path = if name.starts_with(SSH_PREFIX) { name.to_string() }
               else { format!("{}{}", SSH_PREFIX, name) };

    let entry = store.read(&path)?;
    let pub_key = entry.get_field("pub")
        .ok_or_else(|| anyhow::anyhow!("No public key stored for '{}'", name))?;

    // Pipe pubkey to ssh-copy-id via stdin
    use std::io::Write;
    let mut child = std::process::Command::new("ssh-copy-id")
        .args(["-i", "/dev/stdin", user_at_host])
        .stdin(std::process::Stdio::piped())
        .spawn()?;
    child.stdin.as_mut()
        .ok_or_else(|| anyhow::anyhow!("ssh-copy-id stdin unavailable"))?
        .write_all(pub_key.as_bytes())?;
    child.wait()?;
    Ok(())
}
