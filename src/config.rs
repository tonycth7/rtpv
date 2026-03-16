// config.rs — rtpv configuration
// Load order: compiled defaults → ~/.config/rtpv/config.toml → env vars

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Vault directory. Default: ~/.password-vault
    pub vault_dir: PathBuf,
    /// Config directory. Default: ~/.config/rtpv
    pub config_dir: PathBuf,
    /// Seconds before clipboard auto-clears
    pub clip_timeout: u64,
    /// Days before audit flags entry as aged
    pub max_age_days: u64,
    /// Desktop notifications
    pub notify: bool,
    /// Git remote for sync (SSH URL)
    pub git_remote: Option<String>,
    /// Auto git commit+push after mutations
    pub autosync: bool,
    /// UI theme
    pub theme: Theme,
    /// Generated password length
    pub gen_length: usize,
    /// Generated password charset
    pub gen_chars: String,
    /// HIBP check timeout in seconds
    pub hibp_timeout: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Catppuccin,
    Nord,
    Gruvbox,
    Dracula,
    Solarized,
}

impl Theme {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "nord" => Self::Nord,
            "gruvbox" => Self::Gruvbox,
            "dracula" => Self::Dracula,
            "solarized" => Self::Solarized,
            _ => Self::Catppuccin,
        }
    }

    /// ANSI color codes for this theme
    pub fn colors(&self) -> Colors {
        match self {
            Self::Catppuccin => Colors {
                red: "\x1b[38;5;203m",
                green: "\x1b[38;5;151m",
                yellow: "\x1b[38;5;222m",
                blue: "\x1b[38;5;110m",
                cyan: "\x1b[38;5;117m",
                magenta: "\x1b[38;5;183m",
            },
            Self::Nord => Colors {
                red: "\x1b[38;5;131m",
                green: "\x1b[38;5;108m",
                yellow: "\x1b[38;5;179m",
                blue: "\x1b[38;5;67m",
                cyan: "\x1b[38;5;110m",
                magenta: "\x1b[38;5;139m",
            },
            Self::Gruvbox => Colors {
                red: "\x1b[38;5;167m",
                green: "\x1b[38;5;142m",
                yellow: "\x1b[38;5;214m",
                blue: "\x1b[38;5;109m",
                cyan: "\x1b[38;5;108m",
                magenta: "\x1b[38;5;175m",
            },
            Self::Dracula => Colors {
                red: "\x1b[38;5;203m",
                green: "\x1b[38;5;84m",
                yellow: "\x1b[38;5;228m",
                blue: "\x1b[38;5;61m",
                cyan: "\x1b[38;5;117m",
                magenta: "\x1b[38;5;212m",
            },
            Self::Solarized => Colors {
                red: "\x1b[38;5;160m",
                green: "\x1b[38;5;64m",
                yellow: "\x1b[38;5;136m",
                blue: "\x1b[38;5;33m",
                cyan: "\x1b[38;5;37m",
                magenta: "\x1b[38;5;125m",
            },
        }
    }

    /// ratatui Color for primary accent
    pub fn accent(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            Self::Catppuccin => Color::Indexed(110),
            Self::Nord => Color::Indexed(67),
            Self::Gruvbox => Color::Indexed(142),
            Self::Dracula => Color::Indexed(117),
            Self::Solarized => Color::Indexed(33),
        }
    }

    pub fn ok_color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            Self::Catppuccin => Color::Indexed(151),
            Self::Nord => Color::Indexed(108),
            Self::Gruvbox => Color::Indexed(142),
            Self::Dracula => Color::Indexed(84),
            Self::Solarized => Color::Indexed(64),
        }
    }

    pub fn warn_color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            Self::Catppuccin => Color::Indexed(222),
            Self::Nord => Color::Indexed(179),
            Self::Gruvbox => Color::Indexed(214),
            Self::Dracula => Color::Indexed(228),
            Self::Solarized => Color::Indexed(136),
        }
    }

    pub fn err_color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            Self::Catppuccin => Color::Indexed(203),
            Self::Nord => Color::Indexed(131),
            Self::Gruvbox => Color::Indexed(167),
            Self::Dracula => Color::Indexed(203),
            Self::Solarized => Color::Indexed(160),
        }
    }
}

pub struct Colors {
    pub red: &'static str,
    pub green: &'static str,
    pub yellow: &'static str,
    pub blue: &'static str,
    pub cyan: &'static str,
    pub magenta: &'static str,
}

