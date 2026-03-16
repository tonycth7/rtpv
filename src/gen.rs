// gen.rs — password / passphrase / PIN generation

use anyhow::{bail, Result};
use rand::Rng;

pub enum GenMode {
    Random { length: usize, chars: String },
    Words { count: usize, sep: char },
    Pin { length: usize },
    Pronounceable { length: usize },
}

pub fn generate(mode: &GenMode) -> Result<String> {
    match mode {
        GenMode::Random { length, chars } => random(*length, chars),
        GenMode::Words { count, sep } => passphrase(*count, *sep),
        GenMode::Pin { length } => pin(*length),
        GenMode::Pronounceable { length } => pronounceable(*length),
    }
}

pub fn random(length: usize, chars: &str) -> Result<String> {
    if length == 0 {
        bail!("Length must be > 0");
    }
    let pool = expand_charset(chars)?;
    if pool.is_empty() {
        bail!("Empty charset");
    }
    let mut rng = rand::thread_rng();
    Ok((0..length)
        .map(|_| pool[rng.gen_range(0..pool.len())])
        .collect())
}

fn expand_charset(chars: &str) -> Result<Vec<char>> {
    let cv: Vec<char> = chars.chars().collect();
    let mut pool = Vec::new();
    let mut i = 0;
    while i < cv.len() {
        if i + 2 < cv.len() && cv[i + 1] == '-' {
            let start = cv[i] as u32;
            let end = cv[i + 2] as u32;
            if start > end {
                bail!("Invalid range {}-{}", cv[i], cv[i + 2]);
            }
            for c in start..=end {
                if let Some(ch) = char::from_u32(c) {
                    pool.push(ch);
                }
            }
            i += 3;
        } else {
            pool.push(cv[i]);
            i += 1;
        }
    }
    Ok(pool)
}

pub fn passphrase(count: usize, sep: char) -> Result<String> {
    if count == 0 {
        bail!("Word count must be > 0");
    }
    let words = system_wordlist().unwrap_or_else(builtin_words);
    let mut rng = rand::thread_rng();
    let chosen: Vec<&str> = (0..count)
        .map(|_| words[rng.gen_range(0..words.len())].as_str())
        .collect();
    Ok(chosen.join(&sep.to_string()))
}

pub fn pin(length: usize) -> Result<String> {
    if length == 0 {
        bail!("PIN length must be > 0");
    }
    let mut rng = rand::thread_rng();
    Ok((0..length)
        .map(|_| char::from_digit(rng.gen_range(0..10), 10).unwrap_or('0'))
        .collect())
}

pub fn pronounceable(length: usize) -> Result<String> {
    const C: &[u8] = b"bcdfghjklmnprstvwxyz";
    const V: &[u8] = b"aeiou";
    if length == 0 {
        bail!("Length must be > 0");
    }
    let mut rng = rand::thread_rng();
    Ok((0..length)
        .map(|i| {
            let pool = if i % 2 == 0 { C } else { V };
            pool[rng.gen_range(0..pool.len())] as char
        })
        .collect())
}

fn system_wordlist() -> Option<Vec<String>> {
    for path in &[
        "/usr/share/dict/words",
        "/usr/share/dict/american-english",
        "/usr/share/dict/british-english",
    ] {
        if let Ok(text) = std::fs::read_to_string(path) {
            let words: Vec<String> = text
                .lines()
                .filter(|w| {
                    w.len() >= 4 && w.len() <= 8 && w.chars().all(|c| c.is_ascii_lowercase())
                })
                .map(String::from)
                .collect();
            if words.len() > 100 {
                return Some(words);
            }
        }
    }
    None
}

fn builtin_words() -> Vec<String> {
    "apple bench brick cabin dance eagle flame globe hatch ivory jelly kneel lemon maple
     niche ocean pearl quail rider siren tiger ultra vigor waltz yacht zebra abbey blaze
     cider delta ember flint gravel haven inlet jumbo kraft lunar mason nerve orbit pivot
     quartz ridge solar thorn ulcer verse whirl abbot bench choir depot envoy fable gnome
     hinge jewel karma latch mercy novel obese panel quota rivet staff tidal under venom
     witch axiom blast cubic diner elbow finch grout hedge iris jumpy knack lingo mural
     night oxide phase quiver radon scald three valid wheat xylem arena blend crane drift
     egret flare grain hotel index joust kitty lodge mocha notch optic plaza relay spoke
     tread unity vault wrath brisk civic dodge epoch foyer graft hoard imply joker knob"
        .split_whitespace()
        .map(String::from)
        .collect()
}

// ── Strength ─────────────────────────────────────────────────────────────────

pub struct Strength {
    pub score: u8, // 0-4
    pub bits: f64, // Shannon entropy
    pub crack_time: String,
    pub label: &'static str,
    pub suggestions: Vec<String>,
}

pub fn check_strength(password: &str) -> Strength {
    let est = match zxcvbn::zxcvbn(password, &[]) {
        Ok(e) => e,
        Err(_) => {
            return Strength {
                score: 0,
                bits: 0.0,
                crack_time: "instant".into(),
                label: "very weak",
                suggestions: vec![],
            }
        }
    };

    let score: u8 = est.score();
    let bits = shannon_entropy(password);
    let crack = format!(
        "{}",
        est.crack_times().offline_slow_hashing_1e4_per_second()
    );
    // feedback() returns &Option<Feedback> in zxcvbn 2.x
    let suggestions: Vec<String> = est
        .feedback()
        .as_ref()
        .map(|fb| {
            fb.suggestions()
                .iter()
                .map(|s| format!("{:?}", s))
                .collect()
        })
        .unwrap_or_default();
    let label = match score {
        0 => "very weak",
        1 => "weak",
        2 => "fair",
        3 => "strong",
        _ => "very strong",
    };

    Strength {
        score,
        bits,
        crack_time: crack,
        label,
        suggestions,
    }
}

pub fn shannon_entropy(s: &str) -> f64 {
    use std::collections::HashMap;
    let mut counts: HashMap<char, usize> = HashMap::new();
    for c in s.chars() {
        *counts.entry(c).or_insert(0) += 1;
    }
    let n = s.chars().count() as f64;
    if n == 0.0 {
        return 0.0;
    }
    counts.values().fold(0.0, |acc, &c| {
        let p = c as f64 / n;
        acc - p * p.log2()
    }) * n
}
