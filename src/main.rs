mod config;
mod export;
mod github;
mod history;
mod notify;
mod radar;
mod ranking;
// Placeholder for the future AI-summary feature (Roadmap R1); intentionally unused for now.
#[allow(dead_code)]
mod summarize;

use anyhow::Result;
use chrono::{FixedOffset, Utc};

use github::Repo;
use ranking::Gainer;

/// Beijing offset (the runner's local clock is UTC; the report date must match
/// the reader's calendar).
fn beijing_today() -> String {
    let shanghai = FixedOffset::east_opt(8 * 3600).expect("valid offset");
    Utc::now()
        .with_timezone(&shanghai)
        .format("%Y-%m-%d")
        .to_string()
}

const SNAPSHOT_FILE: &str = "trending_history.json";
const STAR_HISTORY_FILE: &str = "star_history.json";
const OUTPUT_MD: &str = "daily_new.md";
const DATA_JSON: &str = "docs/data.json";
const CONFIG_FILE: &str = "config.json";
const RANK_LIMIT: usize = 10;
/// Repository web URL, used to link the full report from the DingTalk digest.
const REPO_URL: &str = "https://github.com/AllenHu000/github-trending-monitor";

fn main() -> Result<()> {
    let cfg = config::Config::load(CONFIG_FILE);
    let today = beijing_today();

    let repos = github::fetch_trending(&cfg)?;

    // Load the persisted time series BEFORE appending today's point, so it still
    // reflects past runs. "Newly listed" means never seen in any past run, not
    // merely absent from yesterday's snapshot — a repo that dropped off and
    // comes back must not be re-reported as new.
    let mut star_history = history::StarHistory::load(STAR_HISTORY_FILE);
    let seen_before = |name: &str| star_history.0.contains_key(name);
    let new_items: Vec<&Repo> = repos
        .iter()
        .filter(|repo| !seen_before(&repo.name))
        .collect();

    // The full unseen list may be noisy (emulators, utilities with no domain
    // relevance). Only surface radar hits and explicit watchlist repos in the
    // report; everything else still gets recorded into the time series below.
    let is_watch = |name: &str| cfg.watchlist.iter().any(|w| w.trim() == name);
    let shown_new: Vec<&Repo> = new_items
        .iter()
        .copied()
        .filter(|repo| is_watch(&repo.name) || !radar::match_keywords(repo, &cfg.keywords).is_empty())
        .collect();

    // Update the star time series (today overwrites, prev record = last observation).
    star_history.append_today(&repos, &today, cfg.history_days);

    let gainers = ranking::top_gainers(&repos, &star_history, &today, RANK_LIMIT);
    let horses = ranking::dark_horses(&repos, &star_history, &today, &cfg.dark_horse, RANK_LIMIT);

    let markdown = build_markdown(&today, &shown_new, &gainers, &horses, &repos, &cfg);

    // Persist state and outputs.
    let snapshot: history::Snapshot = repos
        .iter()
        .map(|repo| (repo.name.clone(), repo.clone()))
        .collect();
    history::save_snapshot(SNAPSHOT_FILE, &snapshot)?;
    star_history.save(STAR_HISTORY_FILE)?;
    std::fs::write(OUTPUT_MD, &markdown)?;
    std::fs::create_dir_all("docs")?;
    export::export_json(DATA_JSON, &repos, &star_history, &today)?;

    // DingTalk only receives a short digest; the full report lives in daily_new.md.
    let digest = build_digest(&today, &shown_new, &gainers, &horses, &repos, &cfg);
    if let Err(err) = notify::notify_dingtalk(&digest, &cfg.at_mobiles) {
        // A notification-channel failure must not block persisting the snapshot and report.
        eprintln!("Warning: DingTalk notification failed (data already saved): {err:#}");
    }
    println!("采集完成！今日新上榜数量：{}", new_items.len());
    Ok(())
}

