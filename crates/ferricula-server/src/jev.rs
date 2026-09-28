//! Hosted JEV (TypeSafe) as the second gate tier, next to the local Ollaya
//! sidecar (`inbox/JEV_PLAN.md`).
//!
//! The operator sets the key from the UI (`POST /settings/jev`). It is kept
//! in the writable state volume at `state_dir/secrets/typesafe_api_key`
//! (owner-only where the platform allows), falls back to the
//! `TYPESAFE_API_KEY` environment variable, and is never returned by any
//! endpoint, logged, or echoed in an error. Settings (enabled, mode,
//! privacy, model, budget) live in `state_dir/settings/jev.json`.
//!
//! Routing, per gate call ([`AgentRuntime::gate_decide`]):
//! - `backup` (default): Ollaya answers; JEV is asked only when Ollaya could
//!   not judge (sidecar down or error, or the state overflowed its window).
//! - `primary`: JEV answers; Ollaya is the fallback when JEV fails.
//!
//! JEV is a third party: states built from private conversation go to it
//! only with `private_context` on. Every route decision is reported so the
//! UI can show which tier judged and why.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result, bail};
use ferricula_cognition::gates::AbstainReason;
use ferricula_gates::ollaya::{Decision, OllayaClient};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const KEY_ENV: &str = "TYPESAFE_API_KEY";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JevMode {
    Backup,
    Primary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct JevSettings {
    pub enabled: bool,
    pub mode: JevMode,
    /// Whether states built from private conversation may be sent to JEV.
    pub private_context: bool,
    pub base_url: String,
    /// Pin a version for gates that will be calibrated; never `jev-latest`.
    pub model: String,
    pub timeout_ms: u64,
    pub max_calls_per_day: u32,
    /// Largest state sent to JEV (its context is far larger than Laya's).
    pub max_state_bytes: usize,
    /// When the UI last set the key (the key itself is never stored here).
    pub key_set_at: Option<u64>,
}

impl Default for JevSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: JevMode::Backup,
            private_context: false,
            base_url: "https://api.typesafe.ai".into(),
            model: "jev-1.13.0".into(),
            timeout_ms: 5_000,
            max_calls_per_day: 200,
            max_state_bytes: 60_000,
            key_set_at: None,
        }
    }
}

/// Operator update from the UI; absent fields are left unchanged.
#[derive(Debug, Default, Deserialize)]
pub struct JevUpdate {
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_key: bool,
    pub enabled: Option<bool>,
    pub mode: Option<JevMode>,
    pub private_context: Option<bool>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub max_calls_per_day: Option<u32>,
}

#[derive(Default)]
struct Usage {
    day: u64,
    calls: u32,
    usd: f64,
    last_ok_at: Option<u64>,
    last_error: Option<String>,
}

pub struct JevPlane {
    settings_path: PathBuf,
    key_path: PathBuf,
    settings: Mutex<JevSettings>,
    usage: Mutex<Usage>,
}

/// Why a gate call went where it did; journaled and streamed to the UI.
#[derive(Debug, Clone, Serialize)]
pub struct GateRoute {
    /// The tier whose answer is returned (`ollaya` or `jev`).
    pub tier: &'static str,
    /// Set when the answering tier was reached after the other failed.
    pub escalated_from: Option<&'static str>,
    pub escalation_reason: Option<String>,
    /// Why JEV was not consulted, when it was not.
    pub jev_skipped: Option<String>,
}

impl JevPlane {
    pub fn open(state_dir: &Path) -> Result<Self> {
        let settings_path = state_dir.join("settings").join("jev.json");
        let key_path = state_dir.join("secrets").join("typesafe_api_key");
        let settings = match fs::read_to_string(&settings_path) {
            Ok(text) => serde_json::from_str(&text).with_context(|| format!("parse {}", settings_path.display()))?,
            Err(_) => JevSettings::default(),
        };
        Ok(Self { settings_path, key_path, settings: Mutex::new(settings), usage: Mutex::new(Usage::default()) })
    }

