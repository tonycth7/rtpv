// git.rs — git sync over SSH (no HTTPS, no credentials in config)

use anyhow::{bail, Context, Result};
use git2::{Repository, Signature};
use std::path::Path;

pub struct GitSync {
    repo: Repository,
}

impl GitSync {
    pub fn open(vault_dir: &Path) -> Result<Self> {
        let repo = Repository::open(vault_dir)
            .context("Vault is not a git repo. Run: git init && git remote add origin git@github.com:user/vault.git")?;
        Ok(Self { repo })
    }

    pub fn is_repo(vault_dir: &Path) -> bool {
        Repository::open(vault_dir).is_ok()
    }

    /// Stage all changes and commit.
    pub fn commit(&self, message: &str) -> Result<()> {
        let mut index = self.repo.index()?;
        index.add_all(["."], git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;

        let sig = self.sig()?;
        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;

        let parent: Option<git2::Commit> = match self.repo.head() {
            Ok(h) => h.peel_to_commit().ok(),
            Err(_) => None,
        };
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        self.repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?;
        Ok(())
    }

    /// Pull from SSH remote — fetch then fast-forward merge only.
    pub fn pull(&self, remote_name: &str) -> Result<()> {
        // Validate URL before borrowing mutably
        {
            let remote = self.find_remote(remote_name)?;
            self.validate_ssh_remote(&remote)?;
        }

        let mut remote = self.find_remote(remote_name)?;
        let remote_name_str = remote.name().unwrap_or("origin").to_string();

        let mut cb = git2::RemoteCallbacks::new();
        cb.credentials(ssh_credentials_cb);
        let mut fetch_opts = git2::FetchOptions::new();
        fetch_opts.remote_callbacks(cb);

        remote.fetch(&[remote_name_str.as_str()], Some(&mut fetch_opts), None)?;

        let fetch_head = self.repo.find_reference("FETCH_HEAD")?;
        let fetch_commit = self.repo.reference_to_annotated_commit(&fetch_head)?;
        let (analysis, _): (git2::MergeAnalysis, _) = self.repo.merge_analysis(&[&fetch_commit])?;
        if analysis.is_up_to_date() {
            return Ok(());
        }
        if analysis.is_fast_forward() {
            let mut head = self.repo.find_reference("HEAD")?;
            head.set_target(fetch_commit.id(), "Fast-forward")?;
            self.repo
                .checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
        } else {
            bail!("Cannot fast-forward — merge conflict. Resolve manually in the vault directory.");
        }
        Ok(())
    }

    /// Push to SSH remote.
    pub fn push(&self, remote_name: &str) -> Result<()> {
        {
            let remote = self.find_remote(remote_name)?;
            self.validate_ssh_remote(&remote)?;
        }

        let head = self.repo.head()?;
        let branch = head.shorthand().unwrap_or("main").to_string();
        let refspec = format!("refs/heads/{}:refs/heads/{}", branch, branch);

        let mut remote = self.find_remote(remote_name)?;
        let mut cb = git2::RemoteCallbacks::new();
        cb.credentials(ssh_credentials_cb);
        let mut push_opts = git2::PushOptions::new();
        push_opts.remote_callbacks(cb);

        remote.push(&[refspec.as_str()], Some(&mut push_opts))?;
        Ok(())
    }

    /// Recent commits log.
    pub fn log(&self, count: usize) -> Result<Vec<CommitInfo>> {
        let mut walk = self.repo.revwalk()?;
        walk.push_head()?;
        walk.set_sorting(git2::Sort::TIME)?;
        walk.take(count)
            .filter_map(|oid: Result<git2::Oid, _>| oid.ok())
            .filter_map(|oid| self.repo.find_commit(oid).ok())
            .map(|c: git2::Commit| {
                Ok(CommitInfo {
                    hash: c.id().to_string()[..8].to_string(),
                    message: c.summary().unwrap_or("").to_string(),
                    author: c.author().name().unwrap_or("unknown").to_string(),
                    time: c.time().seconds(),
                })
            })
            .collect()
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn find_remote(&self, name: &str) -> Result<git2::Remote<'_>> {
        self.repo.find_remote(name)
            .or_else(|_| self.repo.find_remote("origin"))
            .context("No git remote configured.\nRun: git remote add origin git@github.com:user/vault.git")
    }

    fn validate_ssh_remote(&self, remote: &git2::Remote<'_>) -> Result<()> {
        let url = remote.url().unwrap_or("");
        if url.starts_with("https://") || url.starts_with("http://") {
            bail!(
                "Remote '{}' uses HTTPS — rtpv requires SSH.\n\
                 Fix with: git remote set-url origin git@github.com:user/vault.git",
                url
            );
        }
        Ok(())
    }

    fn sig(&self) -> Result<Signature<'static>> {
        let cfg = self.repo.config()?;
        let name = cfg
            .get_string("user.name")
            .unwrap_or_else(|_| "rtpv".to_string());
        let email = cfg
            .get_string("user.email")
            .unwrap_or_else(|_| "rtpv@localhost".to_string());
        Ok(Signature::now(&name, &email)?)
    }
}

pub struct CommitInfo {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub time: i64,
}

/// SSH credential callback — tries agent first, then ~/.ssh/id_* keys.
fn ssh_credentials_cb(
    _url: &str,
    username: Option<&str>,
    allowed: git2::CredentialType,
) -> std::result::Result<git2::Cred, git2::Error> {
    let user = username.unwrap_or("git");

    if allowed.contains(git2::CredentialType::SSH_KEY) {
        // 1. Try ssh-agent
        if let Ok(cred) = git2::Cred::ssh_key_from_agent(user) {
            return Ok(cred);
        }
        // 2. Try common key files
        let home = dirs::home_dir().unwrap_or_default();
        for key_name in &["id_ed25519", "id_rsa", "id_ecdsa"] {
            let pvt = home.join(".ssh").join(key_name);
            let pub_ = home.join(".ssh").join(format!("{}.pub", key_name));
            if pvt.exists() {
                return git2::Cred::ssh_key(
                    user,
                    pub_.exists().then_some(pub_.as_path()),
                    &pvt,
                    None,
                );
            }
        }
    }

    Err(git2::Error::from_str(
        "No SSH credentials available — start ssh-agent or add key",
    ))
}
