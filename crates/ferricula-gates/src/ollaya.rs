//! Ollaya decision-model backends (`POST /api/decide`, default :11435).
//!
//! Ollaya serves classifiers, not generators: one forward pass returns
//! probabilities for typed questions (`choice`, `score`, `noul`). These are
//! the fast "unconscious" judgments of the cognitive process; the LLM is the
//! slow deliberate path, reached only when a judge says it is worth it or
//! abstains. Ollaya's probabilities are its own calibration; they are
//! flagged `calibrated` for lifecycle use only when a held-out measurement
//! authorized it (see [`Calibration`]).

use std::time::{Duration, Instant};

use ferricula_cognition::AgentId;
use ferricula_cognition::gates::{
    AbstainReason, Bounded, GateProvenance, Judged, MergeGate, MergeVerdict, SatiRecallGate,
    SatiRecallVerdict, TaskSucceededGate, TaskSucceededVerdict, UsageCost, VedanaGate,
    VedanaVerdict, Verdict, sha256_hex,
};
use ferricula_cognition::outcome::Evidence;
use ferricula_cognition::sati::Valence;
use serde_json::{Map, Value, json};

use crate::{Calibration, VERSION};

/// Question-set version; bump on any question or criteria text change.
pub const QUESTIONS_VERSION: &str = "ollaya-questions/2026-09-26.v1";

#[derive(Debug, Clone)]
pub struct OllayaClient {
    pub base_url: String,
    pub model: String,
    pub timeout_ms: u64,
}

