// rtpv — CLI entry point.
//
// All output formatting lives here. Core logic is in rtpv-core.

use anyhow::Result;
use clap::{Parser, Subcommand};
use rtpv_core::{
    config::{Config, CryptoBackend},
    cmd,
};

mod ui;
mod completions;
mod manpage;
use ui::Ui;

// ─────────────────────────────────────────────────────────────────────────────
// CLI definition
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Parser)]
#[command(
    name    = "rtpv",
    about   = "rtpv — power-user password manager",
    version = rtpv_core::VERSION,
    propagate_version = true,
    arg_required_else_help = false,
)]
struct Cli {
    /// Crypto backend: age (default) or gpg
    #[arg(long, global = true, env = "RTPV_BACKEND")]
    backend: Option<String>,

    /// Override store directory
    #[arg(long, global = true, env = "PASSWORD_STORE_DIR")]
    store: Option<String>,

    /// UI theme
    #[arg(long, global = true, env = "RTPV_THEME")]
    theme: Option<String>,

    /// Output JSON where supported
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize the store and generate an age identity
    Init {
        /// Force re-generate identity even if one exists
        #[arg(long)]
        force: bool,
        /// Use gpg backend instead of age
        #[arg(long)]
        gpg:   bool,
        /// GPG key ID (required with --gpg)
        key_id: Option<String>,
    },

    /// Add a new entry
    Add {
        /// Entry path (e.g. github or work/slack)
        path: String,
        /// Email address
        #[arg(short, long)]
        email: Option<String>,
        /// Username
        #[arg(short, long)]
        username: Option<String>,
        /// URL
        #[arg(short = 'l', long)]
        url: Option<String>,
        /// Notes
        #[arg(short, long)]
        notes: Option<String>,
        /// Prompt for your own password instead of generating
        #[arg(short = 'p', long)]
        password: bool,
        /// Provide password directly (use with caution — visible in shell history)
        #[arg(long, value_name = "PW")]
        pw: Option<String>,
        /// Generate a passphrase (n words)
        #[arg(long, value_name = "N")]
        words: Option<usize>,
        /// Generate a numeric PIN
        #[arg(long, value_name = "N")]
        pin: Option<usize>,
        /// Password length for generated passwords
        #[arg(short = 'L', long)]
        length: Option<usize>,
        /// Don't print generated password
        #[arg(short = 'n', long)]
        no_print: bool,
    },

    /// Show an entry or a specific field (path:field syntax supported)
    Show {
        /// Entry path, optionally with :field suffix
        path: String,
        /// Print password in clear text
        #[arg(long)]
        reveal: bool,
    },

    /// Copy a field to clipboard
    #[command(alias = "cp")]
    Copy {
        path: String,
        /// Copy username instead of password
        #[arg(short = 'u', long)]
        username: bool,
        /// Copy email
        #[arg(short = 'e', long)]
        email: bool,
        /// Copy token
        #[arg(short = 't', long)]
        token: bool,
        /// Copy current OTP code
        #[arg(long)]
        otp: bool,
        /// Clipboard timeout in seconds
        #[arg(long)]
        timeout: Option<u64>,
    },

    /// Edit an entry in $EDITOR
    Edit { path: String },

    /// Rename an entry
    Rename { src: String, dst: String },

    /// Clone an entry to a new path
    Clone { src: String, dst: String },

    /// Delete an entry
    #[command(alias = "rm")]
    Delete {
        path: String,
        #[arg(short, long)]
        force: bool,
    },

    /// Add or update a field in an existing entry
    SetField {
        path:  String,
        field: String,
        value: Option<String>,
    },

    /// Rotate (replace) the password of an entry
    Rotate {
        path: String,
        #[arg(short = 'L', long)]
        length: Option<usize>,
    },

    /// Generate a password without storing it
    Gen {
        /// Length (for random mode)
        length: Option<usize>,
        #[arg(long, value_name = "N")]
        words: Option<usize>,
        #[arg(long, value_name = "N")]
        pin: Option<usize>,
        #[arg(long)]
        pronounceable: bool,
        /// Don't copy to clipboard
        #[arg(long)]
        no_copy: bool,
    },

    /// Fuzzy search entry names
    Search { query: String },

    /// List all entries
    Ls,

    // ── OTP ──────────────────────────────────────────────────────
    /// OTP commands
    Otp {
        #[command(subcommand)]
        sub: OtpCmd,
    },

    // ── Security ─────────────────────────────────────────────────
    /// Audit all entries for weak / duplicate / aged passwords
    Audit {
        #[arg(long)]
        fix: bool,
        #[arg(long)]
        hibp: bool,
    },

    /// Check a single entry against HIBP breach database
    Hibp { path: Option<String> },

    /// Show password strength for an entry
    Strength { path: String },

    /// Show Shannon entropy of a password
    Entropy { path: String },

