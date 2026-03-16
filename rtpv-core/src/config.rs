//! Configuration: XDG paths, rtpv.conf (TOML), env var overrides.
//!
//! Load order:
//!   1. compiled defaults
//!   2. ~/.config/rtpv/rtpv.toml  (TOML preferred)
//!   3. ~/.config/rtpv/rtpv.conf  (legacy key=value, best-effort)
//!   4. environment variables       (highest priority)

use std::path::PathBuf;
use std::str::FromStr;
use anyhow::Result;
use directories::ProjectDirs;
use dirs;
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Crypto backend choice
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CryptoBackend {
    /// Native age encryption via rage crate — no external deps
    #[default]
    Age,
    /// Subprocess GPG — for legacy .password-store compatibility
    Gpg,
}

impl FromStr for CryptoBackend {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "age" => Ok(Self::Age),
            "gpg" => Ok(Self::Gpg),
            other => anyhow::bail!("Unknown crypto backend '{}' (age | gpg)", other),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Theme
// ─────────────────────────────────────────────────────────────────────────────
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

impl FromStr for Theme {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "catppuccin" => Ok(Self::Catppuccin),
            "nord"       => Ok(Self::Nord),
            "gruvbox"    => Ok(Self::Gruvbox),
            "dracula"    => Ok(Self::Dracula),
            "solarized"  => Ok(Self::Solarized),
            other        => anyhow::bail!("Unknown theme '{}'", other),
        }
    }
}

/// ANSI color palette for a given theme
#[derive(Debug, Clone)]
pub struct Palette {
    pub red:     &'static str,
    pub green:   &'static str,
    pub yellow:  &'static str,
    pub blue:    &'static str,
    pub cyan:    &'static str,
    pub magenta: &'static str,
}

impl Theme {
    pub fn palette(&self) -> Palette {
        match self {
            Theme::Catppuccin => Palette {
                red:     "\x1b[38;5;203m",
                green:   "\x1b[38;5;151m",
                yellow:  "\x1b[38;5;222m",
                blue:    "\x1b[38;5;110m",
                cyan:    "\x1b[38;5;117m",
                magenta: "\x1b[38;5;183m",
            },
            Theme::Nord => Palette {
                red:     "\x1b[38;5;131m",
                green:   "\x1b[38;5;108m",
                yellow:  "\x1b[38;5;179m",
                blue:    "\x1b[38;5;67m",
                cyan:    "\x1b[38;5;110m",
                magenta: "\x1b[38;5;139m",
            },
            Theme::Gruvbox => Palette {
                red:     "\x1b[38;5;167m",
                green:   "\x1b[38;5;142m",
                yellow:  "\x1b[38;5;214m",
                blue:    "\x1b[38;5;109m",
                cyan:    "\x1b[38;5;108m",
                magenta: "\x1b[38;5;175m",
            },
            Theme::Dracula => Palette {
                red:     "\x1b[38;5;203m",
                green:   "\x1b[38;5;84m",
                yellow:  "\x1b[38;5;228m",
                blue:    "\x1b[38;5;61m",
                cyan:    "\x1b[38;5;117m",
                magenta: "\x1b[38;5;212m",
            },
            Theme::Solarized => Palette {
                red:     "\x1b[38;5;160m",
                green:   "\x1b[38;5;64m",
                yellow:  "\x1b[38;5;136m",
                blue:    "\x1b[38;5;33m",
                cyan:    "\x1b[38;5;37m",
                magenta: "\x1b[38;5;125m",
            },
        }
    }
}          
 

// ─────────────────────────────────────────────────────────────────────────────
// Main config struct
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // paths
    pub store_dir:       PathBuf,
    pub config_dir:      PathBuf,
    pub cache_dir:       PathBuf,
    pub hooks_dir:       PathBuf,
    pub age_identity:    PathBuf,   // ~/.config/rtpv/identity.age

    // behaviour
    pub crypto_backend:  CryptoBackend,
    pub autosync:        bool,
    pub clip_timeout:    u64,       // seconds
    pub max_age_days:    u64,       // audit threshold
    pub notify:          bool,
    pub debug:           bool,
    pub json_output:     bool,
    pub color:           ColorMode,

    // ui
    pub theme:           Theme,

    // generator defaults
    pub gen_length:      usize,
    pub gen_chars:       String,
  }


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode { #[default] Auto, Always, Never }

impl Default for Config {
    fn default() -> Self {
        let dirs = ProjectDirs::from("", "", "rtpv")
            .expect("Cannot determine config directory — is HOME set?");
        let home = dirs::home_dir()
            .or_else(|| std::env::var("HOME").ok().map(std::path::PathBuf::from))
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));

        Self {
            store_dir:      home.join(".password-store"),
            config_dir:     dirs.config_dir().to_path_buf(),
            cache_dir:      dirs.cache_dir().to_path_buf(),
            hooks_dir:      dirs.config_dir().join("hooks"),
            age_identity:   dirs.config_dir().join("identity.age"),

            crypto_backend: CryptoBackend::Age,
            autosync:       false,
            clip_timeout:   20,
            max_age_days:   180,
            notify:         true,
            debug:          false,
            json_output:    false,
            color:          ColorMode::Auto,

            theme:          Theme::Catppuccin,

            gen_length:     32,
            gen_chars:      "A-Za-z0-9@#%+=_".to_string(),
        }
    }
}