impl OllayaClient {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self { base_url: base_url.into(), model: model.into(), timeout_ms: 10_000 }
    }

    /// Default local sidecar with the routing `laya` model.
    pub fn local() -> Self {
        Self::new("http://127.0.0.1:11435", "laya")
    }

    /// One `/api/decide` call. `questions` is the Ollaya questions object.
    pub fn decide(&self, state: &str, questions: Value) -> Result<Decision, AbstainReason> {
        let t0 = Instant::now();
        let agent = ureq::AgentBuilder::new().timeout(Duration::from_millis(self.timeout_ms)).build();
        let url = format!("{}/api/decide", self.base_url.trim_end_matches('/'));
        let res = agent.post(&url).send_json(json!({
            "model": self.model, "state": state, "questions": questions, "keep_alive": "30m",
        }));
        let latency_us = t0.elapsed().as_micros() as u64;
        let body: Value = match res {
            Ok(resp) => resp.into_json().map_err(|e| AbstainReason::InvalidOutput {
                message: format!("response json: {e}"),
            })?,
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                return Err(AbstainReason::ProviderError {
                    message: format!("http {code}: {}", text.chars().take(160).collect::<String>()),
                });
            }
            Err(e) => return Err(AbstainReason::ProviderError { message: format!("transport: {e}") }),
        };
        let answers = body.get("answers").and_then(Value::as_object).cloned().ok_or_else(|| {
            AbstainReason::InvalidOutput { message: "missing answers".into() }
        })?;
        Ok(Decision {
            answers,
            model: body.get("model").and_then(Value::as_str).unwrap_or(&self.model).to_string(),
            state_truncated: body.get("state_truncated").and_then(Value::as_bool).unwrap_or(false),
            input_tokens: body.pointer("/usage/input_tokens").and_then(Value::as_u64).unwrap_or(0) as u32,
            latency_us,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Decision {
    pub answers: Map<String, Value>,
    /// The model that actually answered (the router may pick e.g. `laya:en`).
    pub model: String,
    pub state_truncated: bool,
    pub input_tokens: u32,
    pub latency_us: u64,
}

impl Decision {
    pub fn noul(&self, id: &str) -> Option<f32> {
        self.answers.get(id)?.get("noul")?.as_f64().map(|p| p as f32)
    }

    pub fn score(&self, id: &str) -> Option<f32> {
        self.answers.get(id)?.get("score")?.as_f64().map(|s| s as f32)
    }

    /// (winning label, its probability)
    pub fn choice(&self, id: &str) -> Option<(String, f32)> {
        let answer = self.answers.get(id)?;
        let label = answer.get("choice")?.as_str()?.to_string();
        let p = answer.get("probabilities")?.get(&label)?.as_f64()? as f32;
        Some((label, p))
    }

    /// The full choice distribution in `labels` order (None if any is missing).
    pub fn choice_probs(&self, id: &str, labels: &[&str]) -> Option<Vec<f32>> {
        let probs = self.answers.get(id)?.get("probabilities")?;
        labels.iter().map(|l| probs.get(*l)?.as_f64().map(|p| p as f32)).collect()
    }
}

/// The vedanā gate's question set (valence choice + intensity score).
pub const VEDANA_LABELS: [&str; 3] = ["sukha", "dukkha", "adukkhamasukha"];

pub fn vedana_questions() -> Value {
    json!({
        "valence": choice("What feeling-tone does this experience carry?", &[
            ("sukha", "pleasant, welcome, satisfying"),
            ("dukkha", "unpleasant, painful, threatening"),
            ("adukkhamasukha", "neutral, neither pleasant nor unpleasant"),
        ]),
        "intensity": score("How strong is that feeling-tone?", &[
            "none", "faint", "moderate", "strong", "overwhelming",
        ]),
    })
}

/// The sati-recall gate's state and question set.
pub fn sati_recall_state(query: &str, memory: &str) -> String {
    format!("Query: {query}\n\nMemory: {memory}")
}

pub fn sati_recall_questions() -> Value {
    json!({
        "answers": noul("The memory contains information that answers the query.",
            "it answers or directly helps answer the query", "it does not help answer the query"),
        "relevance": score("How relevant is the memory to the query?", &[
            "unrelated", "same topic only", "partially answers", "fully answers",
        ]),
    })
}

/// One wording of a yes/no statement. `invert` means the statement is the
/// negation of the property of interest (p_property = 1 - p_statement).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YesNoWording<'a> {
    pub id: &'a str,
    pub statement: &'a str,
    pub when_true: &'a str,
    pub when_false: &'a str,
    pub invert: bool,
}

/// Candidate wordings of the curiosity gate ("is this worth researching?").
/// `cur-v0` is the wording the runtime used through the 2026-09-27 soak.
pub const CURIOSITY_WORDINGS: &[YesNoWording<'static>] = &[
    YesNoWording { id: "cur-v0", statement: "This is worth researching further", when_true: "yes", when_false: "no", invert: false },
    YesNoWording {
        id: "cur-v1",
        statement: "There is something here worth looking up on the web to learn more about.",
        when_true: "a topic, claim, name or question here that a web search would teach something about",
        when_false: "a greeting, small talk, an instruction or self-contained text with nothing to look up",
        invert: false,
    },
    YesNoWording { id: "cur-v2", statement: "This text raises a topic, claim or question that would be interesting to research.", when_true: "yes", when_false: "no", invert: false },
    YesNoWording {
        id: "cur-v3",
        statement: "This is small talk, a greeting, an instruction or routine text with nothing to look up.",
        when_true: "nothing here to look up", when_false: "there is a topic or question here worth looking up",
        invert: true,
    },
    YesNoWording {
        id: "cur-v4",
        statement: "Researching this further would teach something new and useful.",
        when_true: "a search would find new, useful information", when_false: "a search would add nothing",
        invert: false,
    },
];

/// The wording the curiosity gate should use (selected on the WP-4 dev split;
/// see research/gates/datasets/CARD-v3.md).
pub const CURIOSITY_WORDING: &str = "cur-v0";

pub fn curiosity_wording(id: &str) -> Option<&'static YesNoWording<'static>> {
    CURIOSITY_WORDINGS.iter().find(|w| w.id == id)
}

pub fn noul(instructions: &str, when_true: &str, when_false: &str) -> Value {
    json!({ "type": "noul", "instructions": instructions,
            "criteria": { "true": when_true, "false": when_false } })
}

pub fn score(instructions: &str, levels: &[&str]) -> Value {
    json!({ "type": "score", "instructions": instructions, "criteria": levels })
}

pub fn choice(instructions: &str, labels: &[(&str, &str)]) -> Value {
    let criteria: Map<String, Value> = labels.iter()
        .map(|(label, description)| (label.to_string(), Value::String(description.to_string())))
        .collect();
    json!({ "type": "choice", "instructions": instructions, "criteria": criteria })
}

/// One Ollaya-backed gate: client, gate name and optional calibration.
#[derive(Debug, Clone)]
pub struct OllayaGate {
    pub client: OllayaClient,
    pub gate: &'static str,
    pub calibration: Option<Calibration>,
}

impl OllayaGate {
    pub fn new(client: OllayaClient, gate: &'static str) -> Self {
        Self { client, gate, calibration: None }
    }

    fn provenance(&self, features: &[&str], decision: Option<&Decision>) -> GateProvenance {
        let model = decision.map(|d| d.model.as_str()).unwrap_or(&self.client.model);
        GateProvenance {
            gate: self.gate.to_string(),
            gate_version: format!("{VERSION}|{QUESTIONS_VERSION}|ollaya:{model}"),
            calibration_sha256: self.calibration.as_ref().map(|c| c.sha256.clone()),
            calibrated: self.calibration.as_ref().is_some_and(|c| c.lifecycle_authorized),
            features_sha256: sha256_hex(features),
            latency_us: decision.map(|d| d.latency_us).unwrap_or(0),
            state_truncated: decision.is_some_and(|d| d.state_truncated),
            cost: decision.map(|d| UsageCost {
                input_tokens: d.input_tokens,
                output_tokens: 0,
                usd: Some(0.0),
                profile: format!("local:ollaya:{model}"),
            }),
        }
    }

    /// Run a decision and map it to a verdict; any missing field abstains.
    fn run<T>(
        &self,
        features: &[&str],
        state: &str,
        questions: Value,
        map: impl FnOnce(&Decision) -> Result<T, AbstainReason>,
    ) -> Judged<T> {
        match self.client.decide(state, questions) {
            Err(reason) => Judged { verdict: Verdict::Abstain(reason), provenance: self.provenance(features, None) },
            Ok(decision) => {
                let verdict = if decision.state_truncated {
                    Verdict::Abstain(AbstainReason::StateTruncated)
                } else {
                    match map(&decision) {
                        Ok(answer) => Verdict::Answer(answer),
                        Err(reason) => Verdict::Abstain(reason),
                    }
                };
                Judged { verdict, provenance: self.provenance(features, Some(&decision)) }
            }
        }
    }

    /// A single yes/no judgment. This is the determining moment (voṭṭhapana):
    /// e.g. "is this worth deliberating on?" Low confidence abstains, and an
    /// abstention is never read as "no".
    pub fn yes_no(&self, state: &str, statement: &str, min_confidence: f32) -> Judged<YesNo> {
        let wording = YesNoWording { id: "", statement, when_true: "yes", when_false: "no", invert: false };
        self.yes_no_worded(state, &wording, min_confidence)
    }

    /// [`Self::yes_no`] with explicit criteria. The returned `p` is the
    /// probability of the property of interest (inverted for an `invert`
    /// wording), after this gate's calibration temperature if any.
    pub fn yes_no_worded(&self, state: &str, w: &YesNoWording<'_>, min_confidence: f32) -> Judged<YesNo> {
        // Plain yes/no criteria keep the pre-wording feature hash.
        let plain = w.when_true == "yes" && w.when_false == "no" && !w.invert;
        let features: Vec<&str> = if plain {
            vec![QUESTIONS_VERSION, w.statement, state]
        } else {
            vec![QUESTIONS_VERSION, w.statement, w.when_true, w.when_false, if w.invert { "invert" } else { "" }, state]
        };
        let calibration = self.calibration.clone();
        self.run(&features, state, json!({ "q": noul(w.statement, w.when_true, w.when_false) }), |d| {
            let raw = d.noul("q").ok_or_else(|| missing("q"))?;
            let raw = if w.invert { 1.0 - raw } else { raw };
            let p = calibration.as_ref().map_or(raw, |c| c.apply_binary(raw));
            let confidence = p.max(1.0 - p);
            if confidence < min_confidence {
                return Err(AbstainReason::LowConfidence { p_max: confidence });
            }
            Ok(YesNo { p })
        })
    }
}

/// Probability that a yes/no statement holds.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct YesNo {
    pub p: f32,
}

