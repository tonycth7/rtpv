//! `rtpv card` — smart picker for entries that have a `number:` field (credit cards).
//!
//! Decrypts all entries and filters to those with a `number:` field.
//! Presents them for selection, then copies the chosen field.

use anyhow::Result;
use crate::config::Config;
use crate::store::{Entry, Store};

pub struct CardEntry {
    pub path:   String,
    pub holder: Option<String>,
    pub last4:  String,   // last 4 digits of card number
    pub expiry: Option<String>,
}

impl CardEntry {
    fn from_entry(entry: &Entry) -> Option<Self> {
        let number = entry.get_field("number")?;
        let digits: String = number.chars().filter(|c| c.is_ascii_digit()).collect();
        let last4 = if digits.len() >= 4 {
            digits[digits.len()-4..].to_string()
        } else {
            digits
        };
        Some(CardEntry {
            path:   entry.path.clone(),
            holder: entry.get_field("holder").map(String::from),
            last4,
            expiry: entry.get_field("expiry").map(String::from),
        })
    }
}

/// List all entries that have a `number:` field — decrypt all, filter.
pub fn list(cfg: &Config) -> Result<Vec<CardEntry>> {
    let store = Store::new(cfg)?;
    let mut cards = Vec::new();
    for path in store.list()? {
        if let Ok(entry) = store.read(&path) {
            if let Some(card) = CardEntry::from_entry(&entry) {
                cards.push(card);
            }
        }
    }
    Ok(cards)
}

/// Get full card details for a specific entry.
pub struct CardDetails {
    pub number: String,
    pub expiry: Option<String>,
    pub cvv:    Option<String>,
    pub holder: Option<String>,
    pub pin:    Option<String>,
}

pub fn get_details(cfg: &Config, path: &str) -> Result<CardDetails> {
    let store = Store::new(cfg)?;
    let entry = store.read(path)?;
    Ok(CardDetails {
        number: entry.get_field("number")
            .ok_or_else(|| anyhow::anyhow!("No card number in '{}'", path))?
            .to_string(),
        expiry: entry.get_field("expiry").map(String::from),
        cvv:    entry.get_field("cvv").map(String::from),
        holder: entry.get_field("holder").map(String::from),
        pin:    entry.get_field("pin").map(String::from),
    })
}

/// Copy card number to clipboard.
pub fn copy_number(cfg: &Config, path: &str) -> Result<()> {
    let store = Store::new(cfg)?;
    let entry = store.read(path)?;
    let number = entry.get_field("number")
        .ok_or_else(|| anyhow::anyhow!("No card number in '{}'", path))?;
    // Copy without spaces/dashes
    let clean: String = number.chars().filter(|c| c.is_ascii_digit()).collect();
    crate::clipboard::copy_with_clear(&clean, cfg.clip_timeout)?;
    Ok(())
}

/// Copy CVV to clipboard.
pub fn copy_cvv(cfg: &Config, path: &str) -> Result<()> {
    let store = Store::new(cfg)?;
    let entry = store.read(path)?;
    let cvv = entry.get_field("cvv")
        .ok_or_else(|| anyhow::anyhow!("No CVV in '{}'", path))?;
    crate::clipboard::copy_with_clear(cvv, cfg.clip_timeout)?;
    Ok(())
}
