use anyhow::{anyhow, bail, Context, Result};
use serde_json::json;

/// DingTalk robot markdown `text` accepts at most 5000 characters; stay under
/// it with a safety margin so an overlong digest cannot be rejected as oversized.
const MAX_TEXT_CHARS: usize = 4500;

/// Send a Markdown message to a DingTalk robot webhook.
///
/// Skips silently when `DINGTALK_WEBHOOK` is not configured. When `at_mobiles`
/// is non-empty, those users are @-mentioned. A DingTalk business error
/// (`errcode != 0`) is treated as a failure even though it arrives with HTTP 200.
pub fn notify_dingtalk(markdown: &str, at_mobiles: &[String]) -> Result<()> {
    let webhook = match std::env::var("DINGTALK_WEBHOOK") {
        Ok(value) if !value.is_empty() => value,
        _ => {
            println!("DingTalk notification skipped: DINGTALK_WEBHOOK is not configured.");
            return Ok(());
        }
    };

    let text = truncate_for_dingtalk(markdown);

    let payload = json!({
        "msgtype": "markdown",
        "markdown": {
            "title": "GitHub 热门项目日报",
            "text": text,
        },
        "at": {
            "atMobiles": at_mobiles,
            "isAtAll": false,
        },
    });

    let client = reqwest::blocking::Client::new();
    let response = client
        .post(&webhook)
        .json(&payload)
        .send()
        .context("request to DingTalk webhook failed")?;

    let result: serde_json::Value = response
        .json()
        .map_err(|err| anyhow!("DingTalk returned a non-JSON response: {err}"))?;

    let errcode = result.get("errcode").and_then(|v| v.as_i64()).unwrap_or(-1);
    if errcode != 0 {
        let errmsg = result
            .get("errmsg")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        bail!("DingTalk notification failed: errcode={errcode}, errmsg={errmsg}");
    }

    println!("DingTalk notification sent successfully.");
    Ok(())
}

/// Repository web URL, used to link the full report from the fallback footer.
const REPO_URL: &str = "https://github.com/AllenHu000/github-trending-monitor";

/// Truncate to the robot's character limit at a character boundary, appending a
/// pointer to the full report when content was cut.
fn truncate_for_dingtalk(text: &str) -> String {
    const FOOTER_TEMPLATE: &str =
        "\n\n> …（内容过长已截断，完整日报：[daily_new.md]({repo_url}/blob/main/daily_new.md)）";
    let footer = FOOTER_TEMPLATE.replace("{repo_url}", REPO_URL);
    if text.chars().count() <= MAX_TEXT_CHARS {
        return text.to_string();
    }
    let keep = MAX_TEXT_CHARS.saturating_sub(footer.chars().count());
    let mut cut: String = text.chars().take(keep).collect();
    cut.push_str(&footer);
    cut
}

#[cfg(test)]
mod tests {
    use super::truncate_for_dingtalk;

    #[test]
    fn short_text_is_unchanged() {
        let text = "# 日报\n- item";
        assert_eq!(truncate_for_dingtalk(text), text);
    }

    #[test]
    fn long_text_is_cut_within_limit_and_keeps_footer() {
        let text = "a".repeat(6000);
        let out = truncate_for_dingtalk(&text);
        assert!(out.chars().count() <= super::MAX_TEXT_CHARS);
        assert!(out.contains("daily_new.md"));
        assert!(out.contains("blob/main/daily_new.md"));
    }
}
