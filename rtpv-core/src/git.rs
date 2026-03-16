//! Git operations on the password store — sync, log, diff, commit.
//!
//! Uses the `git2` crate for native git without subprocess.

use std::path::Path;
use anyhow::{bail, Context, Result};
use git2::{Repository, Signature};

// ─────────────────────────────────────────────────────────────────────────────
// Store git wrapper
// ─────────────────────────────────────────────────────────────────────────────
pub struct StoreGit {
    repo: Repository,
}

impl StoreGit {
    pub fn open(store_dir: &Path) -> Result<Self> {
        let repo = Repository::open(store_dir)
            .with_context(|| format!("Store is not a git repo: {:?} — run: git init", store_dir))?;
        Ok(Self { repo })
    }

    pub fn is_git_repo(store_dir: &Path) -> bool {
        Repository::open(store_dir).is_ok()
    }

    /// Auto-commit a file change with a message.
    pub fn commit(&self, message: &str) -> Result<()> {
        let mut index = self.repo.index()?;
        index.add_all(["."], git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;

        let sig = self.sig()?;
        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;

        let parent_commit = match self.repo.head() {
            Ok(head) => Some(head.peel_to_commit()?),
            Err(_)   => None,
        };
        let parents: Vec<&git2::Commit> = parent_commit.iter().map(|c| c).collect();

        self.repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?;
        Ok(())
    }

    /// Pull from remote (fetch + merge fast-forward).
    pub fn pull(&self, remote_name: &str) -> Result<()> {
        let mut remote = self.repo.find_remote(remote_name)
            .or_else(|_| self.repo.find_remote("origin"))?;
        let remote_name = remote.name().unwrap_or("origin").to_string();
        remote.fetch(&[remote_name.as_str()], None, None)?;

        let fetch_head = self.repo.find_reference("FETCH_HEAD")?;
        let fetch_commit = self.repo.reference_to_annotated_commit(&fetch_head)?;
        let (analysis, _) = self.repo.merge_analysis(&[&fetch_commit])?;

        if analysis.is_up_to_date() {
            return Ok(());
        }
        if analysis.is_fast_forward() {
            let mut reference = self.repo.find_reference("HEAD")?;
            reference.set_target(fetch_commit.id(), "Fast-forward pull")?;
            self.repo.set_head("HEAD")?;
            self.repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
        } else {
            bail!("Cannot fast-forward — merge required. Use: git -C <store> pull");
        }
        Ok(())
    }

    /// Push to remote.
    pub fn push(&self, remote_name: &str) -> Result<()> {
        let mut remote = self.repo.find_remote(remote_name)
            .or_else(|_| self.repo.find_remote("origin"))?;
        let head = self.repo.head()?;
        let branch = head.shorthand().unwrap_or("main");
        remote.push(&[&format!("refs/heads/{}:refs/heads/{}", branch, branch)], None)?;
        Ok(())
    }

    /// Return last N commit messages.
    pub fn log(&self, n: usize) -> Result<Vec<CommitInfo>> {
        let mut walk = self.repo.revwalk()?;
        walk.push_head()?;
        walk.set_sorting(git2::Sort::TIME)?;
        let mut entries = Vec::new();
        for oid in walk.take(n) {
            let oid = oid?;
            let commit = self.repo.find_commit(oid)?;
            entries.push(CommitInfo {
                hash:    oid.to_string()[..8].to_string(),
                message: commit.summary().unwrap_or("").to_string(),
                author:  commit.author().name().unwrap_or("unknown").to_string(),
                time:    commit.time().seconds(),
            });
        }
        Ok(entries)
    }

    fn sig(&self) -> Result<Signature<'static>> {
        // Try git config, fall back to generic
        let cfg = self.repo.config()?;
        let name  = cfg.get_string("user.name").unwrap_or_else(|_| "rtpv".to_string());
        let email = cfg.get_string("user.email").unwrap_or_else(|_| "rtpv@localhost".to_string());
        Ok(Signature::now(&name, &email)?)
    }
}

#[derive(Debug)]
pub struct CommitInfo {
    pub hash:    String,
    pub message: String,
    pub author:  String,
    pub time:    i64,   // Unix timestamp
}