fn build_markdown(
    today: &str,
    new_items: &[&Repo],
    gainers: &[Gainer<'_>],
    horses: &[Gainer<'_>],
    repos: &[Repo],
    cfg: &config::Config,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {today} GitHub Trending 日报\n\n"));

    out.push_str("## 🆕 新上榜\n\n");
    if new_items.is_empty() {
        out.push_str("✅ 今日无值得关注的新上榜仓库（其余新仓已静默入档）\n\n");
    } else {
        for repo in new_items {
            let badge = watch_badge(&repo.name, &cfg.watchlist);
            out.push_str(&format!(
                "- {badge}[{}]({}) ⭐{} | {}\n",
                repo.name, repo.url, repo.stars, repo.description
            ));
        }
        out.push('\n');
    }

    if !gainers.is_empty() {
        out.push_str("## 🚀 涨星最快（按日均）\n\n");
        for gainer in gainers {
            out.push_str(&format!(
                "- [{}]({}) +{}/day (+{} over {}d) ⭐{}\n",
                gainer.repo.name, gainer.repo.url, gainer.daily, gainer.delta, gainer.days, gainer.repo.stars
            ));
        }
        out.push('\n');
    }

    if !horses.is_empty() {
        out.push_str("## 🐎 黑马（按日均）\n\n");
        for horse in horses {
            out.push_str(&format!(
                "- [{}]({}) +{}/day (+{} over {}d) ⭐{}\n",
                horse.repo.name, horse.repo.url, horse.daily, horse.delta, horse.days, horse.repo.stars
            ));
        }
        out.push('\n');
    }

    const FULL_RADAR_LIMIT: usize = 100;
    let radar_lines: Vec<String> = repos
        .iter()
        .filter_map(|repo| {
            let hits = radar::match_keywords(repo, &cfg.keywords);
            if hits.is_empty() {
                return None;
            }
            let badge = watch_badge(&repo.name, &cfg.watchlist);
            Some(format!(
                "- {badge}[{}]({}) ⭐{} `{}` | {}",
                repo.name,
                repo.url,
                repo.stars,
                hits.join(","),
                repo.description
            ))
        })
        .collect();
    if !radar_lines.is_empty() {
        let omitted = radar_lines.len().saturating_sub(FULL_RADAR_LIMIT);
        let shown: Vec<String> = radar_lines
            .into_iter()
            .take(FULL_RADAR_LIMIT)
            .collect();
        out.push_str("## 🎯 关键词雷达\n\n");
        out.push_str(&shown.join("\n"));
        if omitted > 0 {
            out.push_str(&format!(
                "\n- … 其余 {} 条雷达命中见 docs/data.json",
                omitted
            ));
        }
        out.push('\n');
    }

    out
}

/// Short digest for the DingTalk side channel: capped per section so it stays
/// well under the robot's 5000-character markdown limit. The full report with
/// every candidate is written to `daily_new.md`.
fn build_digest(
    today: &str,
    new_items: &[&Repo],
    gainers: &[Gainer<'_>],
    horses: &[Gainer<'_>],
    repos: &[Repo],
    cfg: &config::Config,
) -> String {
    const NEW_LIMIT: usize = 15;
    const RANK_LIMIT: usize = 8;
    const RADAR_LIMIT: usize = 25;
    /// Character budget for the digest body, leaving headroom under the robot's
    /// 5000-character limit (footer link + truncation fallback live outside it).
    const BUDGET: usize = 4300;
    const DESC_LIMIT: usize = 80;

    let mut out = String::new();
    out.push_str(&format!("# {today} GitHub Trending 日报（摘要）\n\n"));

    if new_items.is_empty() {
        out.push_str("✅ 今日无值得关注的新上榜仓库\n\n");
    } else {
        out.push_str("## 🆕 新上榜\n\n");
        for repo in new_items.iter().take(NEW_LIMIT) {
            let badge = watch_badge(&repo.name, &cfg.watchlist);
            let desc = truncate_desc(&repo.description, DESC_LIMIT);
            out.push_str(&format!(
                "- {badge}[{}]({}) ⭐{} | {}\n",
                repo.name, repo.url, repo.stars, desc
            ));
        }
        if new_items.len() > NEW_LIMIT {
            out.push_str(&format!(
                "- … 其余 {} 个新上榜见仓库日报\n",
                new_items.len() - NEW_LIMIT
            ));
        }
        out.push('\n');
    }

    if !gainers.is_empty() {
        out.push_str("## 🚀 涨星最快\n\n");
        for gainer in gainers.iter().take(RANK_LIMIT) {
            out.push_str(&format!(
                "- [{}]({}) +{}/day ⭐{}\n",
                gainer.repo.name, gainer.repo.url, gainer.daily, gainer.repo.stars
            ));
        }
        out.push('\n');
    }

    if !horses.is_empty() {
        out.push_str("## 🐎 黑马\n\n");
        for horse in horses.iter().take(RANK_LIMIT) {
            out.push_str(&format!(
                "- [{}]({}) +{}/day ⭐{}\n",
                horse.repo.name, horse.repo.url, horse.daily, horse.repo.stars
            ));
        }
        out.push('\n');
    }

    let radar_lines: Vec<String> = repos
        .iter()
        .filter_map(|repo| {
            let hits = radar::match_keywords(repo, &cfg.keywords);
            if hits.is_empty() {
                return None;
            }
            let badge = watch_badge(&repo.name, &cfg.watchlist);
            Some(format!(
                "- {badge}[{}]({}) ⭐{} `{}`",
                repo.name,
                repo.url,
                repo.stars,
                hits.join(",")
            ))
        })
        .take(RADAR_LIMIT)
        .collect();
    if !radar_lines.is_empty() {
        out.push_str("## 🎯 关键词雷达\n\n");
        let mut first = true;
        for line in radar_lines {
            // Fill the remaining character budget instead of a fixed item cap,
            // so quiet days surface more radar hits and busy days stay in limit.
            if !first && out.chars().count() + line.chars().count() + 1 > BUDGET {
                break;
            }
            if !first {
                out.push('\n');
            }
            out.push_str(&line);
            first = false;
        }
        out.push('\n');
    }

    out.push_str(&format!(
        "\n> 完整日报与全部候选仓库：[daily_new.md]({repo_url}/blob/main/daily_new.md)\n",
        repo_url = REPO_URL
    ));
    out
}

/// Shorten a description to at most `limit` characters, appending an ellipsis
/// when it is cut, so one long description cannot crowd out other digest items.
fn truncate_desc(desc: &str, limit: usize) -> String {
    if desc.chars().count() <= limit {
        return desc.to_string();
    }
    let mut cut: String = desc.chars().take(limit).collect();
    cut.push_str("…");
    cut
}

/// Prefix badge for repos on the explicit watchlist.
fn watch_badge(name: &str, watchlist: &[String]) -> &'static str {
    if watchlist.iter().any(|w| w.trim() == name) {
        "🔖 "
    } else {
        ""
    }
}
