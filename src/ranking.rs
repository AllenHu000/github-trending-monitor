use crate::config::DarkHorse;
use crate::github::Repo;
use crate::history::StarHistory;

/// A repository together with its star growth since the previous observation.
///
/// `delta` is the raw total star change over `days`; `daily` is the normalized
/// per-day rate used for ranking, so a missed run does not inflate the growth
/// figure.
pub struct Gainer<'a> {
    pub repo: &'a Repo,
    pub delta: i64,
    pub daily: i64,
    pub days: i64,
}

/// Repositories with the largest positive normalized daily star growth, descending.
pub fn top_gainers<'a>(
    repos: &'a [Repo],
    history: &StarHistory,
    today: &str,
    limit: usize,
) -> Vec<Gainer<'a>> {
    let mut gainers: Vec<Gainer<'a>> = repos
        .iter()
        .filter_map(|repo| {
            let gainer = delta_for(repo, history, today)?;
            (gainer.daily > 0).then_some(gainer)
        })
        .collect();
    gainers.sort_by(|a, b| b.daily.cmp(&a.daily));
    gainers.truncate(limit);
    gainers
}

/// "Dark horses": small-base repositories growing fast, filtered by config
/// thresholds (against the normalized daily rate), ordered by daily growth desc.
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
            let gainer = delta_for(repo, history, today)?;
            let qualifies = repo.stars >= cfg.min_stars
                && repo.stars <= cfg.max_stars
                && gainer.daily >= cfg.min_delta;
            qualifies.then_some(gainer)
        })
        .collect();
    horses.sort_by(|a, b| b.daily.cmp(&a.daily));
    horses.truncate(limit);
    horses
}

fn delta_for<'a>(repo: &'a Repo, history: &StarHistory, today: &str) -> Option<Gainer<'a>> {
    let (previous, days) = history.previous_observation(&repo.name, today)?;
    let delta = repo.stars as i64 - previous as i64;
    let daily = delta / days.max(1);
    Some(Gainer {
        repo,
        delta,
        daily,
        days,
    })
}
