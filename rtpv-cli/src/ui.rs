//! CLI output formatting — themed ANSI colors, banners, spinners.

use rtpv_core::config::{Config, Theme};

pub struct Ui {
    palette: Palette,
    use_color: bool,
}

struct Palette {
    red: &'static str, green: &'static str, yellow: &'static str,
    blue: &'static str, cyan: &'static str, magenta: &'static str,
    bold: &'static str, dim: &'static str, reset: &'static str,
}

impl Ui {
    pub fn new(cfg: &Config) -> Self {
        let use_color = atty::is(atty::Stream::Stdout);
        let p = match cfg.theme {
            Theme::Catppuccin => Palette {
                red:     "\x1b[38;5;203m", green:   "\x1b[38;5;151m",
                yellow:  "\x1b[38;5;222m", blue:    "\x1b[38;5;110m",
                cyan:    "\x1b[38;5;117m", magenta: "\x1b[38;5;183m",
                bold:    "\x1b[1m",        dim:     "\x1b[2m",
                reset:   "\x1b[0m",
            },
            Theme::Nord => Palette {
                red:     "\x1b[38;5;131m", green:   "\x1b[38;5;108m",
                yellow:  "\x1b[38;5;179m", blue:    "\x1b[38;5;67m",
                cyan:    "\x1b[38;5;110m", magenta: "\x1b[38;5;139m",
                bold: "\x1b[1m", dim: "\x1b[2m", reset: "\x1b[0m",
            },
            Theme::Gruvbox => Palette {
                red:     "\x1b[38;5;167m", green:   "\x1b[38;5;142m",
                yellow:  "\x1b[38;5;214m", blue:    "\x1b[38;5;109m",
                cyan:    "\x1b[38;5;108m", magenta: "\x1b[38;5;175m",
                bold: "\x1b[1m", dim: "\x1b[2m", reset: "\x1b[0m",
            },
            Theme::Dracula => Palette {
                red:     "\x1b[38;5;203m", green:   "\x1b[38;5;84m",
                yellow:  "\x1b[38;5;228m", blue:    "\x1b[38;5;61m",
                cyan:    "\x1b[38;5;117m", magenta: "\x1b[38;5;212m",
                bold: "\x1b[1m", dim: "\x1b[2m", reset: "\x1b[0m",
            },
            Theme::Solarized => Palette {
                red:     "\x1b[38;5;160m", green:   "\x1b[38;5;64m",
                yellow:  "\x1b[38;5;136m", blue:    "\x1b[38;5;33m",
                cyan:    "\x1b[38;5;37m",  magenta: "\x1b[38;5;125m",
                bold: "\x1b[1m", dim: "\x1b[2m", reset: "\x1b[0m",
            },
        };
        Self { palette: p, use_color }
    }

    fn c(&self, code: &'static str) -> &'static str {
        if self.use_color { code } else { "" }
    }

    pub fn ok(&self, msg: &str) {
        println!("{}{}  ✔  {}{}",
            self.c(self.palette.green), self.c(self.palette.bold), msg, self.c(self.palette.reset));
    }
    pub fn error(&self, msg: &str) {
        eprintln!("{}{}  ✖  {}{}",
            self.c(self.palette.red), self.c(self.palette.bold), msg, self.c(self.palette.reset));
    }
    pub fn warn(&self, msg: &str) {
        eprintln!("{}  ⚠  {}{}",
            self.c(self.palette.yellow), msg, self.c(self.palette.reset));
    }
    pub fn info(&self, msg: &str) {
        println!("{}  ℹ  {}{}",
            self.c(self.palette.cyan), msg, self.c(self.palette.reset));
    }
    pub fn step(&self, msg: &str) {
        println!("{}  →  {}{}",
            self.c(self.palette.blue), msg, self.c(self.palette.reset));
    }
    pub fn dim(&self, msg: &str) {
        println!("{}     {}{}",
            self.c(self.palette.dim), msg, self.c(self.palette.reset));
    }
    pub fn banner(&self, title: &str) {
        let w = 54usize;
        let pad = (w.saturating_sub(title.len())) / 2;
        println!();
        println!("{}{}  ╔{}╗{}",
            self.c(self.palette.bold), self.c(self.palette.cyan),
            "═".repeat(w), self.c(self.palette.reset));
        println!("{}{}  ║{:pad$}{}{:pad$}║{}",
            self.c(self.palette.bold), self.c(self.palette.cyan),
            "", title, "",
            self.c(self.palette.reset),
            pad = pad);
        println!("{}{}  ╚{}╝{}",
            self.c(self.palette.bold), self.c(self.palette.cyan),
            "═".repeat(w), self.c(self.palette.reset));
        println!();
    }
}
