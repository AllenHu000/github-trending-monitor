# GitHub Trending Monitor

每天通过 GitHub 官方 REST API 生成热门项目日报，并提交结果到仓库。

GitHub 没有官方的 Trending API。本项目将“热门”定义为：过去 24 小时内有推送、未归档的公开仓库，并按总 Star 数降序排列。`trending_history.json` 保存上一轮榜单快照，`daily_new.md` 只列出本轮新入榜的项目。

## GitHub Actions

工作流每天北京时间 08:00 运行，也可以从 Actions 页面手动触发。默认使用 GitHub Actions 自带的 `github.token` 调用 API 和提交结果；如需使用 Personal Access Token，可在仓库 Secrets 中配置 `GH_TOKEN` 覆盖默认 Token。

## DingTalk 日报

脚本保留了可选的钉钉机器人推送适配。将机器人 Webhook 保存为仓库 Secret `DINGTALK_WEBHOOK` 后，任务会在生成日报后发送 Markdown 消息；未配置该 Secret 时，只生成并提交本地日报文件。

如果机器人启用了签名校验，需要在后续扩展中同时配置签名密钥并为请求生成签名；当前实现适用于仅使用 Webhook 地址的机器人。

### 排查未收到消息

在 Actions 运行日志的 `Run trending crawler` 步骤检查钉钉日志：

- `DINGTALK_WEBHOOK is not configured`：确认 Secret 名称为 `DINGTALK_WEBHOOK`，并重新触发工作流。Fork 仓库发起的 Pull Request 不会获得 Actions Secrets。
- `errcode=...`：钉钉已拒绝请求，按照日志中的 `errmsg` 检查机器人是否被禁用、Webhook 是否过期、安全设置是否匹配。
- 工作流显示 `DingTalk notification sent successfully` 但未收到消息：检查目标群与机器人是否正确，及机器人“自定义关键词”规则是否允许消息中的 `GitHub`。

钉钉业务错误可能以 HTTP 200 返回；脚本会将这类响应视为失败，因此日志中的错误码是排查的首要依据。
