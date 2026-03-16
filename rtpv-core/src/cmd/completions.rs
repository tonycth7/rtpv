//! `rtpv completions` — print shell completion script.
//!
//! Completions are generated at runtime from the clap CLI definition
//! using clap_complete. No build-time code generation needed.
//!
//!   rtpv completions bash   → source in ~/.bashrc
//!   rtpv completions zsh    → source in ~/.zshrc
//!   rtpv completions fish   → source in fish config

// This module only contains the Shell enum re-exported for the CLI.
// The actual generation lives in rtpv-cli/src/completions.rs
// because clap_complete needs access to the full Cli struct.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shell { Bash, Zsh, Fish, Elvish, PowerShell }

impl std::str::FromStr for Shell {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s.to_lowercase().as_str() {
            "bash"       => Ok(Shell::Bash),
            "zsh"        => Ok(Shell::Zsh),
            "fish"       => Ok(Shell::Fish),
            "elvish"     => Ok(Shell::Elvish),
            "powershell" | "pwsh" => Ok(Shell::PowerShell),
            other => anyhow::bail!(
                "Unknown shell '{}'. Options: bash | zsh | fish | elvish | powershell", other
            ),
        }
    }
}