    // ── Autofill ─────────────────────────────────────────────────
    /// Type username + TAB + password into focused window
    Fill {
        path: String,
        #[arg(long, default_value = "500")]
        delay: u64,
    },

    // ── Templates ────────────────────────────────────────────────
    /// Create entry from a template
    #[command(alias = "t")]
    Template {
        /// Template type (web-login, server, database, api-key, ...)
        #[arg(short, long)]
        kind: Option<String>,
        /// Entry path
        path: Option<String>,
        /// Prompt for own password
        #[arg(short = 'p', long)]
        ask: bool,
    },

    // ── Notes / Env ───────────────────────────────────────────────
    /// Secure note management
    Note {
        #[command(subcommand)]
        sub: NoteCmd,
    },

    /// Export credentials as shell environment variables
    Env { path: String },

    /// Export credentials as .env file format
    Dotenv { path: String },

    /// Run a command with credentials injected as environment variables
    Run {
        path: String,
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },

    // ── SSH ───────────────────────────────────────────────────────
    /// SSH key management
    Ssh {
        #[command(subcommand)]
        sub: SshCmd,
    },

    // ── Import / Export ───────────────────────────────────────────
    /// Import from another password manager
    Import {
        #[command(subcommand)]
        sub: ImportCmd,
    },

    /// Export to Bitwarden JSON format
    ExportBitwarden {
        #[arg(short, long, default_value = "bitwarden-export.json")]
        output: String,
    },

    // ── Maintenance ───────────────────────────────────────────────
    /// Sync store with git remote
    Sync,

    /// Show git log for the store
    Log {
        #[arg(short, long, default_value = "20")]
        count: usize,
    },

    /// Show git diff for an entry between commits
    Diff { path: String },

    /// Health check — verify all tools and configuration
    Doctor,

    /// Store statistics
    Stats,

    /// Garbage-collect empty directories
    Gc,

    /// Lint entries for issues
    Lint,

    /// Generate a sample config file
    GenConf,

    /// Migrate store from gpg to age encryption
    MigrateToAge,

    /// Lock (clear clipboard, kill agent)
    Lock,

    /// List recently modified entries
    Recent {
        #[arg(short = 'n', long, default_value = "10")]
        count: usize,
    },

