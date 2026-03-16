use notify_rust;
// `rtpv copy` — copy a field to clipboard with auto-clear.
//
//   rtpv copy github            → copy password
//   rtpv copy github -u         → copy username
//   rtpv copy github -e         → copy email
//   rtpv copy github:token      → copy specific field

use anyhow::Result;
use crate::config::Config;
use crate::store::Store;
use crate::clipboard;

pub struct CopyArgs {
    /// "entry/name" or "entry/name:field"
    pub path:       String,
    pub field:      CopyField,
    pub timeout:    u64,
    pub notify:     bool,
    pub silent:     bool,   // suppress "Copied!" message
}

#[derive(Debug, Default)]
pub enum CopyField {
    #[default]
    Password,
    Username,
    Email,
    Token,
    Otp,
    Named(String),
}

pub struct CopyResult {
    pub field_name: String,
    pub value_len:  usize,
}

pub fn run(cfg: &Config, mut args: CopyArgs) -> Result<CopyResult> {
    let store = Store::new(cfg)?;

    // Support "path:field" shorthand
    let (entry_path, field_hint) = crate::cmd::show::split_path_field(&args.path);
    if let Some(f) = field_hint {
        args.field = CopyField::Named(f.to_string());
    }

    let entry = store.read(entry_path)?;

    let (field_name, value) = match &args.field {
        CopyField::Password         => ("password".to_string(), entry.password.clone()),
        CopyField::Username         => {
            let v = entry.username()
                .ok_or_else(|| anyhow::anyhow!("No username field in '{}'", entry_path))?
                .to_string();
            ("username".to_string(), v)
        }
        CopyField::Email            => {
            let v = entry.email()
                .ok_or_else(|| anyhow::anyhow!("No email field in '{}'", entry_path))?
                .to_string();
            ("email".to_string(), v)
        }
        CopyField::Token            => {
            let v = entry.token()
                .ok_or_else(|| anyhow::anyhow!("No token field in '{}'", entry_path))?
                .to_string();
            ("token".to_string(), v)
        }
        CopyField::Otp              => {
            let uri = entry.otp_uri()
                .ok_or_else(|| anyhow::anyhow!("No OTP configured for '{}'", entry_path))?;
            let state = crate::otp::code_for_uri(uri)?;
            ("otp".to_string(), state.code)
        }
        CopyField::Named(name)      => {
            let v = entry.get_field(name)
                .ok_or_else(|| anyhow::anyhow!("Field '{}' not found in '{}'", name, entry_path))?
                .to_string();
            (name.clone(), v)
        }
    };

    let len = value.len();
    clipboard::copy_with_clear(&value, args.timeout)?;

    if args.notify {
        let _ = notify_rust::Notification::new()
            .summary("rtpv")
            .body(&format!("Copied {} for {} (clears in {}s)", field_name, entry_path, args.timeout))
            .timeout(3000)
            .show();
    }

    Ok(CopyResult { field_name, value_len: len })
}
