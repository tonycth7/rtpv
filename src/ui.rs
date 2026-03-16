// ui.rs — themed ANSI output helpers (CLI side)

use crate::config::{Colors, Theme};
use std::io::IsTerminal;

pub struct Printer {
    colors: Option<Colors>,
}

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

impl Printer {
    pub fn new(theme: &Theme) -> Self {
        let use_color = std::io::stdout().is_terminal();
        Self {
            colors: if use_color {
                Some(theme.colors())
            } else {
                None
            },
        }
    }

    fn c<'a>(&self, code: &'a str) -> &'a str {
        if self.colors.is_some() {
            code
        } else {
            ""
        }
    }

    pub fn ok(&self, msg: &str) {
        println!(
            "{}{}  ✔  {}{}",
            self.c(self.colors.as_ref().map(|c| c.green).unwrap_or("")),
            self.c(BOLD),
            msg,
            self.c(RESET)
        );
    }

    pub fn err(&self, msg: &str) {
        eprintln!(
            "{}{}  ✖  {}{}",
            self.c(self.colors.as_ref().map(|c| c.red).unwrap_or("")),
            self.c(BOLD),
            msg,
            self.c(RESET)
        );
    }

    pub fn warn(&self, msg: &str) {
        eprintln!(
            "{}  ⚠  {}{}",
            self.c(self.colors.as_ref().map(|c| c.yellow).unwrap_or("")),
            msg,
            self.c(RESET)
        );
    }

    pub fn info(&self, msg: &str) {
        println!(
            "{}  ℹ  {}{}",
            self.c(self.colors.as_ref().map(|c| c.cyan).unwrap_or("")),
            msg,
            self.c(RESET)
        );
    }

    pub fn step(&self, msg: &str) {
        println!(
            "{}  →  {}{}",
            self.c(self.colors.as_ref().map(|c| c.blue).unwrap_or("")),
            msg,
            self.c(RESET)
        );
    }

    pub fn dim(&self, msg: &str) {
        println!("{}     {}{}", self.c(DIM), msg, self.c(RESET));
    }

    pub fn bold(&self, msg: &str) {
        println!("{}  {}{}", self.c(BOLD), msg, self.c(RESET));
    }

    pub fn field(&self, label: &str, value: &str) {
        let cyan = self.colors.as_ref().map(|c| c.cyan).unwrap_or("");
        println!(
            "  {}{:<12}{}  {}",
            self.c(cyan),
            label,
            self.c(RESET),
            value
        );
    }

    pub fn secret_field(&self, label: &str) {
        let dim = self.c(DIM);
        let rst = self.c(RESET);
        println!(
            "  {}{:<12}{}  {}{}{}",
            dim, label, rst, dim, "●●●●●●●●", rst
        );
    }

    pub fn banner(&self, title: &str) {
        let w = 56usize;
        let pad = w.saturating_sub(title.len()) / 2;
        let cyan = self.colors.as_ref().map(|c| c.cyan).unwrap_or("");
        println!();
        println!(
            "{}{}  ╔{}╗{}",
            self.c(BOLD),
            self.c(cyan),
            "═".repeat(w),
            self.c(RESET)
        );
        println!(
            "{}{}  ║{:pad$}{}{:>pad2$}║{}",
            self.c(BOLD),
            self.c(cyan),
            "",
            title,
            "",
            self.c(RESET),
            pad = pad,
            pad2 = w.saturating_sub(pad + title.len()),
        );
        println!(
            "{}{}  ╚{}╝{}",
            self.c(BOLD),
            self.c(cyan),
            "═".repeat(w),
            self.c(RESET)
        );
        println!();
    }

    pub fn section(&self, title: &str) {
        let cyan = self.colors.as_ref().map(|c| c.cyan).unwrap_or("");
        println!(
            "\n  {}{}{}{}",
            self.c(BOLD),
            self.c(cyan),
            title,
            self.c(RESET)
        );
        println!(
            "  {}{}{}",
            self.c(DIM),
            "─".repeat(title.len() + 2),
            self.c(RESET)
        );
    }
}

// ── Password prompt (hidden input) ───────────────────────────────────────────

pub fn prompt_password(label: &str) -> anyhow::Result<String> {
    rpassword::prompt_password(format!("  {} : ", label))
        .map_err(|e| anyhow::anyhow!("Password prompt: {}", e))
}

pub fn prompt_password_confirm(label: &str) -> anyhow::Result<String> {
    loop {
        let pw = prompt_password(label)?;
        if pw.is_empty() {
            anyhow::bail!("Password cannot be empty");
        }
        let pw2 = prompt_password(&format!("Confirm {}", label))?;
        if pw == pw2 {
            return Ok(pw);
        }
        eprintln!("  Passwords do not match, try again");
    }
}

pub fn prompt_text(label: &str) -> anyhow::Result<String> {
    eprint!("  {} : ", label);
    let mut s = String::new();
    std::io::stdin().read_line(&mut s)?;
    Ok(s.trim().to_string())
}

pub fn prompt_text_opt(label: &str) -> anyhow::Result<Option<String>> {
    let s = prompt_text(label)?;
    Ok(if s.is_empty() { None } else { Some(s) })
}

pub fn confirm(prompt: &str) -> anyhow::Result<bool> {
    eprint!("  {} [y/N] ", prompt);
    let mut s = String::new();
    std::io::stdin().read_line(&mut s)?;
    Ok(matches!(s.trim().to_lowercase().as_str(), "y" | "yes"))
}

pub fn pause() {
    eprint!("\n  ↵ Enter to continue...");
    let mut s = String::new();
    let _ = std::io::stdin().read_line(&mut s);
}
