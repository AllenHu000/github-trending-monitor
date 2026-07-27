use anyhow::{anyhow, bail, Context, Result};
use serde_json::json;

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

    let payload = json!({
        "msgtype": "markdown",
        "markdown": {
            "title": "GitHub 热门项目日报",
            "text": markdown,
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
