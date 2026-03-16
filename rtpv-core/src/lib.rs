//! rtpv-core — shared logic for all rtpv binaries
//!
//! Modules
//! ───────
//!   config    – configuration, XDG paths, env vars
//!   crypto    – age-native and gpg-compat encryption backends
//!   store     – entry I/O, field parsing, listing
//!   gen       – password / passphrase / PIN generation
//!   otp       – TOTP / HOTP
//!   clipboard – cross-platform clipboard with auto-clear
//!   git       – sync, log, diff, watch
//!   audit     – strength, HIBP, age audit
//!   cmd       – command implementations (used by CLI and TUI)

pub mod config;
pub mod crypto;
pub mod store;
pub mod gen;
pub mod otp;
pub mod clipboard;
pub mod git;
pub mod audit;
pub mod cmd;
pub mod hooks;

pub use anyhow::{bail, Context, Result};

/// Version exported for all binaries
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
