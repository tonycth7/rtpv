//! Audit — password strength checks, HIBP breach checking, age audit.

use anyhow::{Context, Result};
use sha1::{Sha1, Digest};

use crate::store::Entry;

// ─────────────────────────────────────────────────────────────────────────────
// Per-entry issues
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct AuditIssue {
    pub path:   String,
    pub kind:   IssueKind,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueKind {
    Weak,
    Duplicate,
    Aged,
    Breached,
}

// ─────────────────────────────────────────────────────────────────────────────
// Strength audit
// ─────────────────────────────────────────────────────────────────────────────
pub fn check_strength(entry: &Entry) -> Option<AuditIssue> {
    let report = crate::gen::strength(&entry.password);
    if report.score < 3 || entry.password.len() < 12 {
        Some(AuditIssue {
            path:   entry.path.clone(),
            kind:   IssueKind::Weak,
            detail: format!("score {}/4, {} chars", report.score, entry.password.len()),
        })
    } else {
        None
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Duplicate detection
// ─────────────────────────────────────────────────────────────────────────────
pub fn find_duplicates(entries: &[Entry]) -> Vec<AuditIssue> {
    use std::collections::HashMap;
    let mut by_password: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in entries {
        by_password.entry(&e.password).or_default().push(&e.path);
    }
    let mut issues = Vec::new();
    for (_, paths) in by_password {
        if paths.len() > 1 {
            for path in &paths {
                issues.push(AuditIssue {
                    path:   path.to_string(),
                    kind:   IssueKind::Duplicate,
                    detail: format!("same password as {} other entries", paths.len() - 1),
                });
            }
        }
    }
    issues
}

// ─────────────────────────────────────────────────────────────────────────────
// Age audit
// ─────────────────────────────────────────────────────────────────────────────
pub fn check_age(entry: &Entry, max_days: u64) -> Option<AuditIssue> {
    let modified = entry.modified?;
    let age = chrono::Utc::now().signed_duration_since(modified);
    if age.num_days() as u64 > max_days {
        Some(AuditIssue {
            path:   entry.path.clone(),
            kind:   IssueKind::Aged,
            detail: format!("last changed {} days ago (max {})", age.num_days(), max_days),
        })
    } else {
        None
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// HIBP — k-anonymity breach check
// ─────────────────────────────────────────────────────────────────────────────
pub fn hibp_check(password: &str) -> Result<u32> {
    let hash = format!("{:X}", Sha1::digest(password.as_bytes()));
    let prefix = &hash[..5];
    let suffix = &hash[5..];

    let url = format!("https://api.pwnedpasswords.com/range/{}", prefix);
    let resp = reqwest::blocking::Client::new()
        .get(&url)
        .header("User-Agent", concat!("rtpv/", env!("CARGO_PKG_VERSION")))
        .header("Add-Padding", "true")
        .send()
        .context("HIBP request failed — check network connection")?;

    if !resp.status().is_success() {
        anyhow::bail!("HIBP API error: {}", resp.status());
    }

    let body = resp.text()?;
    for line in body.lines() {
        if let Some((s, count)) = line.split_once(':') {
            if s.trim().eq_ignore_ascii_case(suffix) {
                return Ok(count.trim().parse().unwrap_or(0));
            }
        }
    }
    Ok(0) // Not found = not breached
}

pub fn hibp_issue(entry: &Entry) -> Result<Option<AuditIssue>> {
    let count = hibp_check(&entry.password)?;
    if count > 0 {
        Ok(Some(AuditIssue {
            path:   entry.path.clone(),
            kind:   IssueKind::Breached,
            detail: format!("found {} times in HIBP database", count),
        }))
    } else {
        Ok(None)
    }
}
