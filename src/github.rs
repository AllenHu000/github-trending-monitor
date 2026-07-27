use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

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

/// Fetch trending repositories: public, non-archived repos pushed in the last
/// 24h, ordered by total stars descending.
///
/// Uses the `GH_TOKEN` environment variable for authentication when present.
pub fn fetch_trending(per_page: u32) -> Result<Vec<Repo>> {
    let since = (Utc::now() - Duration::days(1)).date_naive();
    let query = format!("pushed:>={since} archived:false");
    let per_page = per_page.to_string();

    let client = reqwest::blocking::Client::builder()
        .user_agent("github-trending-monitor")
        .build()
        .context("failed to build HTTP client")?;

    let mut request = client
        .get(GITHUB_SEARCH_API)
        .query(&[
            ("q", query.as_str()),
            ("sort", "stars"),
            ("order", "desc"),
            ("per_page", per_page.as_str()),
        ])
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");

    if let Ok(token) = std::env::var("GH_TOKEN") {
        if !token.is_empty() {
            request = request.header("Authorization", format!("Bearer {token}"));
        }
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
