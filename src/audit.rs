// audit.rs — security checks

use anyhow::Result;
use sha1::{Digest, Sha1};

use crate::gen;
use crate::vault::{Entry, Vault};

#[derive(Debug, Clone)]
pub struct Issue {
    pub path: String,
    pub kind: IssueKind,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueKind {
    Weak,
    Duplicate,
    Aged,
    Breached,
}

impl Issue {
    pub fn label(&self) -> &'static str {
        match self.kind {
            IssueKind::Weak => "WEAK",
            IssueKind::Duplicate => "DUPE",
            IssueKind::Aged => "AGED",
            IssueKind::Breached => "BREACH",
        }
    }
}

// ── Full audit ────────────────────────────────────────────────────────────────

pub struct AuditReport {
    pub issues: Vec<Issue>,
    pub scanned: usize,
}

pub fn run_audit(vault: &Vault, max_age_days: u64) -> AuditReport {
    let paths = match vault.list() {
        Ok(p) => p,
        Err(_) => {
            return AuditReport {
                issues: vec![],
                scanned: 0,
            }
        }
    };
    let scanned = paths.len();

    let mut entries: Vec<(String, Entry)> = paths
        .iter()
        .filter_map(|p| vault.read(p).ok().map(|e| (p.clone(), e)))
        .collect();

    let mut issues = Vec::new();

    // Weak passwords
    for (path, entry) in &entries {
        if let Some(pw) = entry.fields.password.as_deref() {
            let s = gen::check_strength(pw);
            if s.score < 3 || pw.len() < 12 {
                issues.push(Issue {
                    path: path.clone(),
                    kind: IssueKind::Weak,
                    detail: format!("score {}/4, {} chars", s.score, pw.len()),
                });
            }
        }
    }

    // Duplicates
    let mut seen: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for (path, entry) in &entries {
        if let Some(pw) = entry.fields.password.as_deref() {
            seen.entry(pw.to_string()).or_default().push(path.clone());
        }
    }
    for (_, paths) in &seen {
        if paths.len() > 1 {
            for path in paths {
                issues.push(Issue {
                    path: path.clone(),
                    kind: IssueKind::Duplicate,
                    detail: format!("same password as {} other entries", paths.len() - 1),
                });
            }
        }
    }

    // Aged
    let now = chrono::Utc::now();
    for (path, entry) in &entries {
        let age = now.signed_duration_since(entry.meta.modified).num_days();
        if age as u64 > max_age_days {
            issues.push(Issue {
                path: path.clone(),
                kind: IssueKind::Aged,
                detail: format!("{} days since last change (limit {})", age, max_age_days),
            });
        }
    }

    AuditReport { issues, scanned }
}

// ── HIBP k-anonymity ─────────────────────────────────────────────────────────

pub fn hibp_check(password: &str, timeout_secs: u64) -> Result<u32> {
    let hash = format!("{:X}", Sha1::digest(password.as_bytes()));
    let prefix = &hash[..5];
    let suffix = &hash[5..];

    let url = format!("https://api.pwnedpasswords.com/range/{}", prefix);
    let resp = ureq::get(&url)
        .set("User-Agent", &format!("rtpv/{}", env!("CARGO_PKG_VERSION")))
        .set("Add-Padding", "true")
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .call()
        .map_err(|e| anyhow::anyhow!("HIBP request failed: {}", e))?;

    let body = resp.into_string()?;
    for line in body.lines() {
        if let Some((s, count)) = line.split_once(':') {
            if s.trim().eq_ignore_ascii_case(suffix) {
                return Ok(count.trim().parse().unwrap_or(0));
            }
        }
    }
    Ok(0)
}