impl Default for Config {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| home.join(".config"))
            .join("rtpv");
        Self {
            vault_dir: home.join(".password-vault"),
            config_dir,
            clip_timeout: 20,
            max_age_days: 180,
            notify: true,
            git_remote: None,
            autosync: false,
            theme: Theme::Catppuccin,
            gen_length: 32,
            gen_chars: "A-Za-z0-9@#%+=_".to_string(),
            hibp_timeout: 10,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let mut cfg = Config::default();

        // Load TOML file if exists
        let toml_path = cfg.config_dir.join("config.toml");
        if toml_path.exists() {
            let text = std::fs::read_to_string(&toml_path)?;
            if let Ok(partial) = toml::from_str::<toml::Value>(&text) {
                cfg.apply_toml(&partial);
            }
        }

        // Env overrides (highest priority)
        cfg.apply_env();
        Ok(cfg)
    }

    fn apply_toml(&mut self, v: &toml::Value) {
        macro_rules! get_str {
            ($key:expr, $field:expr) => {
                if let Some(s) = v.get($key).and_then(|x| x.as_str()) {
                    $field = s.to_string();
                }
            };
        }
        macro_rules! get_bool {
            ($key:expr, $field:expr) => {
                if let Some(b) = v.get($key).and_then(|x| x.as_bool()) {
                    $field = b;
                }
            };
        }
        macro_rules! get_u64 {
            ($key:expr, $field:expr) => {
                if let Some(n) = v.get($key).and_then(|x| x.as_integer()) {
                    $field = n as u64;
                }
            };
        }
        macro_rules! get_usize {
            ($key:expr, $field:expr) => {
                if let Some(n) = v.get($key).and_then(|x| x.as_integer()) {
                    $field = n as usize;
                }
            };
        }

        if let Some(s) = v.get("vault_dir").and_then(|x| x.as_str()) {
            self.vault_dir = PathBuf::from(shellexpand(s));
        }
        if let Some(s) = v.get("theme").and_then(|x| x.as_str()) {
            self.theme = Theme::from_str(s);
        }
        if let Some(s) = v.get("git_remote").and_then(|x| x.as_str()) {
            self.git_remote = Some(s.to_string());
        }
        get_u64!("clip_timeout", self.clip_timeout);
        get_u64!("max_age_days", self.max_age_days);
        get_u64!("hibp_timeout", self.hibp_timeout);
        get_bool!("notify", self.notify);
        get_bool!("autosync", self.autosync);
        get_usize!("gen_length", self.gen_length);
        get_str!("gen_chars", self.gen_chars);
    }

    fn apply_env(&mut self) {
        if let Ok(v) = std::env::var("RTPV_VAULT") {
            self.vault_dir = PathBuf::from(v);
        }
        if let Ok(v) = std::env::var("RTPV_THEME") {
            self.theme = Theme::from_str(&v);
        }
        if let Ok(v) = std::env::var("RTPV_CLIP_TIMEOUT") {
            if let Ok(n) = v.parse() {
                self.clip_timeout = n;
            }
        }
        if let Ok(v) = std::env::var("RTPV_AUTOSYNC") {
            self.autosync = matches!(v.as_str(), "1" | "true" | "yes");
        }
        if let Ok(v) = std::env::var("RTPV_GIT_REMOTE") {
            self.git_remote = Some(v);
        }
        if let Ok(v) = std::env::var("RTPV_NOTIFY") {
            self.notify = matches!(v.as_str(), "1" | "true" | "yes");
        }
    }

    pub fn write_sample(&self) -> Result<()> {
        std::fs::create_dir_all(&self.config_dir)?;
        let path = self.config_dir.join("config.toml");
        let sample = format!(
            r#"# rtpv configuration
# vault_dir = "{}"
clip_timeout = {}
max_age_days = {}
notify       = {}
autosync     = {}
theme        = "catppuccin"   # catppuccin | nord | gruvbox | dracula | solarized
gen_length   = {}
gen_chars    = "{}"
# git_remote = "git@github.com:user/vault.git"
"#,
            self.vault_dir.display(),
            self.clip_timeout,
            self.max_age_days,
            self.notify,
            self.autosync,
            self.gen_length,
            self.gen_chars,
        );
        std::fs::write(path, sample)?;
        Ok(())
    }
}

fn shellexpand(s: &str) -> String {
    if s.starts_with("~/") {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        format!("{}/{}", home.display(), &s[2..])
    } else {
        s.to_string()
    }
}
