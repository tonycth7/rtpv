//! Shell completion generation using clap_complete.
//!
//! Called from main.rs when the user runs `rtpv completions <shell>`.
//! Writes the completion script to stdout.
//!
//! Setup instructions printed per shell:
//!
//!   bash:  source <(rtpv completions bash)
//!          # or permanently:
//!          rtpv completions bash > ~/.local/share/bash-completion/completions/rtpv
//!
//!   zsh:   rtpv completions zsh > ~/.zfunc/_rtpv
//!          # then in ~/.zshrc: fpath+=~/.zfunc && autoload -Uz compinit && compinit
//!
//!   fish:  rtpv completions fish > ~/.config/fish/completions/rtpv.fish

use std::io;
use clap::CommandFactory;
use clap_complete::{generate, Shell as ClapShell};

use crate::Cli;

pub fn generate_completions(shell: &rtpv_core::cmd::completions::Shell) {
    let clap_shell = match shell {
        rtpv_core::cmd::completions::Shell::Bash       => ClapShell::Bash,
        rtpv_core::cmd::completions::Shell::Zsh        => ClapShell::Zsh,
        rtpv_core::cmd::completions::Shell::Fish       => ClapShell::Fish,
        rtpv_core::cmd::completions::Shell::Elvish     => ClapShell::Elvish,
        rtpv_core::cmd::completions::Shell::PowerShell => ClapShell::PowerShell,
    };

    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    generate(clap_shell, &mut cmd, name, &mut io::stdout());
}

pub fn print_setup(shell: &rtpv_core::cmd::completions::Shell) {
    use rtpv_core::cmd::completions::Shell::*;
    let hint = match shell {
        Bash => "\
# Add to ~/.bashrc:
source <(rtpv completions bash)

# Or install permanently:
rtpv completions bash > ~/.local/share/bash-completion/completions/rtpv",
        Zsh => "\
# Create the completions file:
mkdir -p ~/.zfunc
rtpv completions zsh > ~/.zfunc/_rtpv

# Add to ~/.zshrc (before compinit):
fpath+=~/.zfunc
autoload -Uz compinit && compinit",
        Fish => "\
# Install directly:
rtpv completions fish > ~/.config/fish/completions/rtpv.fish",
        Elvish => "\
# Add to ~/.config/elvish/rc.elv:
eval (rtpv completions elvish | slurp)",
        PowerShell => "\
# Add to $PROFILE:
Invoke-Expression (&rtpv completions powershell | Out-String)",
    };
    eprintln!("\n  Setup:\n\n{}\n", hint);
}