impl YesNo {
    pub fn yes(&self) -> bool {
        self.p >= 0.5
    }
}

impl Bounded for YesNo {
    fn bounded_fields(&self) -> Vec<(&'static str, f32, f32, f32)> {
        vec![("p", self.p, 0.0, 1.0)]
    }
}

fn missing(id: &str) -> AbstainReason {
    AbstainReason::InvalidOutput { message: format!("answer {id} missing") }
}

impl VedanaGate for OllayaGate {
    fn judge(&self, _agent: &AgentId, text: &str) -> Judged<VedanaVerdict> {
        let features = [QUESTIONS_VERSION, "vedana", text];
        let calibration = self.calibration.clone();
        self.run(&features, text, vedana_questions(), |d| vedana_verdict(d, calibration.as_ref()))
    }
}

/// Map a vedanā decision to a verdict: temper the full valence distribution
/// when a calibration is loaded, then abstain below p = 0.5.
pub fn vedana_verdict(d: &Decision, calibration: Option<&Calibration>) -> Result<VedanaVerdict, AbstainReason> {
    let raw = match d.choice_probs("valence", &VEDANA_LABELS) {
        Some(raw) => raw,
        // Distribution incomplete: only the winner is usable, uncalibrated.
        None if calibration.is_none() => {
            let (label, p) = d.choice("valence").ok_or_else(|| missing("valence"))?;
            let k = VEDANA_LABELS.iter().position(|l| *l == label).ok_or_else(|| missing("valence"))?;
            let mut v = vec![0.0; 3];
            v[k] = p;
            v
        }
        None => return Err(missing("valence")),
    };
    let probs = calibration.map_or_else(|| raw.clone(), |c| c.apply(&raw));
    let intensity = d.score("intensity").ok_or_else(|| missing("intensity"))?;
    let (k, p) = probs.iter().cloned().enumerate()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .ok_or_else(|| missing("valence"))?;
    if p < 0.5 {
        return Err(AbstainReason::LowConfidence { p_max: p });
    }
    let valence = match VEDANA_LABELS[k] {
        "sukha" => Valence::Sukha,
        "dukkha" => Valence::Dukkha,
        _ => Valence::Neutral,
    };
    Ok(VedanaVerdict { valence, intensity: intensity.clamp(0.0, 4.0), p })
}

