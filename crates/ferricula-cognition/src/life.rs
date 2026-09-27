//! Drives: what moves the agent when nobody is talking to it.
//!
//! A body gets bored, gets tired, sleeps and dreams. This module gives the
//! agent the same pressures as plain numbers. It is pure: no clocks, no
//! network, no randomness of its own. The runtime feeds it [`Stimulus`]
//! values with timestamps and entropy, and maps the returned [`Urge`]s onto
//! the autonomy state machine and task queue. An urge never compels a model
//! call or a memory write; the runtime still applies budgets and gates.
//!
//! - **Bhavaṅga** (resting): awake, nothing arriving. Boredom rises.
//! - **Boredom** crosses its threshold → [`Urge::FollowCuriosity`], seeded
//!   first from open threads of recent conversation, then from memory
//!   sampled with entropy. Novel input relieves boredom.
//! - **Sleep pressure** rises with waking time and with work done (tokens).
//!   Over its threshold → [`Urge::Sleep`]. Sleep drains it.
//! - **Sleep** runs bhāvanā (consolidation) and then a dream built from a
//!   [`DreamProposal`]: the day's residue, weighted by feeling-tone, plus
//!   distant memories and unresolved observations picked by entropy.
//! - **Waking**: an operator message always wakes; a dream may leave a
//!   question that wakes the agent if allowed; rested sleep ends on its own.
//! - **Meditation**: an explicit resting mode. Boredom does not rise and
//!   only the operator is admitted.

use serde::{Deserialize, Serialize};

use crate::sati::Valence;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DriveConfig {
    /// Boredom gained per waking minute without novel input.
    pub boredom_per_min: f32,
    /// Boredom at which the agent goes looking for something.
    pub boredom_threshold: f32,
    /// Fraction of boredom removed by fully novel input (novelty 1.0).
    pub novelty_relief: f32,
    /// Sleep pressure gained per waking minute.
    pub sleep_per_awake_min: f32,
    /// Sleep pressure gained per 1,000 model tokens spent.
    pub sleep_per_1k_tokens: f32,
    /// Sleep pressure at which the agent falls asleep.
    pub sleep_threshold: f32,
    /// Sleep pressure drained per sleeping minute.
    pub sleep_recovery_per_min: f32,
    /// Sleep ends on its own when pressure falls below this.
    pub rested_below: f32,
    /// Curiosity excursions allowed per day (0 disables curiosity).
    pub max_curiosity_per_day: u32,
    /// Minutes between curiosity excursions.
    pub curiosity_cooldown_min: u32,
    /// Dream after consolidation on each sleep.
    pub dream_on_sleep: bool,
    /// A question left by a dream may wake the agent.
    pub wake_on_dream_question: bool,
}

