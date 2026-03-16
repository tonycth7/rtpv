//! `rtpv search` — fuzzy search across entry names.

use anyhow::Result;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use crate::config::Config;
use crate::store::Store;

pub struct SearchResult {
    pub matches: Vec<SearchMatch>,
}

pub struct SearchMatch {
    pub path:  String,
    pub score: i64,
}

pub fn run(cfg: &Config, query: &str) -> Result<SearchResult> {
    let store = Store::new(cfg)?;
    let entries = store.list()?;
    let matcher = SkimMatcherV2::default();

    let mut matches: Vec<SearchMatch> = entries
        .into_iter()
        .filter_map(|path| {
            matcher.fuzzy_match(&path, query)
                .map(|score| SearchMatch { path, score })
        })
        .collect();

    matches.sort_by(|a, b| b.score.cmp(&a.score));
    Ok(SearchResult { matches })
}