    fn key(&self) -> Option<(String, &'static str)> {
        if let Ok(key) = fs::read_to_string(&self.key_path) {
            let key = key.trim().to_string();
            if !key.is_empty() {
                return Some((key, "ui"));
            }
        }
        std::env::var(KEY_ENV).ok().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).map(|k| (k, "env"))
    }

    pub fn settings(&self) -> JevSettings {
        self.settings.lock().expect("jev settings poisoned").clone()
    }

    /// Status for the UI. Never includes the key.
    pub fn status(&self) -> Value {
        let settings = self.settings();
        let usage = self.usage.lock().expect("jev usage poisoned");
        let today = crate::runtime::now() / 86_400;
        let key = self.key();
        json!({
            "enabled": settings.enabled,
            "mode": settings.mode,
            "private_context": settings.private_context,
            "base_url": settings.base_url,
            "model": settings.model,
            "timeout_ms": settings.timeout_ms,
            "max_calls_per_day": settings.max_calls_per_day,
            "key_set": key.is_some(),
            "key_source": key.map(|(_, source)| source),
            "key_set_at": settings.key_set_at,
            "calls_today": if usage.day == today { usage.calls } else { 0 },
            "usd_today": if usage.day == today { usage.usd } else { 0.0 },
            "last_ok_at": usage.last_ok_at,
            "last_error": usage.last_error,
            "active": settings.enabled && self.key().is_some(),
        })
    }

    pub fn update(&self, update: JevUpdate) -> Result<Value> {
        let mut settings = self.settings();
        if let Some(key) = update.api_key.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
            if key.len() > 512 || key.chars().any(char::is_whitespace) {
                bail!("that does not look like an API key");
            }
            write_private(&self.key_path, key)?;
            settings.key_set_at = Some(crate::runtime::now());
        } else if update.clear_key {
            match fs::remove_file(&self.key_path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e).context("remove the stored key"),
            }
            settings.key_set_at = None;
        }
        if let Some(v) = update.enabled { settings.enabled = v; }
        if let Some(v) = update.mode { settings.mode = v; }
        if let Some(v) = update.private_context { settings.private_context = v; }
        if let Some(v) = update.base_url {
            let v = v.trim().trim_end_matches('/').to_string();
            let loopback = v.starts_with("http://127.0.0.1") || v.starts_with("http://localhost");
            if !v.starts_with("https://") && !loopback {
                bail!("base_url must be https (plain http only for loopback)");
            }
            settings.base_url = v;
        }
        if let Some(v) = update.model {
            let v = v.trim().to_string();
            if v.is_empty() || v.len() > 80 { bail!("model must be 1 to 80 characters"); }
            settings.model = v;
        }
        if let Some(v) = update.max_calls_per_day { settings.max_calls_per_day = v.min(100_000); }
        if let Some(parent) = self.settings_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.settings_path, serde_json::to_vec_pretty(&settings)?)?;
        *self.settings.lock().expect("jev settings poisoned") = settings;
        Ok(self.status())
    }

    /// A JEV client for one call, or why none is available. `private`: the
    /// state comes from private conversation or memory.
    fn client(&self, private: bool, force: bool) -> Result<OllayaClient, String> {
        let settings = self.settings();
        if !settings.enabled && !force {
            return Err("disabled".into());
        }
        let Some((key, _)) = self.key() else { return Err("no key".into()) };
        if private && !settings.private_context {
            return Err("private state and private_context is off".into());
        }
        let today = crate::runtime::now() / 86_400;
        let usage = self.usage.lock().expect("jev usage poisoned");
        let calls_today = if usage.day == today { usage.calls } else { 0 };
        if calls_today >= settings.max_calls_per_day {
            return Err(format!("daily budget spent ({} calls)", settings.max_calls_per_day));
        }
        let mut client = OllayaClient::jev(settings.base_url, settings.model, key);
        client.timeout_ms = settings.timeout_ms;
        Ok(client)
    }

    fn call(&self, client: &OllayaClient, state: &str, questions: Value) -> Result<Decision, AbstainReason> {
        let max = self.settings().max_state_bytes;
        let state = crate::recall::truncate_bytes(state, max);
        let result = client.decide(state, questions);
        match &result {
            Ok(decision) => self.note(Ok(decision.usd)),
            Err(reason) => self.note(Err(format!("{reason:?}"))),
        }
        result
    }

    /// Count one JEV call against today's budget.
    fn note(&self, outcome: Result<Option<f64>, String>) {
        let today = crate::runtime::now() / 86_400;
        let mut usage = self.usage.lock().expect("jev usage poisoned");
        if usage.day != today {
            *usage = Usage { day: today, ..Usage::default() };
        }
        usage.calls += 1;
        match outcome {
            Ok(usd) => {
                usage.usd += usd.unwrap_or(0.0);
                usage.last_ok_at = Some(crate::runtime::now());
                usage.last_error = None;
            }
            Err(error) => usage.last_error = Some(error.chars().take(240).collect()),
        }
    }
}

