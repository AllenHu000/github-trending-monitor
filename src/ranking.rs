use crate::config::DarkHorse;
use crate::github::Repo;
use crate::history::StarHistory;

/// A repository together with its daily star delta.
pub struct Gainer<'a> {
    pub repo: &'a Repo,
    pub delta: i64,
}

/// Repositories with the largest positive daily star delta, descending.
pub fn top_gainers<'a>(
    repos: &'a [Repo],
    history: &StarHistory,
    today: &str,
    limit: usize,
) -> Vec<Gainer<'a>> {
    let mut gainers: Vec<Gainer<'a>> = repos
        .iter()
        .filter_map(|repo| delta_for(repo, history, today).filter(|d| *d > 0).map(|delta| Gainer { repo, delta }))
        .collect();
    gainers.sort_by(|a, b| b.delta.cmp(&a.delta));
    gainers.truncate(limit);
    gainers
}

/// "Dark horses": small-base repositories growing fast, filtered by config
/// thresholds, ordered by delta descending.
pub fn dark_horses<'a>(
    repos: &'a [Repo],
    history: &StarHistory,
    today: &str,
    cfg: &DarkHorse,
    limit: usize,
) -> Vec<Gainer<'a>> {
    let mut horses: Vec<Gainer<'a>> = repos
        .iter()
        .filter_map(|repo| {
            let delta = delta_for(repo, history, today)?;
            let qualifies = repo.stars >= cfg.min_stars
                && repo.stars <= cfg.max_stars
                && delta >= cfg.min_delta;
            qualifies.then_some(Gainer { repo, delta })
        })
        .collect();
    horses.sort_by(|a, b| b.delta.cmp(&a.delta));
    horses.truncate(limit);
    horses
}

fn delta_for(repo: &Repo, history: &StarHistory, today: &str) -> Option<i64> {
    let previous = history.previous_stars(&repo.name, today)?;
    Some(repo.stars as i64 - previous as i64)
}
