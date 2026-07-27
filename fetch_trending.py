import requests
import json
import os
from datetime import datetime, timedelta, timezone

GITHUB_SEARCH_API = "https://api.github.com/search/repositories"
HISTORY_FILE = "trending_history.json"
OUTPUT_MD = "daily_new.md"

def load_history():
    if os.path.exists(HISTORY_FILE):
        with open(HISTORY_FILE, "r", encoding="utf-8") as f:
            return json.load(f)
    return {}

def save_history(data):
    with open(HISTORY_FILE, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)

def fetch_trending():
    since = (datetime.now(timezone.utc) - timedelta(days=1)).date().isoformat()
    params = {
        "q": f"pushed:>={since} archived:false",
        "sort": "stars",
        "order": "desc",
        "per_page": 30,
    }
    headers = {
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
    }
    token = os.getenv("GH_TOKEN")
    if token:
        headers["Authorization"] = f"Bearer {token}"

    resp = requests.get(GITHUB_SEARCH_API, params=params, headers=headers, timeout=30)
    resp.raise_for_status()
    return [
        {
            "name": item["full_name"],
            "url": item["html_url"],
            "stars": item["stargazers_count"],
            "description": item["description"] or "No description",
            "language": item["language"],
            "pushed_at": item["pushed_at"],
        }
        for item in resp.json()["items"]
    ]

def notify_dingtalk(markdown):
    webhook = os.getenv("DINGTALK_WEBHOOK")
    if not webhook:
        return

    payload = {
        "msgtype": "markdown",
        "markdown": {
            "title": "GitHub 热门项目日报",
            "text": markdown,
        },
    }
    resp = requests.post(webhook, json=payload, timeout=30)
    resp.raise_for_status()

def main():
    history = load_history()
    today_list = fetch_trending()
    today_ids = {}
    new_items = []

    for repo in today_list:
        repo_full = repo["url"].replace("https://github.com/", "")
        today_ids[repo_full] = repo
        if repo_full not in history:
            new_items.append(repo)

    # 更新历史快照
    save_history(today_ids)

    # 生成markdown日志
    date_str = datetime.now().strftime("%Y-%m-%d")
    md_lines = [f"# {date_str} 新上榜 Trending 项目\n"]
    if not new_items:
        md_lines.append("✅ 今日没有新增上榜仓库")
    else:
        for item in new_items:
            md_lines.append(f"- [{item['name']}]({item['url']}) ⭐{item['stars']} | {item['description']}")

    markdown = "\n".join(md_lines)
    with open(OUTPUT_MD, "w", encoding="utf-8") as f:
        f.write(markdown)

    notify_dingtalk(markdown)
    print(f"采集完成！今日新上榜数量：{len(new_items)}")

if __name__ == "__main__":
    main()