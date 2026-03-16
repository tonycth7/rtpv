// clipboard.rs — cross-platform clipboard with background auto-clear

use anyhow::{Context, Result};

pub fn copy(text: &str) -> Result<()> {
    let mut ctx = arboard::Clipboard::new()
        .context("Cannot open clipboard (is a display server running?)")?;
    ctx.set_text(text).context("Failed to write to clipboard")
}

pub fn copy_with_clear(text: &str, timeout_secs: u64) -> Result<()> {
    copy(text)?;
    let owned = text.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(timeout_secs));
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

pub fn clear() -> Result<()> {
    let mut ctx = arboard::Clipboard::new().context("Cannot open clipboard")?;
    ctx.set_text("").context("Failed to clear clipboard")
}

pub fn available() -> bool {
    arboard::Clipboard::new().is_ok()
}
