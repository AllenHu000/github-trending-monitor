use anyhow::{Context, Result};
use chrono::NaiveDate;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::github::Repo;

/// Snapshot of the current run, keyed by `full_name`.
/// Persisted to `trending_history.json`.
pub type Snapshot = HashMap<String, Repo>;

/// A single daily observation of a repository's star count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarPoint {
    pub date: String,
    pub stars: u64,
}

/// Time series of star counts per repository, keyed by `full_name`.
/// Persisted to `star_history.json`. Repos that drop out of the candidate pool
/// keep their existing series (entries are never deleted), so growth can be
/// resumed when they reappear.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct StarHistory(pub HashMap<String, Vec<StarPoint>>);

impl StarHistory {
    pub fn load(path: &str) -> Self {
        read_json(path).unwrap_or_default()
    }

    pub fn save(&self, path: &str) -> Result<()> {
        write_json(path, self)
    }
}

/// Persist the current snapshot to `trending_history.json`.
pub fn save_snapshot(path: &str, snapshot: &Snapshot) -> Result<()> {
    write_json(path, snapshot)
}

impl StarHistory {
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

    /// Most recent observation that is not `today`, together with the number of
    /// calendar days between that observation and `today` (at least 1). Used to
    /// normalize star growth to a per-day rate when a run was missed.
    pub fn previous_observation(&self, name: &str, today: &str) -> Option<(u64, i64)> {
        let series = self.0.get(name)?;
        let prev = series.iter().rev().find(|point| point.date != today)?;
        let days = NaiveDate::parse_from_str(today, "%Y-%m-%d")
            .ok()
            .zip(NaiveDate::parse_from_str(&prev.date, "%Y-%m-%d").ok())
            .map(|(t, p)| (t - p).num_days().max(1))
            .unwrap_or(1);
        Some((prev.stars, days))
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
