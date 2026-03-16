//! Clipboard — cross-platform copy with auto-clear timer.
//!
//! Uses the `arboard` crate which covers:
//!   Linux/Wayland, Linux/X11, macOS, Windows
//!
//! Auto-clear spawns a background thread that sleeps for `timeout` seconds
//! then clears the clipboard — but only if it still holds our content.

use anyhow::{Context, Result};

// ─────────────────────────────────────────────────────────────────────────────
// Copy to clipboard
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(feature = "clipboard")]
pub fn copy(text: &str) -> Result<()> {
    let mut ctx = arboard::Clipboard::new()
        .context("Cannot open clipboard (is a display server running?)")?;
    ctx.set_text(text)
        .context("Failed to write to clipboard")?;
    Ok(())
}

#[cfg(not(feature = "clipboard"))]
pub fn copy(_text: &str) -> Result<()> {
    anyhow::bail!("rtpv compiled without clipboard support");
}

// ─────────────────────────────────────────────────────────────────────────────
// Copy with auto-clear
// ─────────────────────────────────────────────────────────────────────────────
/// Copy `text` to clipboard, then spawn a background thread to clear it
/// after `timeout_secs`. The thread checks that the clipboard still holds
/// the same text before clearing (respects user's own pastes).
pub fn copy_with_clear(text: &str, timeout_secs: u64) -> Result<()> {
    copy(text)?;
    let owned = text.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(timeout_secs));
        #[cfg(feature = "clipboard")]
        if let Ok(mut ctx) = arboard::Clipboard::new() {
            if let Ok(current) = ctx.get_text() {
                if current == owned {
                    let _ = ctx.set_text("");
                }
            }
        }
    });
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Clear immediately
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(feature = "clipboard")]
pub fn clear() -> Result<()> {
    let mut ctx = arboard::Clipboard::new()
        .context("Cannot open clipboard")?;
    ctx.set_text("")
        .context("Failed to clear clipboard")?;
    Ok(())
}

#[cfg(not(feature = "clipboard"))]
pub fn clear() -> Result<()> { Ok(()) }

// ─────────────────────────────────────────────────────────────────────────────
// Available check
// ─────────────────────────────────────────────────────────────────────────────
pub fn available() -> bool {
    #[cfg(feature = "clipboard")]
    { arboard::Clipboard::new().is_ok() }
    #[cfg(not(feature = "clipboard"))]
    { false }
}