    /// Create an encrypted backup of the store
    Backup {
        /// Output path (default: rtpv-backup-<timestamp>.tar.age)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Restore from an encrypted backup
    Restore {
        /// Path to .tar.age backup file
        path: String,
        /// Target directory (default: current directory)
        #[arg(short, long)]
        dir:  Option<String>,
    },

    /// GPG key management
    Gpg {
        #[command(subcommand)]
        sub: GpgCmd,
    },

    /// Credit card management
    Card {
        #[command(subcommand)]
        sub: CardCmd,
    },

    /// Share entries and manage recipients
    Share {
        #[command(subcommand)]
        sub: ShareCmd,
    },

    /// Generate and print a man page to stdout
    Man,

    /// Generate shell completion script
    Completions {
        /// Shell to generate for: bash | zsh | fish | elvish | powershell
        shell: String,
        /// Print setup instructions instead of the script
        #[arg(long)]
        setup: bool,
    },
}

#[derive(Subcommand)]
enum OtpCmd {
    /// Copy current OTP code for an entry
    Copy { path: String },
    /// Show OTP code with countdown
    Show { path: String },
    /// List all entries with OTP configured
    List,
    /// Import an otpauth:// URI into an entry
    Import { path: String, uri: String },
    /// Import from Google Authenticator migration QR payload
    ImportQr {
        payload: String,
        #[arg(long, default_value = "imported")]
        prefix:  String,
    },
}

#[derive(Subcommand)]
enum NoteCmd {
    /// Add a new note
    Add { path: String },
    /// Show a note
    Show { path: String },
    /// Edit a note in $EDITOR
    Edit { path: String },
    /// Copy note to clipboard
    Copy { path: String },
    /// List all note entries
    List,
}

#[derive(Subcommand)]
enum SshCmd {
    /// Store an SSH keypair
    Add {
        name:        String,
        private_key: String,
        public_key:  Option<String>,
    },
    /// Restore a keypair to ~/.ssh
    Set { name: String },
    /// Copy the public key to clipboard
    CopyId { name: String },
    /// List stored SSH keys
    List,
}

#[derive(Subcommand)]
enum ImportCmd {
    Bitwarden { path: String },
    Firefox   { path: String },
    Chrome    { path: String },
    Keepass   { path: String },
}

#[derive(Subcommand)]
enum GpgCmd {
    /// Export a key from the keyring and store it
    Add {
        /// GPG key ID or email
        key_id: String,
        /// Custom name for the store entry (default: last 8 of fingerprint)
        #[arg(short, long)]
        name: Option<String>,
    },
    /// Import a stored key back into the keyring
    Set { name: String },
    /// Copy the stored public key to clipboard
    Copy { name: String },
    /// Show the stored fingerprint
    Fingerprint { name: String },
    /// List stored GPG keys
    List,
}

#[derive(Subcommand)]
enum CardCmd {
    /// List all credit card entries
    List,
    /// Copy card number to clipboard
    Copy { path: String },
    /// Copy CVV to clipboard
    Cvv  { path: String },
    /// Show full card details
    Show { path: String },
}

#[derive(Subcommand)]
enum ShareCmd {
    /// Share an entry with another age public key
    Send {
        path:      String,
        /// Recipient age public key (age1...)
        recipient: String,
        /// Output file path
        #[arg(short, long)]
        out: Option<String>,
    },
    /// List configured recipients
    Recipients,
    /// Add a recipient public key to recipients.txt
    AddRecipient { key: String },
    /// Remove a recipient
    RemoveRecipient { key: String },
    /// Re-encrypt entire store to current recipients
    Reencrypt,
}

// ─────────────────────────────────────────────────────────────────────────────
// Entry point
// ─────────────────────────────────────────────────────────────────────────────
fn main() {
    let cli = Cli::parse();
    let mut cfg = Config::load().unwrap_or_else(|e| {
        eprintln!("Config error: {}", e);
        std::process::exit(1);
    });

    // CLI overrides
    if let Some(b) = &cli.backend {
        cfg.crypto_backend = b.parse().unwrap_or(CryptoBackend::Age);
    }
    if let Some(s) = &cli.store {
        cfg.store_dir = std::path::PathBuf::from(s);
    }
    if let Some(t) = &cli.theme {
        cfg.theme = t.parse().unwrap_or(rtpv_core::config::Theme::Catppuccin);
    }
    cfg.json_output = cli.json;

    let ui = Ui::new(&cfg);

    if let Err(e) = run(cli.command, &cfg, &ui) {
        ui.error(&e.to_string());
        std::process::exit(1);
    }
}

fn run(command: Option<Commands>, cfg: &Config, ui: &Ui) -> Result<()> {
    let command = match command {
        Some(c) => c,
        None    => {
            // No subcommand → list entries
            let store = rtpv_core::store::Store::new(cfg)?;
            let entries = store.list()?;
            for e in &entries { println!("{}", e); }
            ui.dim(&format!("{} entries", entries.len()));
            return Ok(());
        }
    };

    match command {
        // ── Init ─────────────────────────────────────────────────────────────
        Commands::Init { force, gpg, key_id } => {
            let mut c = cfg.clone();
            if gpg {
                c.crypto_backend = CryptoBackend::Gpg;
                if let Some(id) = key_id {
                    cmd::init::write_gpg_id(&c.store_dir, &id)?;
                }
            }
            let result = cmd::init::run(&c, force)?;
            if result.created {
                ui.ok("Store initialized");
                ui.info(&format!("Backend:    {:?}", result.backend));
                ui.info(&format!("Store:      {}", result.store_dir.display()));
                ui.info(&format!("Public key: {}", result.public_key));
            } else {
                ui.info("Store already initialized (use --force to re-generate identity)");
                ui.dim(&format!("Public key: {}", result.public_key));
            }
        }

        // ── Add ──────────────────────────────────────────────────────────────
        Commands::Add { path, email, username, url, notes, password, pw, words, pin, length, no_print } => {
            let mode = if pin.is_some() { cmd::add::PasswordMode::Pin }
                       else if words.is_some() { cmd::add::PasswordMode::Words }
                       else if password || pw.is_some() { cmd::add::PasswordMode::Manual }
                       else { cmd::add::PasswordMode::Generate };

            let args = cmd::add::AddArgs {
                path:     path.clone(),
                email,
                username,
                url,
                notes,
                mode,
                password: pw,
                length,
                words,
                pin,
                autosync: cfg.autosync,
            };

            let result = cmd::add::run(cfg, args, || prompt_password("Password"))?;

            ui.ok(&format!("Added: {}", path));
            if !no_print {
                ui.dim(&format!("Password: {}", result.password));
            }
            if rtpv_core::clipboard::available() {
                rtpv_core::clipboard::copy_with_clear(&result.password, cfg.clip_timeout)?;
                ui.dim(&format!("Copied to clipboard (clears in {}s)", cfg.clip_timeout));
            }
        }

        // ── Show ─────────────────────────────────────────────────────────────
        Commands::Show { path, reveal } => {
            let result = cmd::show::run(cfg, &path)?;
            if let Some((_field, value)) = result.field {
                println!("{}", value);
            } else {
                ui.banner(&result.entry.path);
                println!("{}", cmd::show::format_entry(&result.entry, reveal));
            }
        }

        // ── Copy ─────────────────────────────────────────────────────────────
        Commands::Copy { path, username, email, token, otp, timeout } => {
            let field = if username { cmd::copy::CopyField::Username }
                        else if email  { cmd::copy::CopyField::Email    }
                        else if token  { cmd::copy::CopyField::Token    }
                        else if otp    { cmd::copy::CopyField::Otp      }
                        else           { cmd::copy::CopyField::Password };

            let args = cmd::copy::CopyArgs {
                path:    path.clone(),
                field,
                timeout: timeout.unwrap_or(cfg.clip_timeout),
                notify:  cfg.notify,
                silent:  false,
            };
            let result = cmd::copy::run(cfg, args)?;
            ui.ok(&format!("Copied {} for {} (clears in {}s)",
                result.field_name, path, cfg.clip_timeout));
        }

        // ── Edit ─────────────────────────────────────────────────────────────
        Commands::Edit { path } => {
            cmd::edit::run(cfg, &path)?;
            ui.ok(&format!("Saved: {}", path));
        }

        // ── Rename ───────────────────────────────────────────────────────────
        Commands::Rename { src, dst } => {
            let store = rtpv_core::store::Store::new(cfg)?;
            store.rename(&src, &dst)?;
            if let Ok(git) = rtpv_core::git::StoreGit::open(&cfg.store_dir) {
                let _ = git.commit(&format!("Rename {} → {}", src, dst));
            }
            ui.ok(&format!("{} → {}", src, dst));
        }

        // ── Clone ────────────────────────────────────────────────────────────
        Commands::Clone { src, dst } => {
            let store = rtpv_core::store::Store::new(cfg)?;
            store.clone_entry(&src, &dst)?;
            if let Ok(git) = rtpv_core::git::StoreGit::open(&cfg.store_dir) {
                let _ = git.commit(&format!("Clone {} → {}", src, dst));
            }
            ui.ok(&format!("Cloned {} → {}", src, dst));
        }

        // ── Delete ───────────────────────────────────────────────────────────
        Commands::Delete { path, force } => {
            if !force {
                eprint!("  Delete '{}'? [y/N] ", path);
                let mut yn = String::new();
                std::io::stdin().read_line(&mut yn)?;
                if !yn.trim().eq_ignore_ascii_case("y") {
                    ui.dim("Aborted");
                    return Ok(());
                }
            }
            let store = rtpv_core::store::Store::new(cfg)?;
            store.remove(&path)?;
            if let Ok(git) = rtpv_core::git::StoreGit::open(&cfg.store_dir) {
                let _ = git.commit(&format!("Delete {}", path));
            }
            ui.ok(&format!("Deleted: {}", path));
        }

        // ── SetField ─────────────────────────────────────────────────────────
        Commands::SetField { path, field, value } => {
            let value = match value {
                Some(v) => v,
                None    => {
                    let is_secret = matches!(field.as_str(), "password" | "token" | "cvv" | "number");
                    if is_secret { prompt_password(&format!("Value for {}", field))? }
                    else         { prompt_text(&format!("Value for {}", field))? }
                }
            };
            let store = rtpv_core::store::Store::new(cfg)?;
            store.set_field(&path, &field, &value)?;
            if let Ok(git) = rtpv_core::git::StoreGit::open(&cfg.store_dir) {
                let _ = git.commit(&format!("Update {} in {}", field, path));
            }
            ui.ok(&format!("Updated field '{}' in '{}'", field, path));
        }

        // ── Rotate ───────────────────────────────────────────────────────────
        Commands::Rotate { path, length } => {
            let result = cmd::rotate::run(cfg, &path, length)?;
            ui.ok(&format!("Rotated password for {}", path));
            if rtpv_core::clipboard::available() {
                rtpv_core::clipboard::copy_with_clear(&result.new_password, cfg.clip_timeout)?;
                ui.dim(&format!("New password copied (clears in {}s)", cfg.clip_timeout));
            }
        }

        // ── Gen ──────────────────────────────────────────────────────────────
        Commands::Gen { length, words, pin, pronounceable, no_copy } => {
            let pw = if let Some(n) = words {
                rtpv_core::gen::passphrase(n, '-')?
            } else if let Some(n) = pin {
                rtpv_core::gen::pin(n)?
            } else if pronounceable {
                rtpv_core::gen::pronounceable(length.unwrap_or(16))?
            } else {
                rtpv_core::gen::random_password(length.unwrap_or(cfg.gen_length), &cfg.gen_chars)?
            };
            println!("{}", pw);
            if !no_copy && rtpv_core::clipboard::available() {
                rtpv_core::clipboard::copy_with_clear(&pw, cfg.clip_timeout)?;
                ui.dim(&format!("Copied (clears in {}s)", cfg.clip_timeout));
            }
        }

        // ── Search ───────────────────────────────────────────────────────────
        Commands::Search { query } => {
            let result = cmd::search::run(cfg, &query)?;
            for m in &result.matches {
                println!("{}", m.path);
            }
            ui.dim(&format!("{} matches", result.matches.len()));
        }

        // ── Ls ───────────────────────────────────────────────────────────────
        Commands::Ls => {
            let store = rtpv_core::store::Store::new(cfg)?;
            let entries = store.list()?;
            for e in &entries { println!("{}", e); }
        }

        // ── OTP ──────────────────────────────────────────────────────────────
        Commands::Otp { sub } => match sub {
            OtpCmd::Copy { path } => {
                let state = cmd::otp::copy_code(cfg, &path)?;
                ui.ok(&format!("OTP copied ({} seconds remaining)", state.remaining));
            }
            OtpCmd::Show { path } => {
                let store = rtpv_core::store::Store::new(cfg)?;
                let entry = store.read(&path)?;
                let uri = entry.otp_uri()
                    .ok_or_else(|| anyhow::anyhow!("No OTP for '{}'", path))?;
                let state = rtpv_core::otp::code_for_uri(uri)?;
                println!("  Code:      {}", state.code);
                println!("  Remaining: {}s", state.remaining);
                if let Some(i) = &state.issuer  { println!("  Issuer:    {}", i); }
                if let Some(a) = &state.account { println!("  Account:   {}", a); }
            }
            OtpCmd::List => {
                let entries = cmd::otp::list_all(cfg)?;
                for e in &entries {
                    println!("  {:40} {} ({}s)", e.path, e.state.code, e.state.remaining);
                }
                ui.dim(&format!("{} OTP entries", entries.len()));
            }
            OtpCmd::Import { path, uri } => {
                cmd::otp::import_uri(cfg, &path, &uri)?;
                ui.ok(&format!("OTP imported into '{}'", path));
            }
            OtpCmd::ImportQr { payload, prefix } => {
                let result = cmd::otp::import_migration_qr(cfg, &payload, &prefix)?;
                ui.ok(&format!("Imported {} OTP entries", result.imported));
                if result.failed > 0 { ui.warn(&format!("{} failed", result.failed)); }
            }
        }

        // ── Audit ────────────────────────────────────────────────────────────
        Commands::Audit { fix, hibp } => {
            ui.banner("Audit");
            let report = cmd::audit::run(cfg, fix)?;
            ui.dim(&format!("Scanned {} entries", report.scanned));
            if report.issues.is_empty() {
                ui.ok("No issues found");
            } else {
                for issue in &report.issues {
                    let tag = format!("{:?}", issue.kind).to_uppercase();
                    ui.warn(&format!("[{}] {} — {}", tag, issue.path, issue.detail));
                }
            }
            if hibp {
                ui.step("Checking HIBP...");
                let issues = cmd::audit::hibp_all(cfg, 1000)?;
                for issue in &issues {
                    ui.warn(&format!("[BREACHED] {} — {}", issue.path, issue.detail));
                }
                if issues.is_empty() { ui.ok("No breached passwords found"); }
            }
        }

        // ── Hibp ─────────────────────────────────────────────────────────────
        Commands::Hibp { path } => {
            let path = match path {
                Some(p) => p,
                None    => anyhow::bail!("Specify a path or use: rtpv audit --hibp"),
            };
            let store = rtpv_core::store::Store::new(cfg)?;
            let entry = store.read(&path)?;
            let count = rtpv_core::audit::hibp_check(&entry.password)?;
            if count > 0 {
                ui.warn(&format!("BREACHED — found {} times in HIBP database", count));
            } else {
                ui.ok("Not found in HIBP — password looks safe");
            }
        }

        // ── Strength ─────────────────────────────────────────────────────────
        Commands::Strength { path } => {
            let store = rtpv_core::store::Store::new(cfg)?;
            let entry = store.read(&path)?;
            let report = rtpv_core::gen::strength(&entry.password);
            println!("  Score:      {}/4", report.score);
            println!("  Crack time: {}", report.crack_time);
            for s in &report.suggestions { ui.dim(s); }
        }

        // ── Entropy ──────────────────────────────────────────────────────────
        Commands::Entropy { path } => {
            let store = rtpv_core::store::Store::new(cfg)?;
            let entry = store.read(&path)?;
            let bits = rtpv_core::gen::entropy_bits(&entry.password);
            println!("  Shannon entropy: {:.1} bits", bits);
        }

        // ── Fill ─────────────────────────────────────────────────────────────
        Commands::Fill { path, delay } => {
            cmd::fill::run(cfg, cmd::fill::FillArgs {
                path, delay_ms: delay, type_password: true, press_enter: true,
            })?;
        }

        // ── Template ─────────────────────────────────────────────────────────
        Commands::Template { kind, path, ask } => {
            let kind = resolve_template_kind(kind.as_deref())?;
            let path = path.unwrap_or_else(|| prompt_text("Entry path").unwrap_or_default());

            // Collect fields interactively
            let mut fields = std::collections::HashMap::new();
            for (key, label, is_secret) in kind.fields() {
                let val = if *is_secret { prompt_password(label)? } else { prompt_text(label)? };
                if !val.is_empty() { fields.insert(key.to_string(), val); }
            }

            let password = if ask {
                Some(prompt_password("Password")?)
            } else { None };

            let result = cmd::template::run(cfg, cmd::template::TemplateArgs {
                path:     path.clone(),
                kind,
                fields,
                password,
                gen_len:  cfg.gen_length,
            })?;

            ui.ok(&format!("Created: {}", path));
            if rtpv_core::clipboard::available() {
                rtpv_core::clipboard::copy_with_clear(&result.password, cfg.clip_timeout)?;
                ui.dim(&format!("Password copied (clears in {}s)", cfg.clip_timeout));
            }
        }

        // ── Note ─────────────────────────────────────────────────────────────
        Commands::Note { sub } => match sub {
            NoteCmd::Add { path } => {
                let content = prompt_multiline("Note content (Ctrl-D to finish)")?;
                cmd::note::add(cfg, &path, &content)?;
                ui.ok(&format!("Note saved: {}", path));
            }
            NoteCmd::Show { path } => {
                let content = cmd::note::show(cfg, &path)?;
                println!("{}", content);
            }
            NoteCmd::Edit { path } => { cmd::note::edit(cfg, &path)?; ui.ok("Saved"); }
            NoteCmd::Copy { path } => {
                cmd::note::copy(cfg, &path)?;
                ui.ok(&format!("Note copied (clears in {}s)", cfg.clip_timeout));
            }
            NoteCmd::List => {
                let entries = cmd::note::list_note_entries(cfg, false)?;
                for e in &entries { println!("{}", e); }
            }
        }

        // ── Env ──────────────────────────────────────────────────────────────
        Commands::Env    { path } => {
            let out = cmd::env::run(cfg, &path, cmd::env::EnvFormat::Shell)?;
            println!("{}", out.to_string());
        }
        Commands::Dotenv { path } => {
            let out = cmd::env::run(cfg, &path, cmd::env::EnvFormat::DotEnv)?;
            println!("{}", out.to_string());
        }
        Commands::Run    { path, command } => {
            let status = cmd::env::run_with_env(cfg, &path, &command)?;
            std::process::exit(status.code().unwrap_or(1));
        }

        // ── SSH ───────────────────────────────────────────────────────────────
        Commands::Ssh { sub } => match sub {
            SshCmd::Add { name, private_key, public_key } => {
                cmd::ssh::add(cfg, &name, &private_key, public_key.as_deref())?;
                ui.ok(&format!("SSH key stored: ssh/{}", name));
            }
            SshCmd::Set { name } => {
                let r = cmd::ssh::restore(cfg, &name, None)?;
                ui.ok(&format!("Key restored to {}", r.private_path.display()));
                if let Some(p) = r.public_path { ui.dim(&format!("Public: {}", p.display())); }
            }
            SshCmd::CopyId { name } => {
                cmd::ssh::copy_pubkey(cfg, &name)?;
                ui.ok("Public key copied to clipboard");
            }
            SshCmd::List => {
                for k in cmd::ssh::list(cfg)? { println!("{}", k); }
            }
        }

        // ── Import ────────────────────────────────────────────────────────────
        Commands::Import { sub } => {
            let result = match sub {
                ImportCmd::Bitwarden { path } => cmd::import::import_bitwarden(cfg, &path)?,
                ImportCmd::Firefox   { path } => cmd::import::import_firefox(cfg, &path)?,
                ImportCmd::Chrome    { path } => cmd::import::import_chrome(cfg, &path)?,
                ImportCmd::Keepass   { path } => cmd::import::import_keepass(cfg, &path)?,
            };
            ui.ok(&format!("Imported: {} entries", result.imported));
            if result.skipped > 0  { ui.dim(&format!("Skipped:  {}", result.skipped)); }
            for e in &result.errors { ui.warn(e); }
        }

        Commands::ExportBitwarden { output } => {
            let json = cmd::export::export_bitwarden(cfg)?;
            std::fs::write(&output, &json)?;
            ui.ok(&format!("Exported to {}", output));
        }

        // ── Maintenance ───────────────────────────────────────────────────────
        Commands::Sync => {
            ui.step("Pulling...");
            cmd::maintenance::sync(cfg)?;
            ui.ok("Synced");
        }
        Commands::Log { count } => {
            let git = rtpv_core::git::StoreGit::open(&cfg.store_dir)?;
            for c in git.log(count)? {
                println!("  {} {} ({})", c.hash, c.message, c.author);
            }
        }
        Commands::Diff { path } => {
            // Simple diff: show current entry content
            let store = rtpv_core::store::Store::new(cfg)?;
            let entry = store.read(&path)?;
            println!("{}", entry.to_plaintext());
        }
        Commands::Doctor => {
            ui.banner("Doctor");
            for check in cmd::maintenance::doctor(cfg) {
                if check.ok { ui.ok(&format!("{:30} {}", check.name, check.detail)); }
                else        { ui.warn(&format!("{:30} {}", check.name, check.detail)); }
            }
        }
        Commands::Stats => {
            let s = cmd::maintenance::stats(cfg)?;
            ui.banner("Stats");
            println!("  Store:       {}", s.store_path);
            println!("  Backend:     {}", s.backend);
            println!("  Entries:     {}", s.total);
            println!("  Directories: {}", s.dirs);
            println!("  With OTP:    {}", s.with_otp);
            println!("  With URL:    {}", s.with_url);
        }
        Commands::Gc => {
            let n = cmd::maintenance::gc(cfg)?;
            ui.ok(&format!("Removed {} empty directories", n));
        }
        Commands::Lint => {
            let issues = cmd::maintenance::lint(cfg)?;
            if issues.is_empty() { ui.ok("No issues found"); }
            for i in &issues { ui.warn(&format!("{}: {}", i.path, i.detail)); }
        }
        Commands::GenConf => {
            cfg.write_sample()?;
            ui.ok(&format!("Config written to {}/rtpv.toml", cfg.config_dir.display()));
        }
        Commands::MigrateToAge => {
            ui.step("Migrating store to age encryption...");
            let gpg_cfg = { let mut c = cfg.clone(); c.crypto_backend = CryptoBackend::Gpg; c };
            let gpg_backend = rtpv_core::crypto::build_backend(&gpg_cfg)?;
            let age_backend = rtpv_core::crypto::build_backend(cfg)?;
            let (ok, fail) = rtpv_core::crypto::migrate_store_to_age(&cfg.store_dir, gpg_backend.as_ref(), age_backend.as_ref())?;
            ui.ok(&format!("Migrated {} entries", ok));
            if fail > 0 { ui.warn(&format!("{} entries skipped (decrypt failed)", fail)); }
        }
        Commands::Lock => {
            rtpv_core::clipboard::clear()?;
            ui.ok("Clipboard cleared");
        }

        Commands::Recent { count } => {
            for e in cmd::recent::run(cfg, count)? {
                println!("  {:40} {}",
                    e.path,
                    e.modified.format("%Y-%m-%d %H:%M"));
            }
        }

        Commands::Backup { output } => {
            ui.step("Creating backup...");
            let r = cmd::backup::run(cfg, output.as_deref())?;
            ui.ok(&format!("Backup saved: {}", r.path.display()));
            ui.dim(&format!("{} entries, {} bytes uncompressed",
                r.entries, r.size_bytes));
        }

        Commands::Restore { path, dir } => {
            ui.step("Decrypting and extracting backup...");
            let n = cmd::backup::restore(cfg, &path, dir.as_deref())?;
            ui.ok(&format!("Restored {} files", n));
        }

        // ── GPG ──────────────────────────────────────────────────────────────
        Commands::Gpg { sub } => match sub {
            GpgCmd::Add { key_id, name } => {
                let r = cmd::gpg::add(cfg, &key_id, name.as_deref())?;
                ui.ok(&format!("Stored GPG key at '{}'", r.path));
                ui.dim(&format!("Fingerprint: {}", r.fingerprint));
            }
            GpgCmd::Set { name } => {
                let fp = cmd::gpg::restore(cfg, &name)?;
                ui.ok(&format!("Imported key into keyring: {}", fp));
            }
            GpgCmd::Copy { name } => {
                cmd::gpg::copy_pubkey(cfg, &name)?;
                ui.ok("Public key copied to clipboard");
            }
            GpgCmd::Fingerprint { name } => {
                let fp = cmd::gpg::fingerprint(cfg, &name)?;
                println!("{}", fp);
            }
            GpgCmd::List => {
                for k in cmd::gpg::list(cfg)? { println!("{}", k); }
            }
        }

        // ── Card ─────────────────────────────────────────────────────────────
        Commands::Card { sub } => match sub {
            CardCmd::List => {
                let cards = cmd::card::list(cfg)?;
                if cards.is_empty() {
                    ui.info("No credit card entries found");
                    ui.dim("Add one with: rtpv template --kind credit-card");
                } else {
                    for c in &cards {
                        let holder = c.holder.as_deref().unwrap_or("—");
                        let expiry = c.expiry.as_deref().unwrap_or("—");
                        println!("  {:35} •••• {}  {}  {}",
                            c.path, c.last4, expiry, holder);
                    }
                }
            }
            CardCmd::Copy { path } => {
                cmd::card::copy_number(cfg, &path)?;
                ui.ok(&format!("Card number copied (clears in {}s)", cfg.clip_timeout));
            }
            CardCmd::Cvv { path } => {
                cmd::card::copy_cvv(cfg, &path)?;
                ui.ok(&format!("CVV copied (clears in {}s)", cfg.clip_timeout));
            }
            CardCmd::Show { path } => {
                let d = cmd::card::get_details(cfg, &path)?;
                println!("  Number: {}", d.number);
                if let Some(e) = d.expiry { println!("  Expiry: {}", e); }
                if let Some(h) = d.holder { println!("  Holder: {}", h); }
                if let Some(c) = d.cvv    { println!("  CVV:    {}", c); }
                if let Some(p) = d.pin    { println!("  PIN:    {}", p); }
            }
        }

        // ── Share ────────────────────────────────────────────────────────────
        Commands::Share { sub } => match sub {
            ShareCmd::Send { path, recipient, out } => {
                let r = cmd::share::run(cfg, &path, &recipient, out.as_deref())?;
                ui.ok(&format!("Shared '{}' → {}", r.entry_path, r.output_path.display()));
                ui.dim("Recipient decrypts with: rage -d <file>");
            }
            ShareCmd::Recipients => {
                let keys = cmd::share::list_recipients(cfg)?;
                if keys.is_empty() { ui.info("No recipients configured"); }
                else { for k in &keys { println!("  {}", k); } }
            }
            ShareCmd::AddRecipient { key } => {
                cmd::share::add_recipient(cfg, &key)?;
                ui.ok("Recipient added");
                ui.dim("Run 'rtpv share reencrypt' to re-encrypt the store");
            }
            ShareCmd::RemoveRecipient { key } => {
                let removed = cmd::share::remove_recipient(cfg, &key)?;
                if removed { ui.ok("Recipient removed"); }
                else       { ui.warn("Recipient not found"); }
            }
            ShareCmd::Reencrypt => {
                ui.step("Re-encrypting store...");
                let (ok, fail) = cmd::share::reencrypt_store(cfg)?;
                ui.ok(&format!("Re-encrypted {} entries", ok));
                if fail > 0 { ui.warn(&format!("{} failed", fail)); }
            }
        }

        Commands::Man => {
            manpage::print_man()?;
        }

        Commands::Completions { shell, setup } => {
            let shell: rtpv_core::cmd::completions::Shell = shell.parse()?;
            if setup {
                completions::print_setup(&shell);
            } else {
                completions::generate_completions(&shell);
            }
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Input helpers
// ─────────────────────────────────────────────────────────────────────────────
fn prompt_password(label: &str) -> Result<String> {
    rpassword::prompt_password(format!("  {} : ", label))
        .map_err(|e| anyhow::anyhow!("Password prompt failed: {}", e))
}

fn prompt_text(label: &str) -> Result<String> {
    eprint!("  {} : ", label);
    let mut s = String::new();
    std::io::stdin().read_line(&mut s)?;
    Ok(s.trim().to_string())
}

fn prompt_multiline(label: &str) -> Result<String> {
    eprintln!("  {} :", label);
    use std::io::BufRead;
    let stdin = std::io::stdin();
    let lines: Vec<String> = stdin.lock().lines().collect::<std::io::Result<_>>()?;
    Ok(lines.join("\n"))
}

fn resolve_template_kind(name: Option<&str>) -> Result<cmd::template::TemplateKind> {
    use cmd::template::TemplateKind;
    if let Some(n) = name {
        return match n {
            "web-login" | "web"      => Ok(TemplateKind::WebLogin),
            "server"    | "ssh"      => Ok(TemplateKind::Server),
            "database"  | "db"       => Ok(TemplateKind::Database),
            "api-key"   | "api"      => Ok(TemplateKind::ApiKey),
            "email"     | "email-account" => Ok(TemplateKind::EmailAccount),
            "credit-card" | "card"   => Ok(TemplateKind::CreditCard),
            "wifi"                   => Ok(TemplateKind::Wifi),
            "note"                   => Ok(TemplateKind::Note),
            other => anyhow::bail!("Unknown template '{}'. Options: web-login, server, database, api-key, email-account, credit-card, wifi, note", other),
        };
    }
    // Interactive picker
    eprintln!("  Choose a template:");
    for (i, k) in TemplateKind::all().iter().enumerate() {
        eprintln!("  {}. {:15} {}", i + 1, k.name(), k.description());
    }
    eprint!("  Selection [1-{}]: ", TemplateKind::all().len());
    let mut s = String::new();
    std::io::stdin().read_line(&mut s)?;
    let idx: usize = s.trim().parse().unwrap_or(0);
    TemplateKind::all().get(idx.wrapping_sub(1))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Invalid selection"))
}
