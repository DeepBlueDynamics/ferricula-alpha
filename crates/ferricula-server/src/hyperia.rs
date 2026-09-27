//! Hyperia client (inbox/HYPERIA_COMMS.md). The agent authenticates with its
//! own `hyp_agent_` token from `HYPERIA_TOKEN`; the token is read at call
//! time and never logged.
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

pub const TOKEN_ENV: &str = "HYPERIA_TOKEN";

pub fn token() -> Option<String> {
    std::env::var(TOKEN_ENV).ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

/// `POST /api/tts`: speak `text` aloud on the Hyperia host. Blocks until
/// playback ends (callers run it off the request path). Returns Hyperia's
/// reply, whose `spoken` field is the exact transcript that was said.
pub fn speak(base_url: &str, token: &str, text: &str, voice: Option<&str>, speed: Option<f32>) -> Result<Value> {
    let mut body = json!({ "text": text });
    if let Some(voice) = voice {
        body["voice"] = json!(voice);
    }
    if let Some(speed) = speed {
        body["speed"] = json!(speed.clamp(0.5, 2.0));
    }
    let url = format!("{}/api/tts", base_url.trim_end_matches('/'));
    // Playback can wait up to 120 s for its turn in Hyperia's queue.
    let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(240)).build()?;
    let reply: Value = client.post(&url).bearer_auth(token).json(&body).send()
        .context("Hyperia unreachable")?
        .json().context("Hyperia reply was not JSON")?;
    if reply["ok"] != true {
        bail!("Hyperia refused the spoken summary: {}", reply["error"].as_str().unwrap_or("unknown error"));
    }
    Ok(reply)
}

/// Web panes open in Hyperia: `(pane id, name)` for every pane of kind `web`.
pub fn web_panes(base_url: &str, token: &str) -> Result<Vec<(String, String)>> {
    let url = format!("{}/api/status", base_url.trim_end_matches('/'));
    let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(10)).build()?;
    let status: Value = client.get(&url).bearer_auth(token).send().context("Hyperia unreachable")?
        .json().context("Hyperia status was not JSON")?;
    let mut panes = Vec::new();
    for window in status["windows"].as_array().into_iter().flatten() {
        for tab in window["tabs"].as_array().into_iter().flatten() {
            for pane in tab["panes"].as_array().into_iter().flatten() {
                if pane["kind"] == "web" {
                    if let Some(id) = pane["paneId"].as_str() {
                        let name = pane["title"].as_str().or(pane["name"].as_str()).unwrap_or("").to_string();
                        panes.push((id.to_string(), name));
                    }
                }
            }
        }
    }
    Ok(panes)
}

/// `POST /api/web-pane/content`: the pane's rendered page as markdown
/// (`{success, url, title, markdown}`). Needs the `web_nav` grant; an
/// ungranted caller gets 202 while Kord is asked, returned as an error.
pub fn web_pane_content(base_url: &str, token: &str, pane_id: &str) -> Result<Value> {
    let url = format!("{}/api/web-pane/content", base_url.trim_end_matches('/'));
    let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(30)).build()?;
    let resp = client.post(&url).query(&[("pane", pane_id)]).bearer_auth(token).send().context("Hyperia unreachable")?;
    if resp.status().as_u16() == 202 {
        bail!("Hyperia is asking Kord to approve web-pane access for this agent; try again after he approves");
    }
    let body: Value = resp.json().context("Hyperia reply was not JSON")?;
    if body["success"] != true {
        bail!("Hyperia could not read the pane: {}", body["error"].as_str().or(body["message"].as_str()).unwrap_or("unknown error"));
    }
    Ok(body)
}