/// Write `text` to `path` readable only by the owner (Unix) and synced.
fn write_private(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp).context("write the key")?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    fs::rename(&tmp, path).context("store the key")?;
    Ok(())
}

/// Whether an Ollaya outcome means "could not judge" (a capacity problem the
/// larger hosted model can fix), as opposed to an answer or a real abstention.
fn unable_to_judge(result: &Result<Decision, AbstainReason>) -> Option<String> {
    match result {
        Err(AbstainReason::ProviderError { message }) => Some(format!("ollaya error: {}", message.chars().take(120).collect::<String>())),
        Ok(decision) if decision.state_truncated => Some("state_truncated".into()),
        _ => None,
    }
}

impl crate::runtime::AgentRuntime {
    /// One gate decision through the tiers (see the module doc). Blocking.
    pub fn gate_decide(&self, ollaya: &OllayaClient, state: &str, questions: Value, private: bool)
        -> (Result<Decision, AbstainReason>, OllayaClient, GateRoute)
    {
        let jev = self.jev.client(private, false);
        let mode = self.jev.settings().mode;
        if let (JevMode::Primary, Ok(client)) = (mode, &jev) {
            let result = self.jev.call(client, state, questions.clone());
            if result.is_ok() {
                return (result, client.clone(), GateRoute { tier: "jev", escalated_from: None, escalation_reason: None, jev_skipped: None });
            }
            let reason = format!("jev failed: {:?}", result.err()).chars().take(200).collect::<String>();
            let fallback = ollaya.decide(state, questions);
            return (fallback, ollaya.clone(), GateRoute {
                tier: "ollaya", escalated_from: Some("jev"), escalation_reason: Some(reason), jev_skipped: None,
            });
        }
        let first = ollaya.decide(state, questions.clone());
        match (unable_to_judge(&first), jev) {
            (Some(reason), Ok(client)) => {
                let second = self.jev.call(&client, state, questions);
                if second.is_ok() {
                    (second, client, GateRoute { tier: "jev", escalated_from: Some("ollaya"), escalation_reason: Some(reason), jev_skipped: None })
                } else {
                    let note = format!("{reason}; jev also failed: {:?}", second.err()).chars().take(240).collect();
                    (first, ollaya.clone(), GateRoute { tier: "ollaya", escalated_from: None, escalation_reason: None, jev_skipped: Some(note) })
                }
            }
            (Some(reason), Err(skip)) => (first, ollaya.clone(), GateRoute {
                tier: "ollaya", escalated_from: None, escalation_reason: None,
                jev_skipped: Some(format!("{reason}; jev not consulted: {skip}")),
            }),
            (None, _) => (first, ollaya.clone(), GateRoute { tier: "ollaya", escalated_from: None, escalation_reason: None, jev_skipped: None }),
        }
    }

    /// A yes/no gate through the tiers, for callers built on
    /// [`OllayaGate`](ferricula_gates::ollaya::OllayaGate). Blocking.
    pub fn gate_yes_no(&self, gate: &'static str, ollaya: OllayaClient, state: &str, statement: &str,
        min_confidence: f32, private: bool) -> (ferricula_cognition::gates::Judged<ferricula_gates::ollaya::YesNo>, GateRoute)
    {
        use ferricula_cognition::gates::Verdict;
        use ferricula_gates::ollaya::OllayaGate;
        let jev = self.jev.client(private, false);
        let run_jev = |client: OllayaClient| {
            let judged = OllayaGate::new(client, gate).yes_no(state, statement, min_confidence);
            let failed = matches!(&judged.verdict, Verdict::Abstain(AbstainReason::ProviderError { .. } | AbstainReason::InvalidOutput { .. }));
            self.jev.note(if failed { Err(format!("{:?}", judged.verdict)) } else { Ok(judged.provenance.cost.as_ref().and_then(|c| c.usd)) });
            (judged, failed)
        };
        if let (JevMode::Primary, Ok(client)) = (self.jev.settings().mode, &jev) {
            let (judged, failed) = run_jev(client.clone());
            if !failed {
                return (judged, GateRoute { tier: "jev", escalated_from: None, escalation_reason: None, jev_skipped: None });
            }
            let fallback = OllayaGate::new(ollaya, gate).yes_no(state, statement, min_confidence);
            return (fallback, GateRoute { tier: "ollaya", escalated_from: Some("jev"),
                escalation_reason: Some(format!("jev failed: {:?}", judged.verdict).chars().take(200).collect()), jev_skipped: None });
        }
        let first = OllayaGate::new(ollaya, gate).yes_no(state, statement, min_confidence);
        let reason = match &first.verdict {
            Verdict::Abstain(AbstainReason::ProviderError { message }) => Some(format!("ollaya error: {}", message.chars().take(120).collect::<String>())),
            Verdict::Abstain(AbstainReason::StateTruncated) => Some("state_truncated".to_string()),
            _ => None,
        };
        match (reason, jev) {
            (Some(reason), Ok(client)) => {
                let (judged, failed) = run_jev(client);
                if failed {
                    (first, GateRoute { tier: "ollaya", escalated_from: None, escalation_reason: None,
                        jev_skipped: Some(format!("{reason}; jev also failed: {:?}", judged.verdict).chars().take(240).collect()) })
                } else {
                    (judged, GateRoute { tier: "jev", escalated_from: Some("ollaya"), escalation_reason: Some(reason), jev_skipped: None })
                }
            }
            (Some(reason), Err(skip)) => (first, GateRoute { tier: "ollaya", escalated_from: None, escalation_reason: None,
                jev_skipped: Some(format!("{reason}; jev not consulted: {skip}")) }),
            (None, _) => (first, GateRoute { tier: "ollaya", escalated_from: None, escalation_reason: None, jev_skipped: None }),
        }
    }

