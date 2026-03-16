//! Password generation — random, passphrase (wordlist), PIN, pronounceable.

use anyhow::{bail, Result};
use rand::Rng;

// ─────────────────────────────────────────────────────────────────────────────
// Generation modes
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub enum GenMode {
    /// Random characters from a charset
    Random { length: usize, chars: String },
    /// Diceware-style word passphrase
    Words { count: usize, separator: char },
    /// Numeric PIN
    Pin { length: usize },
    /// CV alternating (pronounceable)
    Pronounceable { length: usize },
}

pub fn generate(mode: &GenMode) -> Result<String> {
    match mode {
        GenMode::Random { length, chars } => random_password(*length, chars),
        GenMode::Words  { count, separator } => passphrase(*count, *separator),
        GenMode::Pin    { length }           => pin(*length),
        GenMode::Pronounceable { length }    => pronounceable(*length),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Random charset password
// ─────────────────────────────────────────────────────────────────────────────
pub fn random_password(length: usize, chars: &str) -> Result<String> {
    if length == 0 { bail!("Password length must be > 0"); }

    // Expand character classes like A-Z, a-z, 0-9
    let pool = expand_charset(chars)?;
    if pool.is_empty() { bail!("Charset '{}' is empty", chars); }

    let mut rng = rand::thread_rng();
    let pw: String = (0..length)
        .map(|_| pool[rng.gen_range(0..pool.len())])
        .collect();
    Ok(pw)
}

/// Expand a charset string with ranges (e.g. "A-Za-z0-9@#") into a char vec.
fn expand_charset(chars: &str) -> Result<Vec<char>> {
    let mut pool = Vec::new();
    let chars_vec: Vec<char> = chars.chars().collect();
    let mut i = 0;
    while i < chars_vec.len() {
        if i + 2 < chars_vec.len() && chars_vec[i + 1] == '-' {
            let start = chars_vec[i] as u32;
            let end   = chars_vec[i + 2] as u32;
            if start > end {
                bail!("Invalid charset range: {}-{}", chars_vec[i], chars_vec[i+2]);
            }
            for c in start..=end {
                if let Some(ch) = char::from_u32(c) { pool.push(ch); }
            }
            i += 3;
        } else {
            pool.push(chars_vec[i]);
            i += 1;
        }
    }
    Ok(pool)
}

// ─────────────────────────────────────────────────────────────────────────────
// Word passphrase
// ─────────────────────────────────────────────────────────────────────────────
/// Built-in EFF wordlist excerpt — 512 common short words.
/// For production, we'll also check system wordlists.
const BUILTIN_WORDS: &[&str] = &[
    "apple","beach","brick","cabin","dance","eagle","flame","globe","hatch",
    "ivory","jelly","kneel","lemon","maple","niche","ocean","pearl","quail",
    "rider","siren","tiger","ultra","vigor","waltz","xerox","yacht","zebra",
    "abbey","blaze","cider","delta","ember","flint","gravel","haven","inlet",
    "jumbo","kraft","lunar","mason","nerve","orbit","pivot","quartz","ridge",
    "solar","thorn","ulcer","verse","whirl","abbot","bench","choir","depot",
    "envoy","fable","gnome","hinge","jewel","karma","latch","mercy","novel",
    "obese","panel","quota","rivet","staff","tidal","under","venom","witch",
    "axiom","blast","cubic","diner","elbow","finch","grout","hedge","iris",
    "jumpy","knack","lingo","mural","night","oxide","phase","quiver","radon",
    "scald","three","utmost","valid","wheat","xylem","arena","blend","crane",
    "drift","egret","flare","grain","hotel","index","joust","kitty","lodge",
    "mocha","notch","optic","plaza","quench","relay","spoke","tread","unity",
    "vault","wrath","anger","brisk","civic","dodge","epoch","foyer","graft",
    "hoard","imply","joker","knob","local","mantle","nerve","often","place",
    "quest","realm","shirt","thick","utter","vivid","woken","eight","ample",
    "broth","clamp","dwarf","ethic","frank","guile","hyena","icing","jiffy",
    "kinky","lofty","mimic","nadir","owing","pixel","quirk","ransom","spout",
    "tulip","usurp","valor","widen","xenon","yeast","agile","boxer","cliff",
    "denim","enact","fluke","gavel","hazel","infer","joust","knelt","lyric",
    "melon","noble","outdo","prism","quoth","ripen","snare","trout","usual",
    "verge","windy","acorn","birch","cedar","dingo","exist","fetch","greed",
    "hippo","ideal","jaded","kudos","leech","midst","nomad","other","porch",
    "rabbi","salve","touch","unwed","voice","warty","exert","brave","cello",
];

pub fn passphrase(count: usize, separator: char) -> Result<String> {
    if count == 0 { bail!("Word count must be > 0"); }

    // Prefer system wordlist
    let words: Vec<String> = if let Some(list) = system_wordlist() {
        list
    } else {
        BUILTIN_WORDS.iter().map(|s| s.to_string()).collect()
    };

    let mut rng = rand::thread_rng();
    let chosen: Vec<&str> = (0..count)
        .map(|_| words[rng.gen_range(0..words.len())].as_str())
        .collect();
    Ok(chosen.join(&separator.to_string()))
}

fn system_wordlist() -> Option<Vec<String>> {
    let candidates = [
        "/usr/share/dict/words",
        "/usr/share/dict/american-english",
        "/usr/share/dict/british-english",
        "/usr/dict/words",
    ];
    for path in &candidates {
        if let Ok(text) = std::fs::read_to_string(path) {
            let words: Vec<String> = text.lines()
                .filter(|w| {
                    let l = w.len();
                    l >= 4 && l <= 8 && w.chars().all(|c| c.is_ascii_lowercase())
                })
                .map(String::from)
                .collect();
            if words.len() > 100 { return Some(words); }
        }
    }
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// PIN
// ─────────────────────────────────────────────────────────────────────────────
pub fn pin(length: usize) -> Result<String> {
    if length == 0 { bail!("PIN length must be > 0"); }
    let mut rng = rand::thread_rng();
    Ok((0..length).map(|_| char::from_digit(rng.gen_range(0..10), 10)
        .unwrap_or('0')).collect())
}

// ─────────────────────────────────────────────────────────────────────────────
// Pronounceable (CV alternation)
// ─────────────────────────────────────────────────────────────────────────────
pub fn pronounceable(length: usize) -> Result<String> {
    const CONSONANTS: &[char] = &['b','c','d','f','g','h','j','k','l','m',
                                   'n','p','r','s','t','v','w','x','y','z'];
    const VOWELS:     &[char] = &['a','e','i','o','u'];
    if length == 0 { bail!("Length must be > 0"); }
    let mut rng = rand::thread_rng();
    let pw: String = (0..length).map(|i| {
        if i % 2 == 0 {
            CONSONANTS[rng.gen_range(0..CONSONANTS.len())]
        } else {
            VOWELS[rng.gen_range(0..VOWELS.len())]
        }
    }).collect();
    Ok(pw)
}

// ─────────────────────────────────────────────────────────────────────────────
// Strength scorer (wrapper around zxcvbn)
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct StrengthReport {
    pub score:       u8,     // 0–4
    pub entropy:     f64,    // bits
    pub crack_time:  String,
    pub suggestions: Vec<String>,
}

pub fn strength(password: &str) -> StrengthReport {
    let est = zxcvbn::zxcvbn(password, &[]);
    let est = match est {
        Ok(e)  => e,
        Err(_) => return StrengthReport { score: 0, entropy: 0.0, crack_time: "unknown".into(), suggestions: vec![] },
    };
    let score = est.score() as u8;
    let entropy = est.guesses_log10() * std::f64::consts::LOG2_E * 10.0 / std::f64::consts::LOG2_10;
    let crack_time = format!("{}", est.crack_times().offline_slow_hashing_1e4_per_second());
    let suggestions: Vec<String> = match est.feedback() {
       Some(f) => f.suggestions().iter().map(|s| format!("{:?}", s)).collect(),
       None    => vec![],
    };
    StrengthReport { score, entropy, crack_time, suggestions }
}

pub fn entropy_bits(password: &str) -> f64 {
    // Shannon entropy in bits: -Σ p(x) log₂ p(x)
    use std::collections::HashMap;
    let mut counts: HashMap<char, usize> = HashMap::new();
    for c in password.chars() { *counts.entry(c).or_insert(0) += 1; }
    let len = password.len() as f64;
    counts.values().fold(0.0, |acc, &c| {
        let p = c as f64 / len;
        acc - p * p.log2()
    })
}
