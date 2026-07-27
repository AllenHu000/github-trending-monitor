use anyhow::{Context, Result};
use serde_json::json;
use std::collections::BTreeMap;

use crate::github::Repo;
use crate::history::StarHistory;

/// Export a static JSON payload consumed by the GitHub Pages front-end:
/// today's ranking, language distribution, and star time series for the repos
/// currently on the board.
pub fn export_json(path: &str, repos: &[Repo], history: &StarHistory, date: &str) -> Result<()> {
    let mut languages: BTreeMap<String, u32> = BTreeMap::new();
    for repo in repos {
        let key = repo.language.clone().unwrap_or_else(|| "Unknown".to_string());
        *languages.entry(key).or_insert(0) += 1;
    }

    let series: BTreeMap<&String, &Vec<_>> = history
        .0
        .iter()
        .filter(|(name, _)| repos.iter().any(|repo| &repo.name == *name))
        .collect();

    let payload = json!({
        "generated_at": date,
        "repos": repos,
        "languages": languages,
        "series": series,
    });

    let content = serde_json::to_string_pretty(&payload)
        .context("failed to serialize docs/data.json payload")?;
    std::fs::write(path, content).with_context(|| format!("failed to write {path}"))?;
    Ok(())
}