impl SatiRecallGate for OllayaGate {
    fn judge(&self, _agent: &AgentId, query: &str, memory: &str) -> Judged<SatiRecallVerdict> {
        let features = [QUESTIONS_VERSION, "sati-recall", query, memory];
        let state = sati_recall_state(query, memory);
        let calibration = self.calibration.clone();
        self.run(&features, &state, sati_recall_questions(), |d| {
            let raw = d.noul("answers").ok_or_else(|| missing("answers"))?;
            Ok(SatiRecallVerdict {
                answers_query: calibration.as_ref().map_or(raw, |c| c.apply_binary(raw)),
                relevance: d.score("relevance").ok_or_else(|| missing("relevance"))?.clamp(0.0, 3.0),
            })
        })
    }
}

impl MergeGate for OllayaGate {
    fn judge(&self, agent: &AgentId, members: &[&str]) -> Judged<MergeVerdict> {
        let mut features = vec![QUESTIONS_VERSION, "sankhara-merge", agent.as_str()];
        features.extend_from_slice(members);
        let state = members.iter().enumerate()
            .map(|(i, m)| format!("Memory {}: {m}", i + 1))
            .collect::<Vec<_>>().join("\n\n");
        let questions = json!({
            "same": noul("These memories describe the same underlying fact or event.",
                "same fact or event", "different facts or events"),
            "contradiction": noul("At least two of these memories contradict each other.",
                "they contradict", "they are consistent"),
        });
        let cal = |p: f32| self.calibration.as_ref().map_or(p, |c| c.apply_binary(p));
        self.run(&features, &state, questions, |d| Ok(MergeVerdict {
            same_truth: cal(d.noul("same").ok_or_else(|| missing("same"))?),
            contradiction: cal(d.noul("contradiction").ok_or_else(|| missing("contradiction"))?),
        }))
    }
}

