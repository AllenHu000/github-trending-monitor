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
use chrono::Local;

use github::Repo;
use ranking::Gainer;

const SNAPSHOT_FILE: &str = "trending_history.json";
const STAR_HISTORY_FILE: &str = "star_history.json";
const OUTPUT_MD: &str = "daily_new.md";
const DATA_JSON: &str = "docs/data.json";
const CONFIG_FILE: &str = "config.json";
const RANK_LIMIT: usize = 10;

fn main() -> Result<()> {
    let cfg = config::Config::load(CONFIG_FILE);
    let today = Local::now().format("%Y-%m-%d").to_string();

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

    // Update the star time series (today overwrites, prev record = last observation).
    star_history.append_today(&repos, &today, cfg.history_days);

    let gainers = ranking::top_gainers(&repos, &star_history, &today, RANK_LIMIT);
    let horses = ranking::dark_horses(&repos, &star_history, &today, &cfg.dark_horse, RANK_LIMIT);

    let markdown = build_markdown(&today, &new_items, &gainers, &horses, &repos, &cfg);

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

    notify::notify_dingtalk(&markdown, &cfg.at_mobiles)?;
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
        out.push_str("✅ 今日没有新增上榜仓库\n\n");
    } else {
        for repo in new_items {
            out.push_str(&format!(
                "- [{}]({}) ⭐{} | {}\n",
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

    let radar_lines: Vec<String> = repos
        .iter()
        .filter_map(|repo| {
            let hits = radar::match_keywords(repo, &cfg.keywords);
            if hits.is_empty() {
                return None;
            }
            Some(format!(
                "- [{}]({}) ⭐{} `{}` | {}",
                repo.name,
                repo.url,
                repo.stars,
                hits.join(","),
                repo.description
            ))
        })
        .collect();
    if !radar_lines.is_empty() {
        out.push_str("## 🎯 关键词雷达\n\n");
        out.push_str(&radar_lines.join("\n"));
        out.push('\n');
    }

    out
}
