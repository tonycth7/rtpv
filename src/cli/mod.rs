// cli/mod.rs — clap commands

pub mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name    = "rtpv",
    about   = "rtpv — Rust Password Vault",
    version = env!("CARGO_PKG_VERSION"),
    arg_required_else_help = false,
)]
pub struct Cli {
    /// Vault directory (default: ~/.password-vault)
    #[arg(long, global = true, env = "RTPV_VAULT")]
    pub vault: Option<String>,

    /// Theme: catppuccin | nord | gruvbox | dracula | solarized
    #[arg(long, global = true, env = "RTPV_THEME")]
    pub theme: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new vault (generates age identity)
    Init {
        #[arg(long)]
        force: bool,
    },

    /// Add a new entry
    Add {
        /// Entry path (e.g. github or work/slack)
        path: String,
        /// Entry type: login | note | ssh | gpg | card | env | wifi | database | license
        #[arg(short = 't', long, default_value = "login")]
        kind: String,
        #[arg(short, long)]
        email: Option<String>,
        #[arg(short, long)]
        username: Option<String>,
        #[arg(short, long)]
        url: Option<String>,
        #[arg(short, long)]
        notes: Option<String>,
        /// Prompt for your own password instead of generating
        #[arg(short = 'p', long)]
        password: bool,
        /// Generate a passphrase (N words)
        #[arg(long)]
        words: Option<usize>,
        /// Generate a numeric PIN
        #[arg(long)]
        pin: Option<usize>,
        /// Password length
        #[arg(short = 'L', long)]
        length: Option<usize>,
        /// Don't print the generated password
        #[arg(short = 'n', long)]
        no_print: bool,
    },

    /// Show an entry
    Show {
        /// Entry path
        path: String,
        /// Reveal password in plaintext
        #[arg(long)]
        reveal: bool,
    },

    /// Copy a field to clipboard
    #[command(alias = "cp")]
    Copy {
        path: String,
        #[arg(short = 'u', long)]
        username: bool,
        #[arg(short = 'e', long)]
        email: bool,
        #[arg(short = 't', long)]
        token: bool,
        #[arg(short = 'o', long)]
        otp: bool,
        #[arg(short = 'n', long)]
        notes: bool,
        #[arg(long)]
        field: Option<String>,
    },

    /// Delete an entry
    #[command(alias = "rm")]
    Remove {
        path: String,
        #[arg(short, long)]
        force: bool,
    },

    /// Edit an entry in $EDITOR
    Edit { path: String },

    /// Rename an entry
    Rename { src: String, dst: String },

    /// Clone an entry
    Clone { src: String, dst: String },

    /// Set a field on an entry
    Set {
        path: String,
        field: String,
        value: Option<String>,
    },

    /// Rotate (regenerate) password for an entry
    Rotate {
        path: String,
        #[arg(short = 'L', long)]
        length: Option<usize>,
    },

    /// List all entries
    #[command(alias = "ls")]
    List {
        /// Filter by prefix
        filter: Option<String>,
    },

    /// Generate a password without storing it
    Gen {
        #[arg(long)]
        words: Option<usize>,
        #[arg(long)]
        pin: Option<usize>,
        #[arg(long)]
        pronounceable: bool,
        length: Option<usize>,
        #[arg(long)]
        no_copy: bool,
    },

    /// OTP subcommands
    Otp {
        #[command(subcommand)]
        sub: OtpCmd,
    },

    /// Security audit
    Audit {
        #[arg(long)]
        hibp: bool,
        #[arg(long)]
        fix: bool,
    },

    /// Check one entry against HIBP
    Hibp { path: String },

    /// Password strength
    Strength { path: String },

    /// Git sync (SSH only)
    Sync {
        #[arg(long, default_value = "origin")]
        remote: String,
    },

    /// Git log
    Log {
        #[arg(short, long, default_value = "20")]
        count: usize,
    },

    /// Import from another password manager
    Import {
        #[command(subcommand)]
        sub: ImportCmd,
    },

    /// Import from existing pass store (migrate from ~/.password-store)
    Migrate {
        /// Path to pass store (default: ~/.password-store)
        store_path: Option<String>,
        /// Use GPG subprocess to decrypt existing entries
        #[arg(long, default_value = "true")]
        gpg: bool,
    },

    /// Export to Bitwarden JSON
    Export {
        #[arg(short, long, default_value = "rtpv-export.json")]
        output: String,
    },

    /// Autofill: type username + TAB + password
    Fill {
        path: String,
        #[arg(long, default_value = "500")]
        delay: u64,
        /// Press Enter after password
        #[arg(short, long)]
        enter: bool,
    },

    /// Run a command with credentials as env vars
    Run {
        path: String,
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },

    /// SSH key management
    Ssh {
        #[command(subcommand)]
        sub: SshCmd,
    },

    /// Recipients management (multi-user sharing)
    Recipients {
        #[command(subcommand)]
        sub: RecipientsCmd,
    },

    /// Lock: clear clipboard
    Lock,

    /// Maintenance
    Doctor,

    /// Recent entries
    Recent {
        #[arg(short = 'n', long, default_value = "10")]
        count: usize,
    },

    /// Generate sample config file
    GenConf,
}

#[derive(Subcommand)]
pub enum OtpCmd {
    /// Copy current OTP code
    Copy { path: String },
    /// Show OTP code + countdown
    Show { path: String },
    /// List entries with OTP
    List,
    /// Add OTP URI to an entry
    Add { path: String, uri: String },
}

#[derive(Subcommand)]
pub enum ImportCmd {
    Bitwarden { path: String },
    Firefox { path: String },
    Chrome { path: String },
    Csv { path: String },
}

#[derive(Subcommand)]
pub enum SshCmd {
    /// Store a keypair
    Add {
        name: String,
        private_key: String,
        public_key: Option<String>,
    },
    /// Restore keypair to ~/.ssh
    Restore { name: String },
    /// Copy public key to clipboard
    CopyPub { name: String },
    /// List stored keys
    List,
    /// Generate new key and store it
    Generate {
        name: String,
        #[arg(long, default_value = "")]
        comment: String,
    },
}

#[derive(Subcommand)]
pub enum RecipientsCmd {
    /// List configured recipients
    List,
    /// Add a recipient public key
    Add { pubkey: String },
    /// Remove a recipient
    Remove { pubkey: String },
    /// Re-encrypt entire vault to current recipients
    Reencrypt,
}
