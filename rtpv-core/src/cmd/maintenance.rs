use notify_rust;
// `rtpv doctor`, `rtpv stats`, `rtpv sync`, `rtpv gc`, `rtpv lint`

use anyhow::Result;
use crate::config::Config;
use crate::store::Store;

// ─────────────────────────────────────────────────────────────────────────────
// doctor
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct DoctorCheck {
    pub name:   &'static str,
    pub ok:     bool,
    pub detail: String,
}

pub fn doctor(cfg: &Config) -> Vec<DoctorCheck> {
    let mut checks = Vec::new();

    // Store directory exists
    checks.push(DoctorCheck {
        name:   "store directory",
        ok:     cfg.store_dir.is_dir(),
        detail: cfg.store_dir.display().to_string(),
    });

    // Age identity
    checks.push(DoctorCheck {
        name:   "age identity",
        ok:     cfg.age_identity.exists(),
        detail: cfg.age_identity.display().to_string(),
    });

    // Recipients
    let rec = cfg.config_dir.join("recipients.txt");
    checks.push(DoctorCheck {
        name:   "age recipients",
        ok:     rec.exists(),
        detail: rec.display().to_string(),
    });

    // Git repo
    checks.push(DoctorCheck {
        name:   "git repository",
        ok:     crate::git::StoreGit::is_git_repo(&cfg.store_dir),
        detail: cfg.store_dir.display().to_string(),
    });

    // Clipboard
    checks.push(DoctorCheck {
        name:   "clipboard",
        ok:     crate::clipboard::available(),
        detail: if crate::clipboard::available() { "available".to_string() }
                else { "no display server / arboard unavailable".to_string() },
    });

    // xdotool / ydotool
    let fill_ok = which("xdotool") || which("ydotool");
    checks.push(DoctorCheck {
        name:   "autofill (xdotool/ydotool)",
        ok:     fill_ok,
        detail: if fill_ok { "found".to_string() } else { "not found — install xdotool or ydotool".to_string() },
    });

    // notify-send
    checks.push(DoctorCheck {
        name:   "notifications (notify-send)",
        ok:     which("notify-send"),
        detail: "desktop notifications".to_string(),
    });

    checks
}

fn which(cmd: &str) -> bool {
    std::process::Command::new("which").arg(cmd).output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ─────────────────────────────────────────────────────────────────────────────
// stats
// ─────────────────────────────────────────────────────────────────────────────
pub struct StoreStats {
    pub total:         usize,
    pub with_otp:      usize,
    pub with_url:      usize,
    pub dirs:          usize,
    pub store_path:    String,
    pub backend:       String,
}

pub fn stats(cfg: &Config) -> Result<StoreStats> {
    let store = Store::new(cfg)?;
    let paths = store.list()?;
    let total = paths.len();

    let mut with_otp = 0;
    let mut with_url = 0;

    for path in &paths {
        if let Ok(entry) = store.read(path) {
            if entry.has_otp() { with_otp += 1; }
            if entry.url().is_some() { with_url += 1; }
        }
    }

    // Count unique directories
    let dirs: std::collections::HashSet<&str> = paths.iter()
        .filter_map(|p| p.rfind('/').map(|i| &p[..i]))
        .collect();

    Ok(StoreStats {
        total,
        with_otp,
        with_url,
        dirs: dirs.len(),
        store_path: cfg.store_dir.display().to_string(),
        backend:    format!("{:?}", cfg.crypto_backend),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// sync
// ─────────────────────────────────────────────────────────────────────────────
pub fn sync(cfg: &Config) -> Result<()> {
    let git = crate::git::StoreGit::open(&cfg.store_dir)?;
    git.pull("origin")?;
    git.push("origin")?;
    if cfg.notify {
        let _ = notify_rust::Notification::new()
            .summary("rtpv")
            .body("Store synced")
            .timeout(2000)
            .show();
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// lint — find orphaned / malformed entries
// ─────────────────────────────────────────────────────────────────────────────
pub struct LintIssue {
    pub path:   String,
    pub detail: String,
}

pub fn lint(cfg: &Config) -> Result<Vec<LintIssue>> {
    let store = Store::new(cfg)?;
    let paths = store.list()?;
    let mut issues = Vec::new();

    for path in &paths {
        match store.read(path) {
            Err(e) => issues.push(LintIssue {
                path:   path.clone(),
                detail: format!("decrypt failed: {}", e),
            }),
            Ok(entry) => {
                if entry.password.is_empty() && entry.fields.is_empty() {
                    issues.push(LintIssue {
                        path:   path.clone(),
                        detail: "empty entry (no password, no fields)".to_string(),
                    });
                }
                // Check OTP URI validity
                if let Some(uri) = entry.otp_uri() {
                    if crate::otp::from_uri(uri).is_err() {
                        issues.push(LintIssue {
                            path:   path.clone(),
                            detail: "invalid OTP URI".to_string(),
                        });
                    }
                }
            }
        }
    }
    Ok(issues)
}

// ─────────────────────────────────────────────────────────────────────────────
// gc — garbage collect empty directories in the store
// ─────────────────────────────────────────────────────────────────────────────
pub fn gc(cfg: &Config) -> Result<usize> {
    let mut removed = 0;
    // Walk bottom-up, removing empty dirs
    for entry in walkdir::WalkDir::new(&cfg.store_dir)
        .contents_first(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path == cfg.store_dir { continue; }
        if path.is_dir() {
            if path.read_dir().map(|mut d| d.next().is_none()).unwrap_or(false) {
                std::fs::remove_dir(path)?;
                removed += 1;
            }
        }
    }
    Ok(removed)
}
