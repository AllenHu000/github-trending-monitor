use serde::Deserialize;
use std::path::Path;

/// Runtime configuration loaded from `config.json`.
///
/// Missing fields fall back to the `Default` implementations, so an absent or
/// partial config file still yields a usable configuration.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Keywords for the radar section (case-insensitive match).
    pub keywords: Vec<String>,
    /// DingTalk mobiles to @ when radar keywords are hit.
    pub at_mobiles: Vec<String>,
    /// Dark-horse ranking thresholds.
    pub dark_horse: DarkHorse,
    /// How many daily points to keep per repo in the star history.
    pub history_days: i64,
    /// Number of repositories to request from the search API.
    pub per_page: u32,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct DarkHorse {
    /// Minimum total stars to filter out dead/noise repos.
    pub min_stars: u64,
    /// Upper bound on total stars, so only "small base" repos qualify.
    pub max_stars: u64,
    /// Minimum daily star delta to qualify as a dark horse.
    pub min_delta: i64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            keywords: Vec::new(),
            at_mobiles: Vec::new(),
            dark_horse: DarkHorse::default(),
            history_days: 90,
            per_page: 30,
        }
    }
}

impl Default for DarkHorse {
    fn default() -> Self {
        Self {
            min_stars: 50,
            max_stars: 2000,
            min_delta: 30,
        }
    }
}

impl Config {
    /// Load config from `path`, falling back to defaults when the file is
    /// missing or cannot be parsed.
    pub fn load(path: &str) -> Self {
        if !Path::new(path).exists() {
            return Config::default();
        }
        match std::fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|err| {
                eprintln!("Warning: failed to parse {path}: {err}. Using defaults.");
                Config::default()
            }),
            Err(err) => {
                eprintln!("Warning: failed to read {path}: {err}. Using defaults.");
                Config::default()
            }
        }
    }
}
