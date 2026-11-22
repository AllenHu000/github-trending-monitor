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

/// Fetch candidate repositories from four pools and merge/dedupe by `full_name`:
///
/// - Pool A: pushed within `pushed_window_days`, sorted by total stars — the
///   established headline repos.
/// - Pool B: created within `new_repo_days`, sorted by total stars — catches
///   fast-rising new repos that total-star sorting would otherwise bury.
/// - Pool C: keyword/topic pool built from `config.keywords` — subscribed domains
///   such as agent / aigc / harness / coding tools; wider page size and a star
///   floor so mid-size keyword hits are not sorted out by total stars.
/// - Pool D: explicit `watchlist` fetched straight from the repository API,
///   independent of search, for projects that never surface otherwise.
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
        let per_page = cfg.topic_pool_per_page.max(1).to_string();
        let mut first = true;
        let pool_c = search_keywords(&cfg.keywords, cfg.topic_min_stars, |query| {
            // Spread search requests to stay below GitHub's authenticated limit.
            if !first {
                std::thread::sleep(std::time::Duration::from_millis(2100));
            }
            first = false;
            search(&client, &token, query, &per_page)
        })?;
        extend_merged(&mut merged, &mut seen, pool_c);
    }

    // Pool D: explicit watchlist, fetched directly, never filtered by search.
    let pool_d = fetch_watchlist(&client, &token, &cfg.watchlist);
    extend_merged(&mut merged, &mut seen, pool_d);

    Ok(merged)
}

// Accumulate the complete pool before returning: a failed keyword must not
// produce a partial snapshot that overwrites persisted history.
fn search_keywords<F>(keywords: &[String], min_stars: u64, mut fetch: F) -> Result<Vec<Repo>>
where
    F: FnMut(&str) -> Result<Vec<Repo>>,
{
    let mut repos = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut seen_keywords = std::collections::HashSet::new();
    for keyword in keywords {
        let keyword = keyword.trim();
        if keyword.is_empty() || !seen_keywords.insert(keyword.to_lowercase()) {
            continue;
        }
        // Embedded quotes would alter the search syntax; reject them explicitly.
        anyhow::ensure!(!keyword.contains('"'), "invalid keyword: contains a quote");
        let query = format!("\"{keyword}\" archived:false stars:>{}", min_stars.max(0));
        let results =
            fetch(&query).with_context(|| format!("keyword search failed for {keyword:?}"))?;
        extend_merged(&mut repos, &mut seen, results);
    }
    Ok(repos)
}

/// Fetch watchlist repos (`owner/name`) directly from the repository endpoint.
/// Individual failures (404, rate limit) are logged and skipped so a bad
/// entry never aborts the whole run.
fn fetch_watchlist(
    client: &reqwest::blocking::Client,
    token: &Option<String>,
    names: &[String],
) -> Vec<Repo> {
    let mut out = Vec::new();
    for name in names {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let mut request = client
            .get(format!("https://api.github.com/repos/{name}"))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(token) = token {
            request = request.header("Authorization", format!("Bearer {token}"));
        }
        match request.send() {
            Ok(response) if response.status().is_success() => match response.json::<RawRepo>() {
                Ok(raw) => out.push(Repo {
                    name: raw.full_name,
                    url: raw.html_url,
                    stars: raw.stargazers_count,
                    description: raw
                        .description
                        .unwrap_or_else(|| "No description".to_string()),
                    language: raw.language,
                    pushed_at: raw.pushed_at,
                    topics: raw.topics,
                }),
                Err(err) => eprintln!("watchlist {name}: parse failed: {err}"),
            },
            Ok(response) => {
                eprintln!("watchlist {name}: skipped (HTTP {})", response.status());
            }
            Err(err) => eprintln!("watchlist {name}: request failed: {err}"),
        }
    }
    out
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

    let mut retries = 0;
    let response = loop {
        let response = request
            .try_clone()
            .context("failed to clone GitHub search request")?
            .send()
            .context("request to GitHub search API failed")?;
        let status = response.status();
        let headers = response.headers();
        let retry_after = headers
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        let exhausted = headers
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            == Some("0");
        let limited = status.as_u16() == 429
            || (status.as_u16() == 403 && (exhausted || retry_after.is_some()));
        if !limited || retries >= 2 {
            break response
                .error_for_status()
                .context("GitHub search API returned an error status")?;
        }
        let reset_wait = if exhausted {
            headers
                .get("x-ratelimit-reset")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .map(|reset| {
                    reset
                        .saturating_sub(Utc::now().timestamp().max(0) as u64)
                        .saturating_add(1)
                })
        } else {
            None
        };
        let wait = retry_after.unwrap_or(60).max(reset_wait.unwrap_or(0));
        anyhow::ensure!(
            wait <= 60,
            "GitHub search rate limit requires waiting {wait}s; aborting to preserve history"
        );
        eprintln!("GitHub search rate limited (HTTP {status}); retrying in {wait}s");
        std::thread::sleep(std::time::Duration::from_secs(wait));
        retries += 1;
    };

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
            description: raw
                .description
                .unwrap_or_else(|| "No description".to_string()),
            language: raw.language,
            pushed_at: raw.pushed_at,
            topics: raw.topics,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(name: &str) -> Repo {
        Repo {
            name: name.into(),
            url: String::new(),
            stars: 0,
            description: String::new(),
            language: None,
            pushed_at: String::new(),
            topics: Vec::new(),
        }
    }

    #[test]
    fn searches_each_keyword_and_deduplicates_results() {
        let keywords = vec![" agent ".into(), "mcp".into(), "AGENT".into(), " ".into()];
        let mut queries = Vec::new();
        let results = search_keywords(&keywords, 1000, |query| {
            queries.push(query.to_string());
            Ok(vec![
                repo("a/shared"),
                repo(if query.starts_with("\"agent\"") {
                    "a/agent"
                } else {
                    "a/mcp"
                }),
            ])
        })
        .unwrap();
        assert_eq!(
            queries,
            [
                "\"agent\" archived:false stars:>1000",
                "\"mcp\" archived:false stars:>1000"
            ]
        );
        assert_eq!(
            results.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            ["a/shared", "a/agent", "a/mcp"]
        );
    }

    #[test]
    fn propagates_failure_instead_of_returning_partial_pool() {
        let keywords = vec!["agent".into(), "mcp".into()];
        let mut calls = 0;
        let result = search_keywords(&keywords, 1000, |_| {
            calls += 1;
            if calls == 1 {
                Ok(vec![repo("a/shared")])
            } else {
                anyhow::bail!("HTTP 422")
            }
        });
        assert!(result.is_err());
        assert_eq!(calls, 2);
        assert!(format!("{:#}", result.unwrap_err()).contains("HTTP 422"));
    }

    #[test]
    fn rejects_quotes_in_keywords() {
        let result = search_keywords(&["bad\"keyword".into()], 1000, |_| unreachable!());
        assert!(result.is_err());
    }
}