impl Config {
    /// Load config, applying env-var overrides.
    pub fn load() -> Result<Self> {
        let mut cfg = Config::default();
        cfg.apply_toml_file()?;
        cfg.apply_env_overrides();
        Ok(cfg)
    }

    fn apply_toml_file(&mut self) -> Result<()> {
        let toml_path = self.config_dir.join("rtpv.toml");
        if toml_path.exists() {
            let text = std::fs::read_to_string(&toml_path)?;
            let partial: toml::Value = toml::from_str(&text)?;
            // Merge known keys
            if let Some(t) = partial.get("theme").and_then(|v| v.as_str()) {
                if let Ok(theme) = t.parse::<Theme>() { self.theme = theme; }
            }
            if let Some(t) = partial.get("crypto_backend").and_then(|v| v.as_str()) {
                if let Ok(b) = t.parse::<CryptoBackend>() { self.crypto_backend = b; }
            }
            if let Some(v) = partial.get("clip_timeout").and_then(|v| v.as_integer()) {
                self.clip_timeout = v as u64;
            }
            if let Some(v) = partial.get("max_age_days").and_then(|v| v.as_integer()) {
                self.max_age_days = v as u64;
            }
            if let Some(v) = partial.get("autosync").and_then(|v| v.as_bool()) {
                self.autosync = v;
            }
            if let Some(v) = partial.get("gen_length").and_then(|v| v.as_integer()) {
                self.gen_length = v as usize;
            }
            if let Some(v) = partial.get("gen_chars").and_then(|v| v.as_str()) {
                self.gen_chars = v.to_string();
            }
            if let Some(v) = partial.get("store_dir").and_then(|v| v.as_str()) {
                self.store_dir = PathBuf::from(shellexpand::tilde(v).as_ref());
            }
        }
        Ok(())
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(v) = std::env::var("PASSWORD_STORE_DIR") {
            self.store_dir = PathBuf::from(v);
        }
        if let Ok(v) = std::env::var("RTPV_THEME") {
            if let Ok(t) = v.parse::<Theme>() { self.theme = t; }
        }
        if let Ok(v) = std::env::var("RTPV_BACKEND") {
            if let Ok(b) = v.parse::<CryptoBackend>() { self.crypto_backend = b; }
        }
        if let Ok(v) = std::env::var("RTPV_CLIP_TIMEOUT") {
            if let Ok(n) = v.parse::<u64>() { self.clip_timeout = n; }
        }
        if let Ok(v) = std::env::var("RTPV_MAX_AGE") {
            if let Ok(n) = v.parse::<u64>() { self.max_age_days = n; }
        }
        if let Ok(v) = std::env::var("RTPV_AUTOSYNC") {
            self.autosync = matches!(v.to_lowercase().as_str(), "true" | "1" | "yes");
        }
        if let Ok(v) = std::env::var("RTPV_DEBUG") {
            self.debug = matches!(v.as_str(), "1" | "true");
        }
        if let Ok(v) = std::env::var("RTPV_JSON") {
            self.json_output = matches!(v.as_str(), "1" | "true");
        }
        if let Ok(v) = std::env::var("RTPV_GEN_LENGTH") {
            if let Ok(n) = v.parse::<usize>() { self.gen_length = n; }
        }
        if let Ok(v) = std::env::var("RTPV_GEN_CHARS") {
            self.gen_chars = v;
        }
    }

    /// Write a sample TOML config to the config directory.
    pub fn write_sample(&self) -> Result<()> {
        std::fs::create_dir_all(&self.config_dir)?;
        let sample = include_str!("../../config/rtpv.sample.toml");
        let path = self.config_dir.join("rtpv.toml");
        std::fs::write(&path, sample)?;
        Ok(())
    }
}