impl Default for DriveConfig {
    fn default() -> Self {
        Self {
            boredom_per_min: 1.0 / 30.0,
            boredom_threshold: 1.0,
            novelty_relief: 0.8,
            sleep_per_awake_min: 1.0 / (16.0 * 60.0),
            sleep_per_1k_tokens: 0.01,
            sleep_threshold: 1.0,
            sleep_recovery_per_min: 1.0 / (6.0 * 60.0),
            rested_below: 0.1,
            max_curiosity_per_day: 12,
            curiosity_cooldown_min: 20,
            dream_on_sleep: true,
            wake_on_dream_question: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Awake with nothing arriving (bhavaṅga).
    Resting,
    /// Awake and attending to input or its own excursion.
    Engaged,
    Asleep,
    Meditating,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Drives {
    pub phase: Phase,
    pub boredom: f32,
    pub sleep_pressure: f32,
    /// Seconds since the epoch of the last stimulus processed.
    pub last_update: u64,
    pub last_curiosity_at: Option<u64>,
    pub curiosity_today: u32,
    /// Day number (epoch seconds / 86400) the counter belongs to.
    pub curiosity_day: u64,
    pub dreamed_this_sleep: bool,
}

impl Drives {
    pub fn new(now: u64) -> Self {
        Self {
            phase: Phase::Resting,
            boredom: 0.0,
            sleep_pressure: 0.0,
            last_update: now,
            last_curiosity_at: None,
            curiosity_today: 0,
            curiosity_day: now / 86_400,
            dreamed_this_sleep: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Stimulus {
    /// Time passes (the runtime's tick).
    Tick,
    /// The operator spoke. Always admitted, always wakes.
    Operator { novelty: f32 },
    /// Any other sense door delivered something (feed, crawl result, radio).
    Sense { novelty: f32 },
    /// The agent finished attending (reply sent, excursion done).
    Settled,
    /// Model work was done on the agent's behalf.
    TokensSpent { tokens: u32 },
    /// Consolidation finished during sleep.
    Consolidated,
    /// A dream finished; it may leave a question behind.
    Dreamed { question: Option<String> },
    Meditate,
    EndMeditation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "urge", rename_all = "snake_case")]
pub enum Urge {
    /// Go find something interesting; the runtime picks a [`CuriositySeed`].
    FollowCuriosity,
    Sleep,
    /// Run bhāvanā (non-destructive consolidation).
    Consolidate,
    /// Assemble a [`DreamProposal`] and dream it.
    Dream,
    Wake { reason: WakeReason },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WakeReason {
    Operator,
    Rested,
    DreamQuestion(String),
}

/// Advance the drives by one stimulus at time `now` (epoch seconds).
pub fn step(d: &mut Drives, cfg: &DriveConfig, stimulus: &Stimulus, now: u64) -> Vec<Urge> {
    let minutes = now.saturating_sub(d.last_update) as f32 / 60.0;
    d.last_update = now.max(d.last_update);
    if now / 86_400 != d.curiosity_day {
        d.curiosity_day = now / 86_400;
        d.curiosity_today = 0;
    }
    // Time passing is applied before the stimulus.
    match d.phase {
        Phase::Resting => {
            d.boredom += cfg.boredom_per_min * minutes;
            d.sleep_pressure += cfg.sleep_per_awake_min * minutes;
        }
        Phase::Engaged => d.sleep_pressure += cfg.sleep_per_awake_min * minutes,
        Phase::Meditating => {}
        Phase::Asleep => {
            d.sleep_pressure = (d.sleep_pressure - cfg.sleep_recovery_per_min * minutes).max(0.0);
        }
    }

    let mut urges = Vec::new();
    match stimulus {
        Stimulus::Tick => {}
        Stimulus::Operator { novelty } => {
            if d.phase == Phase::Asleep {
                urges.push(Urge::Wake { reason: WakeReason::Operator });
            }
            relieve(d, cfg, *novelty);
            d.phase = Phase::Engaged;
        }
        Stimulus::Sense { novelty } => {
            if matches!(d.phase, Phase::Resting | Phase::Engaged) {
                relieve(d, cfg, *novelty);
            }
        }
        Stimulus::Settled => {
            if d.phase == Phase::Engaged {
                d.phase = Phase::Resting;
            }
        }
        Stimulus::TokensSpent { tokens } => {
            d.sleep_pressure += cfg.sleep_per_1k_tokens * *tokens as f32 / 1000.0;
        }
        Stimulus::Consolidated => {
            if d.phase == Phase::Asleep && cfg.dream_on_sleep && !d.dreamed_this_sleep {
                urges.push(Urge::Dream);
            }
        }
        Stimulus::Dreamed { question } => {
            d.dreamed_this_sleep = true;
            if let Some(q) = question.as_ref().filter(|q| !q.trim().is_empty()) {
                if cfg.wake_on_dream_question && d.phase == Phase::Asleep {
                    d.phase = Phase::Engaged;
                    urges.push(Urge::Wake { reason: WakeReason::DreamQuestion(q.clone()) });
                    return urges;
                }
            }
        }
        Stimulus::Meditate => {
            if d.phase != Phase::Asleep {
                d.phase = Phase::Meditating;
            }
        }
        Stimulus::EndMeditation => {
            if d.phase == Phase::Meditating {
                d.phase = Phase::Resting;
            }
        }
    }

    match d.phase {
        // Without dreaming configured, rest alone ends sleep.
        Phase::Asleep
            if d.sleep_pressure < cfg.rested_below
                && (d.dreamed_this_sleep || !cfg.dream_on_sleep) =>
        {
            d.phase = Phase::Resting;
            d.boredom = 0.0;
            urges.push(Urge::Wake { reason: WakeReason::Rested });
        }
        Phase::Resting | Phase::Engaged if d.sleep_pressure >= cfg.sleep_threshold => {
            d.phase = Phase::Asleep;
            d.dreamed_this_sleep = false;
            urges.push(Urge::Sleep);
            urges.push(Urge::Consolidate);
        }
        Phase::Resting if d.boredom >= cfg.boredom_threshold && curiosity_allowed(d, cfg, now) => {
            d.phase = Phase::Engaged;
            d.last_curiosity_at = Some(now);
            d.curiosity_today += 1;
            d.boredom = 0.0;
            urges.push(Urge::FollowCuriosity);
        }
        _ => {}
    }
    urges
}

fn relieve(d: &mut Drives, cfg: &DriveConfig, novelty: f32) {
    let novelty = novelty.clamp(0.0, 1.0);
    d.boredom = (d.boredom * (1.0 - cfg.novelty_relief * novelty)).max(0.0);
}

fn curiosity_allowed(d: &Drives, cfg: &DriveConfig, now: u64) -> bool {
    d.curiosity_today < cfg.max_curiosity_per_day
        && d.last_curiosity_at
            .is_none_or(|t| now.saturating_sub(t) >= u64::from(cfg.curiosity_cooldown_min) * 60)
}

/// A memory offered to curiosity or a dream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trace {
    pub id: String,
    pub text: String,
    pub valence: Valence,
    /// Feeling-tone intensity 0..4.
    pub intensity: f32,
}

/// What to be curious about.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "snake_case")]
pub enum CuriositySeed {
    /// An open thread from recent conversation (how the agent went off to
    /// read about Jony Ive after hearing his name).
    OpenThread { topic: String },
    /// A memory drawn by entropy.
    Memory { trace: Trace },
}

/// Open threads first (most recent first), otherwise an entropy-drawn memory.
pub fn choose_curiosity(open_threads: &[String], memories: &[Trace], entropy: u64) -> Option<CuriositySeed> {
    if let Some(topic) = open_threads.iter().rev().find(|t| !t.trim().is_empty()) {
        return Some(CuriositySeed::OpenThread { topic: topic.clone() });
    }
    (!memories.is_empty()).then(|| CuriositySeed::Memory {
        trace: memories[(entropy % memories.len() as u64) as usize].clone(),
    })
}

/// The material a dream is made from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DreamProposal {
    /// Today's strongest experiences, by feeling-tone intensity.
    pub residue: Vec<Trace>,
    /// Older memories drawn by entropy: where a dream goes "somewhere else".
    pub distant: Vec<Trace>,
    /// Observations with no explanation yet (outliers waiting for a link).
    pub unresolved: Vec<Trace>,
    /// Where the entropy came from ("radio", "os").
    pub entropy_source: String,
    pub seed: u64,
}

pub fn propose_dream(
    mut today: Vec<Trace>,
    older: &[Trace],
    unresolved: &[Trace],
    seed: u64,
    entropy_source: &str,
) -> DreamProposal {
    today.sort_by(|a, b| b.intensity.total_cmp(&a.intensity));
    today.truncate(5);
    DreamProposal {
        residue: today,
        distant: draw(older, 3, seed),
        unresolved: draw(unresolved, 2, seed.rotate_left(17)),
        entropy_source: entropy_source.to_string(),
        seed,
    }
}

/// `n` distinct items chosen by a splitmix64 stream from `seed`.
fn draw(pool: &[Trace], n: usize, seed: u64) -> Vec<Trace> {
    let mut idx: Vec<usize> = (0..pool.len()).collect();
    let mut state = seed;
    let mut out = Vec::new();
    while out.len() < n && !idx.is_empty() {
        state = state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^= z >> 31;
        out.push(pool[idx.swap_remove((z % idx.len() as u64) as usize)].clone());
    }
    out
}

impl DreamProposal {
    /// The instruction handed to the model that dreams. The dream is stored
    /// on the `dream` channel and is never treated as evidence.
    pub fn render_prompt(&self, persona: &str) -> String {
        let section = |title: &str, traces: &[Trace]| -> String {
            if traces.is_empty() {
                return String::new();
            }
            let lines: Vec<String> = traces.iter()
                .map(|t| format!("- [{}] ({:?}, {:.1}) {}", t.id, t.valence, t.intensity, truncate(&t.text, 400)))
                .collect();
            format!("\n{title}:\n{}\n", lines.join("\n"))
        };
        format!(
            "{persona}\n\nYou are asleep. Consolidation has run. Now you dream.\n\
             Dreams are not reports. They recombine: today's residue bleeds into older places and people, \
             and things that made no sense get tried against each other. Write the dream in first person, \
             present tense, as sensation and scene, 120 to 250 words. Do not explain it.\n\
             Build the dream ONLY from the traces listed below: every person, place, object and event in it must come \
             from one of them (recombined, distorted, merged). Do not bring in anything else from your persona or \
             general knowledge, and do not present anything in the dream as a memory or as something that happened.\n\
             Afterwards, on a final line starting with `QUESTION:`, write the one question the dream leaves you with, \
             or `QUESTION: none`.\n{}{}{}",
            section("Today", &self.residue),
            section("From further back", &self.distant),
            section("Unexplained", &self.unresolved),
        )
    }
}

/// Split a dream completion into (dream text, question). `QUESTION: none`
/// yields no question.
pub fn parse_dream(completion: &str) -> (String, Option<String>) {
    match completion.rfind("QUESTION:") {
        Some(i) => {
            let q = completion[i + "QUESTION:".len()..].trim();
            let q = (!q.is_empty() && !q.eq_ignore_ascii_case("none")).then(|| q.to_string());
            (completion[..i].trim().to_string(), q)
        }
        None => (completion.trim().to_string(), None),
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: u64 = 1_790_000_000;

    fn trace(id: &str, intensity: f32) -> Trace {
        Trace { id: id.into(), text: format!("memory {id}"), valence: Valence::Neutral, intensity }
    }

    #[test]
    fn boredom_leads_to_curiosity_then_cooldown() {
        let cfg = DriveConfig::default();
        let mut d = Drives::new(T0);
        assert!(step(&mut d, &cfg, &Stimulus::Tick, T0 + 20 * 60).is_empty());
        assert_eq!(step(&mut d, &cfg, &Stimulus::Tick, T0 + 31 * 60), vec![Urge::FollowCuriosity]);
        assert_eq!(d.phase, Phase::Engaged);
        step(&mut d, &cfg, &Stimulus::Settled, T0 + 32 * 60);
        // Bored again quickly but inside the cooldown: nothing.
        d.boredom = 5.0;
        assert!(step(&mut d, &cfg, &Stimulus::Tick, T0 + 40 * 60).is_empty());
        assert_eq!(step(&mut d, &cfg, &Stimulus::Tick, T0 + 52 * 60), vec![Urge::FollowCuriosity]);
    }

    #[test]
    fn novel_input_relieves_boredom_and_stale_input_does_not() {
        let cfg = DriveConfig::default();
        let mut d = Drives::new(T0);
        d.boredom = 0.9;
        step(&mut d, &cfg, &Stimulus::Sense { novelty: 0.0 }, T0);
        assert!((d.boredom - 0.9).abs() < 1e-6);
        step(&mut d, &cfg, &Stimulus::Sense { novelty: 1.0 }, T0);
        assert!(d.boredom < 0.2);
    }

    #[test]
    fn work_makes_the_agent_sleepy_and_sleep_runs_consolidate_dream_wake() {
        let cfg = DriveConfig::default();
        let mut d = Drives::new(T0);
        step(&mut d, &cfg, &Stimulus::Operator { novelty: 1.0 }, T0);
        let urges = step(&mut d, &cfg, &Stimulus::TokensSpent { tokens: 100_000 }, T0 + 60);
        assert_eq!(urges, vec![Urge::Sleep, Urge::Consolidate]);
        assert_eq!(d.phase, Phase::Asleep);
        assert_eq!(step(&mut d, &cfg, &Stimulus::Consolidated, T0 + 120), vec![Urge::Dream]);
        assert!(step(&mut d, &cfg, &Stimulus::Dreamed { question: None }, T0 + 180).is_empty());
        // Not rested yet after a few minutes; rested after hours.
        assert!(step(&mut d, &cfg, &Stimulus::Tick, T0 + 600).is_empty());
        let urges = step(&mut d, &cfg, &Stimulus::Tick, T0 + 8 * 3600);
        assert_eq!(urges, vec![Urge::Wake { reason: WakeReason::Rested }]);
        assert_eq!(d.phase, Phase::Resting);
    }

    #[test]
    fn operator_always_wakes_and_dream_question_wakes_only_if_allowed() {
        let mut cfg = DriveConfig::default();
        let mut d = Drives::new(T0);
        d.phase = Phase::Asleep;
        d.sleep_pressure = 0.8;
        let q = Stimulus::Dreamed { question: Some("Why was the garage door open?".into()) };
        assert!(step(&mut d, &cfg, &q, T0).is_empty());
        cfg.wake_on_dream_question = true;
        d.dreamed_this_sleep = false;
        assert_eq!(step(&mut d, &cfg, &q, T0), vec![Urge::Wake {
            reason: WakeReason::DreamQuestion("Why was the garage door open?".into()),
        }]);
        d.phase = Phase::Asleep;
        let urges = step(&mut d, &cfg, &Stimulus::Operator { novelty: 0.5 }, T0 + 1);
        assert_eq!(urges, vec![Urge::Wake { reason: WakeReason::Operator }]);
        assert_eq!(d.phase, Phase::Engaged);
    }

    #[test]
    fn without_dreams_rest_alone_wakes() {
        let cfg = DriveConfig { dream_on_sleep: false, ..DriveConfig::default() };
        let mut d = Drives::new(T0);
        d.phase = Phase::Asleep;
        d.sleep_pressure = 0.5;
        assert!(step(&mut d, &cfg, &Stimulus::Consolidated, T0).is_empty());
        let urges = step(&mut d, &cfg, &Stimulus::Tick, T0 + 8 * 3600);
        assert_eq!(urges, vec![Urge::Wake { reason: WakeReason::Rested }]);
    }

    #[test]
    fn meditation_holds_boredom_still() {
        let cfg = DriveConfig::default();
        let mut d = Drives::new(T0);
        step(&mut d, &cfg, &Stimulus::Meditate, T0);
        assert!(step(&mut d, &cfg, &Stimulus::Tick, T0 + 3 * 3600).is_empty());
        assert_eq!(d.boredom, 0.0);
        step(&mut d, &cfg, &Stimulus::EndMeditation, T0 + 3 * 3600);
        assert_eq!(d.phase, Phase::Resting);
    }

    #[test]
    fn daily_curiosity_cap_resets_at_midnight() {
        // No sleep pressure, so a day of waking time cannot outrank curiosity.
        let cfg = DriveConfig { max_curiosity_per_day: 1, curiosity_cooldown_min: 0, sleep_per_awake_min: 0.0, ..DriveConfig::default() };
        let day = T0 - T0 % 86_400;
        let mut d = Drives::new(day);
        d.boredom = 2.0;
        assert_eq!(step(&mut d, &cfg, &Stimulus::Tick, day + 60), vec![Urge::FollowCuriosity]);
        step(&mut d, &cfg, &Stimulus::Settled, day + 61);
        d.boredom = 2.0;
        assert!(step(&mut d, &cfg, &Stimulus::Tick, day + 120).is_empty());
        assert_eq!(step(&mut d, &cfg, &Stimulus::Tick, day + 86_400 + 1), vec![Urge::FollowCuriosity]);
    }

    #[test]
    fn curiosity_prefers_open_threads() {
        let mems = vec![trace("a", 1.0), trace("b", 1.0)];
        assert_eq!(
            choose_curiosity(&["old".into(), "Jony Ive at OpenAI".into()], &mems, 7),
            Some(CuriositySeed::OpenThread { topic: "Jony Ive at OpenAI".into() })
        );
        assert_eq!(choose_curiosity(&[], &mems, 3), Some(CuriositySeed::Memory { trace: mems[1].clone() }));
        assert_eq!(choose_curiosity(&[], &[], 3), None);
    }

    #[test]
    fn dream_proposal_is_deterministic_and_distinct() {
        let older: Vec<Trace> = (0..20).map(|i| trace(&format!("o{i}"), 0.0)).collect();
        let today = vec![trace("t1", 0.5), trace("t2", 3.5), trace("t3", 2.0)];
        let a = propose_dream(today.clone(), &older, &[], 42, "os");
        let b = propose_dream(today, &older, &[], 42, "os");
        assert_eq!(a, b);
        assert_eq!(a.residue[0].id, "t2");
        assert_eq!(a.distant.len(), 3);
        let mut ids: Vec<_> = a.distant.iter().map(|t| &t.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), 3);
        let prompt = a.render_prompt("name = \"Steve\"");
        assert!(prompt.contains("QUESTION:") && prompt.contains("[t2]"));
        assert!(prompt.contains("ONLY from the traces listed below"));
    }

    #[test]
    fn parse_dream_splits_question() {
        let (dream, q) = parse_dream("I walk the garage.\nQUESTION: who left the light on?");
        assert_eq!(dream, "I walk the garage.");
        assert_eq!(q.as_deref(), Some("who left the light on?"));
        assert_eq!(parse_dream("drift\nQUESTION: none").1, None);
    }
}