    /// Operator probe: one noul question on fixed, non-private text, sent
    /// to JEV even when it is disabled (the key must be set). Reports the
    /// answer, latency and cost so the UI can confirm the key works.
    pub fn jev_probe(&self) -> Value {
        let client = match self.jev.client(false, true) {
            Ok(client) => client,
            Err(reason) => return json!({ "ok": false, "error": reason }),
        };
        let questions = json!({ "q": ferricula_gates::ollaya::noul(
            "This sentence is about the weather.", "yes", "no") });
        match self.jev.call(&client, "It is raining hard in San Francisco this morning.", questions) {
            Ok(decision) => json!({
                "ok": true, "model": decision.model, "p_yes": decision.noul("q"),
                "latency_ms": decision.latency_us / 1000, "input_tokens": decision.input_tokens,
                "usd": decision.usd, "answers": decision.answers,
            }),
            Err(reason) => json!({ "ok": false, "error": format!("{reason:?}").chars().take(300).collect::<String>() }),
        }
    }

    pub fn jev_status(&self) -> Value {
        self.jev.status()
    }

    pub fn jev_update(&self, update: JevUpdate) -> Result<Value> {
        self.jev.update(update)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("jev-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn key_is_stored_privately_and_never_reported() {
        let d = dir();
        let plane = JevPlane::open(&d).unwrap();
        let status = plane.update(JevUpdate { api_key: Some("ts-secret-123".into()), enabled: Some(true), ..Default::default() }).unwrap();
        assert_eq!(status["key_set"], true);
        assert_eq!(status["key_source"], "ui");
        assert!(!status.to_string().contains("ts-secret-123"));
        assert_eq!(fs::read_to_string(d.join("secrets/typesafe_api_key")).unwrap(), "ts-secret-123");
        assert!(!fs::read_to_string(d.join("settings/jev.json")).unwrap().contains("ts-secret-123"));
        // Survives a restart; clearing removes it.
        let plane = JevPlane::open(&d).unwrap();
        assert_eq!(plane.status()["enabled"], true);
        let status = plane.update(JevUpdate { clear_key: true, ..Default::default() }).unwrap();
        assert_eq!(status["key_set"], std::env::var(KEY_ENV).is_ok());
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn private_state_needs_private_context_and_budget_is_enforced() {
        let d = dir();
        let plane = JevPlane::open(&d).unwrap();
        plane.update(JevUpdate { api_key: Some("k".into()), enabled: Some(true), max_calls_per_day: Some(0), ..Default::default() }).unwrap();
        assert!(plane.client(true, false).unwrap_err().contains("private_context"));
        assert!(plane.client(false, false).unwrap_err().contains("budget"));
        plane.update(JevUpdate { private_context: Some(true), max_calls_per_day: Some(5), ..Default::default() }).unwrap();
        let client = plane.client(true, false).unwrap();
        assert!(!format!("{client:?}").contains("\"k\""), "debug output hides the key");
        assert!(plane.update(JevUpdate { base_url: Some("http://x".into()), ..Default::default() }).is_err());
        let _ = fs::remove_dir_all(d);
    }
}
