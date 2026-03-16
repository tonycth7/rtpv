use notify_rust;
// `rtpv fill` — type username + TAB + password into the focused window.
//
// Uses xdotool on X11 or ydotool on Wayland.
// Falls back to clipboard + notification if no typing tool is available.

use anyhow::{bail, Result};
use std::process::Command;
use crate::config::Config;
use crate::store::Store;

pub struct FillArgs {
    pub path:          String,
    pub delay_ms:      u64,    // delay before typing (ms), default 500
    pub type_password: bool,   // false = type only username
    pub press_enter:   bool,   // press Enter after password
}

impl Default for FillArgs {
    fn default() -> Self {
        Self { path: String::new(), delay_ms: 500, type_password: true, press_enter: true }
    }
}

pub enum FillBackend { Xdotool, Ydotool }

pub fn detect_backend() -> Option<FillBackend> {
    if which("xdotool") { return Some(FillBackend::Xdotool); }
    if which("ydotool") { return Some(FillBackend::Ydotool); }
    None
}

fn which(cmd: &str) -> bool {
    Command::new("which").arg(cmd).output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn run(cfg: &Config, args: FillArgs) -> Result<()> {
    let store = Store::new(cfg)?;
    let entry = store.read(&args.path)?;

    let username = entry.username().unwrap_or("").to_string();
    let password = entry.password.clone();

    if username.is_empty() && password.is_empty() {
        bail!("Entry '{}' has no username or password to fill", args.path);
    }

    match detect_backend() {
        Some(FillBackend::Xdotool) => {
            std::thread::sleep(std::time::Duration::from_millis(args.delay_ms));
            if !username.is_empty() {
                xdotool_type(&username)?;
                xdotool_key("Tab")?;
            }
            if args.type_password {
                xdotool_type(&password)?;
                if args.press_enter { xdotool_key("Return")?; }
            }
        }
        Some(FillBackend::Ydotool) => {
            std::thread::sleep(std::time::Duration::from_millis(args.delay_ms));
            if !username.is_empty() {
                ydotool_type(&username)?;
                ydotool_key("tab")?;
            }
            if args.type_password {
                ydotool_type(&password)?;
                if args.press_enter { ydotool_key("enter")?; }
            }
        }
        None => {
            // Fallback: copy username then notify, then copy password
            if !username.is_empty() {
                crate::clipboard::copy(&username)?;
                if cfg.notify {
                    let _ = notify_rust::Notification::new()
                        .summary("rtpv fill")
                        .body("Username copied — paste it, then run rtpv copy for password")
                        .timeout(4000)
                        .show();
                }
            } else {
                crate::clipboard::copy_with_clear(&password, cfg.clip_timeout)?;
                if cfg.notify {
                    let _ = notify_rust::Notification::new()
                        .summary("rtpv fill")
                        .body("Password copied (no typing tool found — install xdotool)")
                        .timeout(4000)
                        .show();
                }
            }
        }
    }
    Ok(())
}

fn xdotool_type(text: &str) -> Result<()> {
    let status = Command::new("xdotool")
        .args(["type", "--clearmodifiers", "--", text])
        .status()?;
    if !status.success() { bail!("xdotool type failed"); }
    Ok(())
}

fn xdotool_key(key: &str) -> Result<()> {
    let status = Command::new("xdotool").args(["key", key]).status()?;
    if !status.success() { bail!("xdotool key {} failed", key); }
    Ok(())
}

fn ydotool_type(text: &str) -> Result<()> {
    let status = Command::new("ydotool").args(["type", "--", text]).status()?;
    if !status.success() { bail!("ydotool type failed"); }
    Ok(())
}

fn ydotool_key(key: &str) -> Result<()> {
    let status = Command::new("ydotool").args(["key", key]).status()?;
    if !status.success() { bail!("ydotool key {} failed", key); }
    Ok(())
}
