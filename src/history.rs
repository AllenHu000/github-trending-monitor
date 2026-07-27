use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::github::Repo;

/// Snapshot of the previous run, keyed by `full_name`.
/// Persisted to `trending_history.json` and used to detect newly listed repos.
pub type Snapshot = HashMap<String, Repo>;

/// Load the previous snapshot, returning an empty map when absent/invalid.
pub fn load_snapshot(path: &str) -> Snapshot {
    read_json(path).unwrap_or_default()
}

/// Persist the current snapshot.
pub fn save_snapshot(path: &str, snapshot: &Snapshot) -> Result<()> {
    write_json(path, snapshot)
}

/// A single daily observation of a repository's star count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarPoint {
    pub date: String,
    pub stars: u64,
}

/// Time series of star counts per repository, keyed by `full_name`.
/// Persisted to `star_history.json`.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct StarHistory(pub HashMap<String, Vec<StarPoint>>);

impl StarHistory {
    pub fn load(path: &str) -> Self {
        read_json(path).unwrap_or_default()
    }

    pub fn save(&self, path: &str) -> Result<()> {
        write_json(path, self)
    }

    /// Append today's star counts. If a point for `today` already exists it is
    /// overwritten (idempotent within a day). Prunes each series to the most
    /// recent `keep_days` points afterwards.
    pub fn append_today(&mut self, repos: &[Repo], today: &str, keep_days: i64) {
        for repo in repos {
            let series = self.0.entry(repo.name.clone()).or_default();
            match series.last_mut() {
                Some(last) if last.date == today => last.stars = repo.stars,
                _ => series.push(StarPoint {
                    date: today.to_string(),
                    stars: repo.stars,
                }),
            }
        }
        self.prune(keep_days);
    }

    fn prune(&mut self, keep_days: i64) {
        if keep_days <= 0 {
            return;
        }
        let keep = keep_days as usize;
        for series in self.0.values_mut() {
            if series.len() > keep {
                let start = series.len() - keep;
                series.drain(..start);
            }
        }
    }

    /// Star count from the most recent record that is not `today` (i.e. the
    /// previous day's snapshot), if any.
    pub fn previous_stars(&self, name: &str, today: &str) -> Option<u64> {
        self.0
            .get(name)?
            .iter()
            .rev()
            .find(|point| point.date != today)
            .map(|point| point.stars)
    }
}

fn read_json<T: DeserializeOwned>(path: &str) -> Option<T> {
    if !Path::new(path).exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn write_json<T: Serialize>(path: &str, value: &T) -> Result<()> {
    let content = serde_json::to_string_pretty(value)
        .with_context(|| format!("failed to serialize {path}"))?;
    std::fs::write(path, content).with_context(|| format!("failed to write {path}"))?;
    Ok(())
}