impl TaskSucceededGate for OllayaGate {
    fn judge(&self, _agent: &AgentId, transcript: &str, _evidence: &[Evidence]) -> Judged<TaskSucceededVerdict> {
        let features = [QUESTIONS_VERSION, "task_succeeded", transcript];
        let questions = json!({
            "succeeded": noul("The task in this transcript was completed, and the completion is confirmed by external evidence (a tool result, test, or observation), not only by the agent's own claim.",
                "completed and externally confirmed", "not completed or only self-reported"),
        });
        let cal = |p: f32| self.calibration.as_ref().map_or(p, |c| c.apply_binary(p));
        self.run(&features, transcript, questions, |d| Ok(TaskSucceededVerdict {
            p: cal(d.noul("succeeded").ok_or_else(|| missing("succeeded"))?),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decision(answers: Value, truncated: bool) -> Decision {
        Decision {
            answers: answers.as_object().unwrap().clone(),
            model: "laya:en".into(),
            state_truncated: truncated,
            input_tokens: 10,
            latency_us: 5,
        }
    }

    #[test]
    fn decision_accessors_read_ollaya_shapes() {
        let d = decision(json!({
            "v": {"type": "choice", "choice": "sukha", "confidence": 0.7,
                  "probabilities": {"sukha": 0.8, "dukkha": 0.1, "adukkhamasukha": 0.1}},
            "i": {"type": "score", "score": 2.5, "confidence": 0.3, "probabilities": {}},
            "n": {"type": "noul", "noul": 0.91},
        }), false);
        assert_eq!(d.choice("v"), Some(("sukha".into(), 0.8)));
        assert_eq!(d.score("i"), Some(2.5));
        assert_eq!(d.noul("n"), Some(0.91));
        assert_eq!(d.noul("missing"), None);
    }

    #[test]
    fn vedana_verdict_tempers_then_abstains() {
        let d = decision(json!({
            "valence": {"type": "choice", "choice": "sukha",
                        "probabilities": {"sukha": 0.6, "dukkha": 0.3, "adukkhamasukha": 0.1}},
            "intensity": {"type": "score", "score": 2.0},
        }), false);
        let raw = vedana_verdict(&d, None).unwrap();
        assert!((raw.p - 0.6).abs() < 1e-6);
        let sharpen = Calibration { sha256: "x".into(), temperature: 0.5, lifecycle_authorized: false };
        let v = vedana_verdict(&d, Some(&sharpen)).unwrap();
        assert!((v.p - 0.36 / 0.46).abs() < 1e-5);
        assert_eq!(v.valence, Valence::Sukha);
        let flatten = Calibration { sha256: "x".into(), temperature: 3.0, lifecycle_authorized: false };
        assert!(matches!(vedana_verdict(&d, Some(&flatten)), Err(AbstainReason::LowConfidence { .. })));
    }

    #[test]
    fn curiosity_wordings_are_unique_and_default_exists() {
        let mut ids: Vec<&str> = CURIOSITY_WORDINGS.iter().map(|w| w.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), CURIOSITY_WORDINGS.len());
        assert!(curiosity_wording(CURIOSITY_WORDING).is_some());
        assert_eq!(curiosity_wording("cur-v0").unwrap().statement, "This is worth researching further");
    }

    #[test]
    fn unreachable_sidecar_abstains_with_provider_error() {
        let mut client = OllayaClient::new("http://127.0.0.1:9", "laya");
        client.timeout_ms = 300;
        let gate = OllayaGate::new(client, "sati-recall");
        let agent = AgentId::new("test-agent".to_string()).unwrap();
        let judged = SatiRecallGate::judge(&gate, &agent, "q", "m");
        assert!(matches!(judged.verdict, Verdict::Abstain(AbstainReason::ProviderError { .. })));
        assert!(!judged.provenance.calibrated);
        assert!(judged.validate().is_ok());
    }

    #[test]
    fn uncalibrated_answers_never_reach_the_lifecycle() {
        let gate = OllayaGate::new(OllayaClient::local(), "task_succeeded");
        let judged = Judged {
            verdict: Verdict::Answer(TaskSucceededVerdict { p: 0.99 }),
            provenance: gate.provenance(&["x"], Some(&decision(json!({}), false))),
        };
        assert_eq!(judged.answer().unwrap(), Some(&TaskSucceededVerdict { p: 0.99 }));
        assert_eq!(judged.calibrated_answer().unwrap(), None);
    }
}
