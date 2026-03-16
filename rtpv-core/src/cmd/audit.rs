//! `rtpv audit` — run all security checks across the store.
//!
//!   rtpv audit          show weak / duplicate / aged passwords
//!   rtpv audit --fix    auto-rotate weak entries

use anyhow::Result;
use crate::config::Config;
use crate::store::Store;
use crate::audit::{self, AuditIssue, IssueKind};

pub struct AuditReport {
    pub issues: Vec<AuditIssue>,
    pub scanned: usize,
}

pub fn run(cfg: &Config, fix: bool) -> Result<AuditReport> {
    let store = Store::new(cfg)?;
    let paths = store.list()?;
    let scanned = paths.len();

    // Decrypt all entries (skip failures silently)
    let mut entries = Vec::new();
    for path in &paths {
        if let Ok(entry) = store.read(path) {
            entries.push(entry);
        }
    }

    let mut issues: Vec<AuditIssue> = Vec::new();

    // Strength check
    for entry in &entries {
        if let Some(issue) = audit::check_strength(entry) {
            issues.push(issue);
        }
    }

    // Duplicate check
    issues.extend(audit::find_duplicates(&entries));

    // Age check
    for entry in &entries {
        if let Some(issue) = audit::check_age(entry, cfg.max_age_days) {
            issues.push(issue);
        }
    }

    // Auto-fix weak entries if requested
    if fix {
        let weak_paths: Vec<String> = issues.iter()
            .filter(|i| i.kind == IssueKind::Weak)
            .map(|i| i.path.clone())
            .collect();
        for path in weak_paths {
            if let Ok(_) = crate::cmd::rotate::run(cfg, &path, None) {
                // Remove the issue from the list since it's fixed
                issues.retain(|i| !(i.path == path && i.kind == IssueKind::Weak));
            }
        }
    }

    Ok(AuditReport { issues, scanned })
}

pub fn hibp_all(cfg: &Config, delay_ms: u64) -> Result<Vec<AuditIssue>> {
    let store = Store::new(cfg)?;
    let paths = store.list()?;
    let mut issues = Vec::new();

    for (i, path) in paths.iter().enumerate() {
        if i > 0 {
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        }
        if let Ok(entry) = store.read(path) {
            if let Ok(Some(issue)) = audit::hibp_issue(&entry) {
                issues.push(issue);
            }
        }
    }
    Ok(issues)
}
