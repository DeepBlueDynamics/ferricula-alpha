//! Discord notices through a bot (`https://discord.com/api/v10`).
//!
//! The bot token comes only from `DISCORD_BOT_TOKEN` (delivered as
//! `DISCORD_BOT_TOKEN_FILE` by the container entrypoint, from a file the
//! operator writes outside the repository). It is never logged or returned.
//!
//! Today one notice exists: every web page the agent reads (`read_url`,
//! `ingest_url`, `read_web_pane`, and curiosity excursions) is posted to the
//! `reads_channel` room as a short card: title, address, how it was read and,
//! when kept, the agent's reason. Never the page text. Posting runs on its
//! own thread and never slows or fails a turn.
//!
//! Rooms are named in `[discord] channels = { random = "1234…" }`; a room
//! not listed is looked up by name across the bot's servers (first match,
//! cached). `GET /settings/discord/channels` lists every text channel the bot
//! can see, with ids, so the operator can fill in the map.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const TOKEN_ENV: &str = "DISCORD_BOT_TOKEN";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DiscordConfig {
    pub enabled: bool,
    pub api_base: String,
    /// Room name → channel id. Names may be written with or without `#`.
    pub channels: BTreeMap<String, String>,
    /// Where page reads are posted (a room name or a channel id); empty turns
    /// read notices off.
    pub reads_channel: String,
}

impl Default for DiscordConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            api_base: "https://discord.com/api/v10".into(),
            channels: BTreeMap::new(),
            reads_channel: "random".into(),
        }
    }
}

fn token() -> Option<String> {
    std::env::var(TOKEN_ENV).ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

/// Resolved room names, shared with the posting threads.
#[derive(Default)]
pub struct DiscordPlane {
    resolved: Arc<Mutex<HashMap<String, String>>>,
}

fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder().timeout(Duration::from_secs(10)).build()?)
}

fn get(cfg: &DiscordConfig, token: &str, path: &str) -> Result<Value> {
    let url = format!("{}/{}", cfg.api_base.trim_end_matches('/'), path.trim_start_matches('/'));
    let response = client()?.get(&url).header("Authorization", format!("Bot {token}")).send().context("Discord unreachable")?;
    let status = response.status();
    let text = response.text().unwrap_or_default();
    if !status.is_success() {
        bail!("Discord http {}: {}", status.as_u16(), text.replace(token, "<token>").chars().take(200).collect::<String>());
    }
    serde_json::from_str(&text).context("Discord reply was not JSON")
}

/// Every text channel the bot can see: (server, channel name, id).
pub fn list_channels(cfg: &DiscordConfig) -> Result<Vec<(String, String, String)>> {
    let Some(token) = token() else { bail!("no Discord bot token ({TOKEN_ENV} is not set)") };
    let guilds = get(cfg, &token, "users/@me/guilds")?;
    let mut out = Vec::new();
    for guild in guilds.as_array().cloned().unwrap_or_default() {
        let (Some(gid), gname) = (guild["id"].as_str(), guild["name"].as_str().unwrap_or("?")) else { continue };
        let channels = get(cfg, &token, &format!("guilds/{gid}/channels"))?;
        for c in channels.as_array().cloned().unwrap_or_default() {
            // 0 = text channel, 5 = announcement channel.
            if matches!(c["type"].as_u64(), Some(0 | 5)) {
                if let (Some(name), Some(id)) = (c["name"].as_str(), c["id"].as_str()) {
                    out.push((gname.to_string(), name.to_string(), id.to_string()));
                }
            }
        }
    }
    Ok(out)
}

fn resolve(cfg: &DiscordConfig, cache: &Mutex<HashMap<String, String>>, room: &str) -> Result<String> {
    let name = room.trim().trim_start_matches('#').to_lowercase();
    if !name.is_empty() && name.chars().all(|c| c.is_ascii_digit()) {
        return Ok(name);
    }
    if let Some(id) = cfg.channels.iter().find(|(k, _)| k.trim_start_matches('#').to_lowercase() == name).map(|(_, v)| v.clone()) {
        return Ok(id);
    }
    if let Some(id) = cache.lock().expect("discord poisoned").get(&name).cloned() {
        return Ok(id);
    }
    let found = list_channels(cfg)?.into_iter().find(|(_, n, _)| n.to_lowercase() == name)
        .map(|(_, _, id)| id).with_context(|| format!("no channel named #{name} is visible to the bot"))?;
    cache.lock().expect("discord poisoned").insert(name, found.clone());
    Ok(found)
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_string() } else { format!("{}…", s.chars().take(max - 1).collect::<String>()) }
}

impl crate::runtime::AgentRuntime {
    pub(crate) fn discord_available(&self) -> bool {
        self.config.discord.enabled && token().is_some()
    }

    /// Post a page-read card to the reads channel, on its own thread.
    /// `how`: "read_url", "ingest_url", "read_web_pane" or "curiosity".
    pub(crate) fn discord_page_read(&self, how: &str, title: &str, url: &str, note: Option<&str>) {
        if !self.discord_available() || self.config.discord.reads_channel.trim().is_empty() {
            return;
        }
        let cfg = self.config.discord.clone();
        let cache = self.discord.resolved.clone();
        let agent = self.persona().name.clone();
        let (how, title, url, note) = (how.to_string(), title.to_string(), url.to_string(), note.map(str::to_string));
        std::thread::spawn(move || {
            let result = (|| -> Result<()> {
                let token = token().context("no token")?;
                let channel = resolve(&cfg, &cache, &cfg.reads_channel)?;
                let verb = match how.as_str() {
                    "ingest_url" => "kept",
                    "curiosity" => "read on his own",
                    "read_web_pane" => "looked at a page open on the operator's screen",
                    _ => "read",
                };
                let mut fields = vec![json!({ "name": "How", "value": how, "inline": true })];
                if let Some(note) = note.as_deref().filter(|n| !n.trim().is_empty()) {
                    fields.push(json!({ "name": "Why", "value": clip(note, 900), "inline": false }));
                }
                let mut embed = json!({
                    "title": clip(if title.trim().is_empty() { &url } else { &title }, 240),
                    "description": format!("{agent} {verb}."),
                    "color": 2775208,
                    "fields": fields,
                });
                if url.starts_with("http://") || url.starts_with("https://") {
                    embed["url"] = json!(url);
                }
                let body = json!({ "allowed_mentions": { "parse": [] }, "embeds": [embed] });
                let endpoint = format!("{}/channels/{channel}/messages", cfg.api_base.trim_end_matches('/'));
                let response = client()?.post(&endpoint).header("Authorization", format!("Bot {token}")).json(&body).send()
                    .context("Discord unreachable")?;
                if !response.status().is_success() {
                    let status = response.status().as_u16();
                    let text = response.text().unwrap_or_default().replace(&token, "<token>");
                    bail!("Discord http {status}: {}", text.chars().take(200).collect::<String>());
                }
                Ok(())
            })();
            if let Err(error) = result {
                eprintln!("discord: page-read notice not posted: {error:#}");
            }
        });
    }
}
