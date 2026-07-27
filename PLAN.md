# GitHub Trending Monitor — 演进计划

> 跨端事实源。本文件随 git 同步，多设备 `git pull` / `git commit` 迭代。
> session/repo 记忆不进 git，仅作本机临时缓存。

## 背景

现有实现：单文件 Python 脚本 `fetch_trending.py`，GitHub Actions 每日 08:00（北京时间）运行。
用 GitHub Search API 拉「过去 24h 有推送、未归档」的 top30 仓库，与 `trending_history.json`
快照 diff 出新上榜项目，生成 `daily_new.md`，可选推送钉钉。

## 决策（已确认）

- 用 **Rust** 重写（兼学习目的），blocking `reqwest` + `anyhow`，避开 async。
- 聚焦纯数据类功能：涨星 / 黑马榜、关键词雷达；**暂不接 AI**（仅留 trait 插槽）。
- 加 **GitHub Pages** 静态可视化；保持纯 GitHub Actions 运行。
- 阈值 / 关键词放 `config.json`（可进 git）。
- Rust 验证通过后替换并删除 Python 版，不长期双语言维护。
- 黑马时序只跟踪上过榜仓库（接受曲线断点）。
- 跨端事实源 = 仓库根目录 `PLAN.md`（git 跟踪）。

## 可行性结论

可行且适合练手。风险点仅 CI 编译耗时，用 `Swatinem/rust-cache` 可控。
Crate 选型：reqwest(blocking,json) / serde(derive) / serde_json / chrono / anyhow。

---

## 目标项目结构

```text
Cargo.toml
src/
  main.rs        # 编排（对应 Python main）
  config.rs      # 读 config.json
  github.rs      # Repo struct + fetch_trending
  history.rs     # Snapshot + StarHistory 读写
  ranking.rs     # delta / growth / 黑马计算
  radar.rs       # 关键词匹配
  notify.rs      # 钉钉推送（含 at）
  export.rs      # 导出 docs/data.json
  summarize.rs   # Summarizer trait（占位，默认返回 description）
config.json
star_history.json      # 新：时序
trending_history.json  # 保留：上一轮快照
daily_new.md
docs/
  index.html
  data.json
.github/workflows/*.yml
```

---

## 本轮实施

### Phase 0 — Rust 脚手架

- `cargo init`；`.gitignore` 加 `/target`。
- Cargo.toml 依赖（参考版本，实现时取最新兼容）：
  - `reqwest = { version = "0.12", features = ["blocking", "json"] }`
  - `serde = { version = "1", features = ["derive"] }`
  - `serde_json = "1"`
  - `chrono = { version = "0.4", features = ["clock"] }`
  - `anyhow = "1"`
- 建模块骨架 + `mod` 声明。
- 验收：`cargo build` 通过。

### Phase 1 — 抓取 + 数据模型

- `github.rs`：
  - `struct Repo { name, url, stars, description, language: Option<String>, pushed_at, topics: Vec<String> }`
  - 内部用 `#[derive(Deserialize)]` 的原始响应 struct 再映射为 `Repo`。
  - `fn fetch_trending() -> anyhow::Result<Vec<Repo>>`
    - since = 昨天 UTC date；`q = pushed:>={since} archived:false`，sort=stars，per_page=30。
    - header：Accept + X-GitHub-Api-Version；有 `GH_TOKEN` 则加 Bearer。
- `history.rs`：
  - `Snapshot`（对应 `trending_history.json`，判断新上榜）。
  - `StarPoint { date, stars }`，`StarHistory(HashMap<String, Vec<StarPoint>>)`（`star_history.json`）。
  - `append_today()`：同日覆盖，滚动保留最近 90 天。
- 验收：能拉数据、正确读写两个 JSON。

### Phase 2 — 涨星 / 黑马榜（点子 3）

- `ranking.rs`：
  - `daily_delta(history, name) -> Option<i64>`（今日 − 昨日）。
  - `top_gainers(...)`：按 delta 降序取 Top10。
  - `dark_horses(...)`：基数小但增速高，过滤噪声（min_stars / min_delta）。
- 首次运行只有 1 天数据 → delta 为 None，跳过榜单。

### Phase 3 — 关键词雷达（点子 4）

