import requests
import json
import os
from datetime import datetime

# GitHub trending api 第三方稳定接口（专门获取榜单）
TRENDING_API = "https://gh-trending-api.vercel.app/repositories"
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
    params = {"since": "daily"}
    resp = requests.get(TRENDING_API, params=params, timeout=30)
    resp.raise_for_status()
    return resp.json()

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

    with open(OUTPUT_MD, "w", encoding="utf-8") as f:
        f.write("\n".join(md_lines))

    print(f"采集完成！今日新上榜数量：{len(new_items)}")

if __name__ == "__main__":
    main()