//! Command implementations — shared between CLI and TUI binaries.
//!
//! Each sub-module contains the pure logic for a command group.
//! Binaries call these functions and handle output formatting themselves.

pub mod add;
pub mod show;
pub mod copy;
pub mod edit;
pub mod rotate;
pub mod search;
pub mod otp;
pub mod fill;
pub mod note;
pub mod template;
pub mod env;
pub mod audit;
pub mod ssh;
pub mod import;
pub mod export;
pub mod maintenance;
pub mod init;
pub mod completions;
pub mod recent;
pub mod backup;
pub mod gpg;
pub mod card;
pub mod share;