- `config.rs`：`Config { keywords, at_mobiles, dark_horse{min_stars, min_delta}, history_days }`，缺省给默认值。
- `radar.rs`：`match_keywords(repo, kws)` 大小写不敏感，匹配 name + description + topics。

### Phase 4 — 输出与推送

- `notify.rs`：`notify_dingtalk(markdown, at_mobiles)`，payload 增加 `at { atMobiles, isAtAll:false }`；errcode≠0 视为失败。
- `main.rs` 生成 `daily_new.md` 分段：`🆕 新上榜 / 🚀 涨星最快 / 🐎 黑马 / 🎯 关键词雷达`。

### Phase 5 — GitHub Pages 可视化

- `export.rs`：`export_json(...)` 写 `docs/data.json`（当日榜单 + 语言分布 + 各仓库 star 时序）。
- `docs/index.html`：Chart.js(CDN) 纯静态加载 `data.json`：当日榜单表 / 语言分布饼图 / 单仓库 star 趋势线。
- 仓库设置 Pages Source = `docs/`。

### Phase 6 — CI/CD

- workflow 改：checkout → `dtolnay/rust-toolchain@stable` → `Swatinem/rust-cache` → `cargo build --release` → 运行二进制 → commit 状态文件。
- env/secrets：`GH_TOKEN`（可选）、`DINGTALK_WEBHOOK`。
- 验证通过后删除 `fetch_trending.py`，更新 README 为 Rust 说明。

---

## 验证清单

1. `cargo build` / `cargo clippy` 无 warning。
2. 造 2 天 `star_history.json` 假数据，`cargo run` 验证 delta / 黑马。
3. 关键词命中 / 未命中两 case，检查 md 分段与钉钉 payload（含 `at`）。
4. `python -m http.server` 起 `docs/` 验证图表。
5. Actions 手动触发，确认二进制运行、状态文件提交、Pages 可访问。

---

## 后续可持续演进 Roadmap（逐步完善，非本轮实现）

### R1 — AI 智能摘要 + 中文点评（点子 1）

- 实现 `summarize.rs` 的 `Summarizer` trait：调用 LLM 读 README/description，产出「解决什么 / 适合谁 / 亮点」一句话点评。
- 模型：倾向国内模型（通义 / DeepSeek / Kimi），env 存 API Key。
- 细化：新增 `LlmSummarizer` impl；带缓存（按 repo+commit）避免重复调用；失败降级为原 description。

### R2 — 周报 / 月报（点子 6）

- 新增聚合任务：读 `star_history.json` 汇总一周所有新上榜 + 涨星，生成「本周开源趋势」；workflow 加每周 cron。
- 细化：`ranking.rs` 增加 weekly 聚合；输出 `weekly_report.md` + 推送。

### R3 — 多渠道推送（点子 7）

- 抽象 `trait Notifier { fn send(&self, md) -> Result<()> }`。
- 实现 DingTalk / 飞书 / 企业微信 / Telegram / 邮件(SMTP) / RSS(生成 feed.xml)。
- 细化：`notify.rs` 拆为 `notify/` 子模块；`config.json` 增加 channels 配置。

### R4 — 关键词雷达进阶

- 支持正则 / 多组订阅 / 权重打分 / 每组独立 @人。
- 命中历史去重，避免同一仓库反复提醒。

### R5 — 去重降噪

- 过滤 awesome 列表 / 刷榜 / 无实质代码仓库（启发式：有无 release、code/doc 比例、star 增速异常检测）。

### R6 — 趋势可视化站进阶（点子 5/9）

- `docs/` 升级为多页：语言热度时间线、主题聚类、可搜索 / 可订阅。
- 进一步可做交互式 Web App（前端框架），支持关键词订阅、AI 问答。

### R7 — AI 选型助手（点子 10）

- 基于沉淀的趋势库做检索：输入需求 → 推荐相关热门项目 + 对比。
- 需引入向量检索 / embedding（后期，较大投入）。

### R8 — 开源日报播客（点子 11）

- AI 把日报改写口语化脚本 → TTS 生成音频 → 发布「开源日报播客」。

### 演进优先级

- 近期：R1(AI摘要) → R3(多渠道) → R2(周报)
- 中期：R4/R5(质量提升) → R6(可视化站)
- 远期：R7(选型助手) → R8(播客)
