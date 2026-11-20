use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::config::Config;

const GITHUB_SEARCH_API: &str = "https://api.github.com/search/repositories";

/// A trending repository, normalized from the GitHub search response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repo {
    pub name: String,
    pub url: String,
    pub stars: u64,
    pub description: String,
    pub language: Option<String>,
    pub pushed_at: String,
    #[serde(default)]
    pub topics: Vec<String>,
}

#[derive(Deserialize)]
struct SearchResponse {
    items: Vec<RawRepo>,
}

#[derive(Deserialize)]
struct RawRepo {
    full_name: String,
    html_url: String,
    stargazers_count: u64,
    description: Option<String>,
    language: Option<String>,
    pushed_at: String,
    #[serde(default)]
    topics: Vec<String>,
}

/// Fetch candidate repositories from three pools and merge/dedupe by `full_name`:
///
/// - Pool A: pushed within `pushed_window_days`, sorted by total stars — the
///   established headline repos.
/// - Pool B: created within `new_repo_days`, sorted by total stars — catches
///   fast-rising new repos that total-star sorting would otherwise bury.
/// - Pool C: keyword/topic pool built from `config.keywords` — subscribed domains
///   such as agent / aigc / harness / coding tools.
///
/// The GitHub search API cannot sort by recent star delta, so growth ranking is
/// computed locally from the persisted time series; this stage only widens the
/// candidate pool so nothing hot is filtered out before ranking.
pub fn fetch_trending(cfg: &Config) -> Result<Vec<Repo>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("github-trending-monitor")
        .build()
        .context("failed to build HTTP client")?;

    let token = std::env::var("GH_TOKEN").ok().filter(|t| !t.is_empty());
    let per_page = cfg.per_page.to_string();

    let today = Utc::now().date_naive();
    let pushed_since = (today - Duration::days(cfg.pushed_window_days.max(1)))
        .format("%Y-%m-%d")
        .to_string();
    let created_since = (today - Duration::days(cfg.new_repo_days.max(1)))
        .format("%Y-%m-%d")
        .to_string();

    let mut merged: Vec<Repo> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Pool A: recently pushed, high total stars.
    let pool_a = search(
        &client,
        &token,
        &format!("pushed:>={pushed_since} archived:false"),
        &per_page,
    )?;
    extend_merged(&mut merged, &mut seen, pool_a);

    // Pool B: recently created, independent of push activity.
    let pool_b = search(
        &client,
        &token,
        &format!("created:>={created_since} archived:false"),
        &per_page,
    )?;
    extend_merged(&mut merged, &mut seen, pool_b);

    // Pool C: keyword/topic pool for subscribed domains.
    if cfg.topic_pool && !cfg.keywords.is_empty() {
        let quoted: Vec<String> = cfg
            .keywords
            .iter()
            .map(|k| format!("\"{}\"", k.trim()))
            .collect();
        let query = format!("({}) archived:false", quoted.join(" OR "));
        let pool_c = search(&client, &token, &query, &per_page)?;
        extend_merged(&mut merged, &mut seen, pool_c);
    }

    Ok(merged)
}

fn extend_merged(
    merged: &mut Vec<Repo>,
    seen: &mut std::collections::HashSet<String>,
    items: Vec<Repo>,
) {
    for repo in items {
        if seen.insert(repo.name.clone()) {
            merged.push(repo);
        }
    }
}

fn search(
    client: &reqwest::blocking::Client,
    token: &Option<String>,
    query: &str,
    per_page: &str,
) -> Result<Vec<Repo>> {
    let mut request = client
        .get(GITHUB_SEARCH_API)
        .query(&[
            ("q", query),
            ("sort", "stars"),
            ("order", "desc"),
            ("per_page", per_page),
        ])
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");

    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {token}"));
    }

    let response = request
        .send()
        .context("request to GitHub search API failed")?
        .error_for_status()
        .context("GitHub search API returned an error status")?;

    let data: SearchResponse = response
        .json()
        .context("failed to parse GitHub search response")?;

    Ok(data
        .items
        .into_iter()
        .map(|raw| Repo {
            name: raw.full_name,
            url: raw.html_url,
            stars: raw.stargazers_count,
            description: raw.description.unwrap_or_else(|| "No description".to_string()),
            language: raw.language,
            pushed_at: raw.pushed_at,
            topics: raw.topics,
        })
        .collect())
}
