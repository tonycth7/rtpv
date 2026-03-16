// main.rs — rtpv entry point
//
// - No args → launch TUI
// - Any subcommand → CLI mode

mod audit;
mod cli;
mod clipboard;
mod config;
mod gen;
mod git;
mod otp;
mod tui;
mod ui;
mod vault;

use anyhow::{bail, Result};
use clap::Parser;

use cli::commands::*;
use cli::{Cli, Commands, ImportCmd, OtpCmd, RecipientsCmd, SshCmd};
use config::Config;
use ui::Printer;
use vault::Vault;

fn main() {
    if let Err(e) = run() {
        eprintln!("\x1b[38;5;203m\x1b[1m  ✖  {}\x1b[0m", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // Load config
    let mut cfg = Config::load()?;
    if let Some(v) = &cli.vault {
        cfg.vault_dir = std::path::PathBuf::from(v);
    }
    if let Some(t) = &cli.theme {
        cfg.theme = config::Theme::from_str(t);
    }

    let printer = Printer::new(&cfg.theme);

    // No subcommand → open TUI
    let command = match cli.command {
        Some(c) => c,
        None => {
            let vault = open_vault(&cfg, &printer)?;
            let result = tui::run(vault, cfg)?;
            // Handle post-TUI actions (e.g. edit in $EDITOR)
            if let Some(tui::PostAction::Edit(path)) = result {
                let vault2 = open_vault(&Config::load()?, &printer)?;
                let cfg2 = Config::load()?;
                return cmd_edit(&vault2, &cfg2, &printer, &path);
            }
            return Ok(());
        }
    };

    // Subcommands that don't need a vault
    match &command {
        Commands::Init { force } => {
            return cmd_init(&cfg, &printer, *force);
        }
        Commands::GenConf => {
            cfg.write_sample()?;
            printer.ok(&format!(
                "Config written to {}/config.toml",
                cfg.config_dir.display()
            ));
            return Ok(());
        }
        Commands::Gen {
            words,
            pin,
            pronounceable,
            length,
            no_copy,
        } => {
            return cmd_gen(
                &cfg,
                &printer,
                *words,
                *pin,
                *pronounceable,
                *length,
                *no_copy,
            );
        }
        Commands::Doctor => {
            match Vault::open(&cfg.vault_dir) {
                Ok(vault) => cmd_doctor(&vault, &cfg, &printer),
                Err(_) => {
                    printer.warn("Vault not initialized — run: rtpv init");
                    // Still show what we can
                    printer.banner("Doctor");
                    printer.warn(&format!("vault directory: {}", cfg.vault_dir.display()));
                }
            }
            return Ok(());
        }
        _ => {}
    }

    // All other commands need an open vault
    let vault = open_vault(&cfg, &printer)?;

    match command {
        Commands::Init { .. } | Commands::GenConf | Commands::Gen { .. } | Commands::Doctor => {
            unreachable!()
        }

        Commands::Add {
            path,
            kind,
            email,
            username,
            url,
            notes,
            password,
            words,
            pin,
            length,
            no_print,
        } => {
            cmd_add(
                &vault,
                &cfg,
                &printer,
                AddArgs {
                    path,
                    kind: vault::EntryKind::from_str(&kind),
                    email,
                    username,
                    url,
                    notes,
                    password,
                    words,
                    pin,
                    length,
                    no_print,
                },
            )?;
        }

        Commands::Show { path, reveal } => {
            cmd_show(&vault, &printer, &path, reveal)?;
        }

        Commands::Copy {
            path,
            username,
            email,
            token,
            otp,
            notes,
            field,
        } => {
            cmd_copy(
                &vault,
                &cfg,
                &printer,
                &path,
                username,
                email,
                token,
                otp,
                notes,
                field.as_deref(),
            )?;
        }

        Commands::Remove { path, force } => {
            cmd_remove(&vault, &cfg, &printer, &path, force)?;
        }

        Commands::Edit { path } => {
            cmd_edit(&vault, &cfg, &printer, &path)?;
        }

        Commands::Rename { src, dst } => {
            cmd_rename(&vault, &cfg, &printer, &src, &dst)?;
        }

        Commands::Clone { src, dst } => {
            cmd_clone(&vault, &cfg, &printer, &src, &dst)?;
        }

        Commands::Set { path, field, value } => {
            cmd_set(&vault, &cfg, &printer, &path, &field, value)?;
        }

        Commands::Rotate { path, length } => {
            cmd_rotate(&vault, &cfg, &printer, &path, length)?;
        }

        Commands::List { filter } => {
            cmd_list(&vault, &printer, filter.as_deref())?;
        }

        Commands::Otp { sub } => match sub {
            OtpCmd::Copy { path } => cmd_otp_copy(&vault, &printer, &path)?,
            OtpCmd::Show { path } => cmd_otp_show(&vault, &printer, &path)?,
            OtpCmd::List => cmd_otp_list(&vault, &printer)?,
            OtpCmd::Add { path, uri } => cmd_otp_add(&vault, &cfg, &printer, &path, &uri)?,
        },

        Commands::Audit { hibp, fix } => {
            cmd_audit(&vault, &cfg, &printer, hibp, fix)?;
        }

        Commands::Hibp { path } => {
            cmd_hibp(&vault, &cfg, &printer, &path)?;
        }

        Commands::Strength { path } => {
            cmd_strength(&vault, &printer, &path)?;
        }

        Commands::Sync { remote } => {
            cmd_sync(&vault, &cfg, &printer, &remote)?;
        }

        Commands::Log { count } => {
            cmd_log(&cfg, &printer, count)?;
        }

        Commands::Import { sub } => match sub {
            ImportCmd::Bitwarden { path } => cmd_import_bitwarden(&vault, &cfg, &printer, &path)?,
            ImportCmd::Csv { path } => cmd_import_csv(&vault, &cfg, &printer, &path)?,
            ImportCmd::Firefox { path } => cmd_import_csv(&vault, &cfg, &printer, &path)?,
            ImportCmd::Chrome { path } => cmd_import_csv(&vault, &cfg, &printer, &path)?,
        },

        Commands::Migrate { store_path, gpg: _ } => {
            cmd_migrate(&vault, &cfg, &printer, store_path.as_deref())?;
        }

        Commands::Export { output } => {
            cmd_export(&vault, &printer, &output)?;
        }

        Commands::Fill { path, delay, enter } => {
            cmd_fill(&vault, &printer, &path, delay, enter)?;
        }

        Commands::Run { path, command } => {
            cmd_run(&vault, &path, &command)?;
        }

        Commands::Ssh { sub } => match sub {
            SshCmd::Add {
                name,
                private_key,
                public_key,
            } => cmd_ssh_add(
                &vault,
                &cfg,
                &printer,
                &name,
                &private_key,
                public_key.as_deref(),
            )?,
            SshCmd::Restore { name } => cmd_ssh_restore(&vault, &printer, &name)?,
            SshCmd::CopyPub { name } => {
                if let Ok(entry) = vault.read(&format!("ssh/{}", name)) {
                    if let Some(pub_key) = &entry.fields.pub_key {
                        clipboard::copy(pub_key)?;
                        printer.ok("Public key copied");
                    } else {
                        bail!("No public key stored for '{}'", name);
                    }
                }
            }
            SshCmd::List => cmd_ssh_list(&vault, &printer)?,
            SshCmd::Generate { name, comment } => {
                cmd_ssh_generate(&vault, &cfg, &printer, &name, &comment)?
            }
        },

        Commands::Recipients { sub } => match sub {
            RecipientsCmd::List => cmd_recipients_list(&vault, &printer)?,
            RecipientsCmd::Add { pubkey } => cmd_recipients_add(&vault, &printer, &pubkey)?,
            RecipientsCmd::Remove { pubkey } => cmd_recipients_remove(&vault, &printer, &pubkey)?,
            RecipientsCmd::Reencrypt => cmd_recipients_reencrypt(&vault, &printer)?,
        },

        Commands::Lock => {
            clipboard::clear()?;
            printer.ok("Clipboard cleared");
        }

        Commands::Recent { count } => {
            cmd_recent(&cfg, &printer, count)?;
        }
    }

    Ok(())
}

fn open_vault(cfg: &Config, printer: &Printer) -> Result<Vault> {
    Vault::open(&cfg.vault_dir).map_err(|e| {
        anyhow::anyhow!(
            "{}\n\n  Run: rtpv init\n  Or set RTPV_VAULT to your vault directory",
            e
        )
    })
}
