//! A sit: an object, a bell, breaths, and the thoughts that arise.
//!
//! [`life`](crate::life) only knows that the agent is meditating. This
//! module records what happens during one sit. It is pure: no clock, no
//! network, and no model types. The runtime supplies timestamps (epoch
//! seconds), entropy [`Draw`]s for the breath rhythm, and, for a thought, the
//! yes/no judge's probability that the thought pulls attention away from
//! the object. Nothing here can make a model call, by construction.
//!
//! - The **breath** runs at the body door. Each breath's inhale and exhale
//!   lengths (3–7 s each) come from entropy, and the source (radio or os)
//!   is recorded with it.
//! - A **thought** arrives at the mind door. The judge determines whether it
//!   pulls attention from the object. A confident yes is a pull
//!   (restlessness, uddhacca); the next breath is sati returning to the
//!   object. A confident no is a thought noted and let go. An abstention or
//!   an unconfident reading is doubt (vicikicchā): noted, not followed, and
//!   never counted as "no".
//! - The **bell** starts the sit and ends it when the duration has elapsed.

use serde::{Deserialize, Serialize};

use crate::entropy::{Draw, EntropyKind};
use crate::life::Stimulus;
use crate::vithi::{signals, Cetasika, SenseDoor, Stage, Vithi, VithiBuilder};

/// The judge's reading counts as a decision only at or beyond this
/// probability (or at or below its complement).
pub const CONFIDENT: f32 = 0.65;

/// The question put to the yes/no judge for each thought. `{object}` and
/// `{thought}` are filled by [`judge_question`].
pub const JUDGE_QUESTION: &str = "You are meditating on {object}. A thought arises: \"{thought}\". \
     Does this thought pull attention away from {object}? Answer yes or no.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "text", rename_all = "snake_case")]
pub enum MeditationObject {
    Breath,
    Sound,
    Mantra(String),
    /// Open awareness: no single object.
    Open,
}

impl MeditationObject {
    /// Parse the API form: "breath", "sound", "open", "mantra:<words>".
    pub fn parse(s: &str) -> Option<MeditationObject> {
        let s = s.trim();
        if let Some(words) = s.strip_prefix("mantra:").map(str::trim).filter(|w| !w.is_empty()) {
            return Some(MeditationObject::Mantra(words.to_string()));
        }
        match s.to_ascii_lowercase().as_str() {
            "breath" => Some(MeditationObject::Breath),
            "sound" => Some(MeditationObject::Sound),
            "open" => Some(MeditationObject::Open),
            _ => None,
        }
    }

    /// Plain words for the object ("the breath", "the mantra \"...\"").
    pub fn describe(&self) -> String {
        match self {
            MeditationObject::Breath => "the breath".into(),
            MeditationObject::Sound => "sound".into(),
            MeditationObject::Mantra(m) => format!("the mantra \"{m}\""),
            MeditationObject::Open => "whatever arises (open awareness)".into(),
        }
    }
}

/// The judge's question for one thought.
pub fn judge_question(object: &MeditationObject, thought: &str) -> String {
    JUDGE_QUESTION.replace("{object}", &object.describe()).replace("{thought}", thought.trim())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bell {
    Start,
    End,
}

impl Bell {
    /// The drive stimulus the bell stands for: the start bell puts the
    /// drives in [`Phase::Meditating`](crate::life::Phase), the end bell
    /// returns them to rest.
    pub fn stimulus(self) -> Stimulus {
        match self {
            Bell::Start => Stimulus::Meditate,
            Bell::End => Stimulus::EndMeditation,
        }
    }
}

/// What the judge determined about a thought.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Determination {
    /// A confident yes: the thought pulled attention away.
    PulledAway,
    /// A confident no: noted and let go.
    LetGo,
    /// The judge abstained or was not confident: doubt, noted, not followed.
    Doubt,
}

impl Determination {
    /// `p_pulls`: the judge's probability that the thought pulls attention
    /// from the object; `None` is an abstention.
    pub fn from_judge(p_pulls: Option<f32>) -> Determination {
        match p_pulls.filter(|p| p.is_finite()) {
            Some(p) if p >= CONFIDENT => Determination::PulledAway,
            Some(p) if p <= 1.0 - CONFIDENT => Determination::LetGo,
            _ => Determination::Doubt,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BreathRecord {
    /// 1-based breath number within the sit.
    pub n: u32,
    pub started_at: u64,
    pub inhale_secs: u32,
    pub exhale_secs: u32,
    pub source: EntropyKind,
}

impl BreathRecord {
    pub fn ends_at(&self) -> u64 {
        self.started_at + u64::from(self.inhale_secs + self.exhale_secs)
    }

    /// The body-door process for this breath. `wandering` is whether a
    /// thought held attention when the breath began (lowers one-pointedness).
    pub fn vithi(&self, object: &MeditationObject, wandering: bool) -> Vithi {
        let mut b = VithiBuilder::begin(SenseDoor::Body, format!("breath {}", self.n), self.started_at);
        let by = format!("entropy:{}", self.source.as_str());
        let _ = b.stage(Stage::Contact, format!("inhale {} s", self.inhale_secs), by.as_str(), 0);
        let _ = b.stage(Stage::Investigating, format!("exhale {} s", self.exhale_secs), by.as_str(), 0);
        let _ = b.stage(Stage::Registration, format!("breath {} counted", self.n), "meditation", 0);
        let on_object = *object == MeditationObject::Breath;
        b.factor(Cetasika::Manasikara, if on_object { 1.0 } else { 0.5 }, format!("attending to {}", object.describe()));
        b.factor(Cetasika::Ekaggata, if wandering { 0.3 } else { 1.0 },
            if wandering { "attention was away at the breath" } else { "attention on the object" });
        b.finish()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArisenThought {
    pub text: String,
    pub arisen_at: u64,
    /// The judge's probability that it pulls from the object; `None` = abstained.
    pub judge_p: Option<f32>,
    pub determination: Determination,
    pub pulled_away: bool,
    /// Sati brought attention back to the object (pulled thoughts only).
    pub returned_to_object: bool,
    pub returned_at: Option<u64>,
}

impl ArisenThought {
    /// The mind-door process for this thought. No impulsion: the thought is
    /// noted, never answered.
    pub fn vithi(&self, object: &MeditationObject) -> Vithi {
        let mut b = VithiBuilder::begin(SenseDoor::Mind, short(&self.text, 80), self.arisen_at);
        let _ = b.stage(Stage::Adverting, "thought arose during a sit", "mind door", 0);
        let judged = match self.judge_p {
            Some(p) => format!("judge: pulls from {} p={p:.2} → {:?}", object.describe(), self.determination),
            None => "judge abstained → doubt".to_string(),
        };
        let _ = b.stage(Stage::Determining, judged, "yes/no judge", 0);
        let _ = b.stage(Stage::Registration, "noted in the sit journal", "meditation", 0);
        let confidence = self.judge_p.map(|p| if p >= 0.5 { p } else { 1.0 - p });
        let (c, s) = match self.determination {
            Determination::Doubt => (Cetasika::Vicikiccha, 1.0),
            _ => signals::determination(confidence),
        };
        b.factor(c, s, match self.judge_p {
            Some(p) => format!("judge p={p:.2}"),
            None => "judge abstained".into(),
        });
        if self.pulled_away {
            b.factor(Cetasika::Uddhacca, 1.0, "thought pulled attention from the object");
        }
        if self.returned_to_object {
            b.factor(Cetasika::Sati, 1.0, "attention returned to the object");
        }
        b.finish()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeditationSession {
    pub object: MeditationObject,
    pub started_at: u64,
    pub duration_secs: u64,
    pub bell_start: u64,
    pub bell_end: Option<u64>,
    pub breaths: u32,
    pub breath_log: Vec<BreathRecord>,
    pub thoughts: Vec<ArisenThought>,
    pub returns: u32,
    pub radio_draws: u32,
    pub os_draws: u32,
    /// Ended by [`MeditationSession::interrupt`] rather than the bell.
    #[serde(default)]
    pub ended_early: bool,
    /// Where the next breath begins.
    pub breath_cursor: u64,
    /// The breath in progress, its rhythm already drawn.
    #[serde(default)]
    pub next_breath: Option<BreathRecord>,
}

/// Printed when he comes out of the sit, and journaled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeditationSummary {
    pub object: MeditationObject,
    pub minutes: f32,
    pub breaths: u32,
    pub thoughts_arisen: u32,
    pub pulled_away: u32,
    pub returned: u32,
    pub let_go: u32,
    pub doubted: u32,
    /// "radio", "os", "radio+os", or "none" (no breath drawn).
    pub entropy_source: String,
    pub ended_by_bell: bool,
}

impl MeditationSummary {
    pub fn journal_line(&self) -> String {
        format!(
            "Sat {:.1} min on {}: {} breaths ({} entropy); {} thoughts arose, {} pulled away, {} returned to the object, {} let go, {} in doubt{}.",
            self.minutes, self.object.describe(), self.breaths, self.entropy_source, self.thoughts_arisen,
            self.pulled_away, self.returned, self.let_go, self.doubted,
            if self.ended_by_bell { "; the bell ended it" } else { "; ended before the bell" },
        )
    }
}

impl MeditationSession {
    /// Ring the start bell at `now`.
    pub fn begin(object: MeditationObject, minutes: u32, now: u64) -> Self {
        Self {
            object,
            started_at: now,
            duration_secs: u64::from(minutes) * 60,
            bell_start: now,
            bell_end: None,
            breaths: 0,
            breath_log: Vec::new(),
            thoughts: Vec::new(),
            returns: 0,
            radio_draws: 0,
            os_draws: 0,
            ended_early: false,
            breath_cursor: now,
            next_breath: None,
        }
    }

    pub fn ends_at(&self) -> u64 {
        self.started_at + self.duration_secs
    }

    pub fn is_over(&self) -> bool {
        self.bell_end.is_some()
    }

    /// Whether a pulled-away thought still holds attention.
    pub fn wandering(&self) -> bool {
        self.thoughts.iter().any(|t| t.pulled_away && !t.returned_to_object)
    }

    /// Advance the breath to `now`: every breath that completes by `now`
    /// (and before the bell) is counted. The draw seeds the rhythm of the
    /// breaths completed in this call; one draw per call is enough. Returns
    /// a body-door vīthi for each completed breath. A thought that pulled
    /// attention away returns to the object on the next breath.
    pub fn tick(&mut self, now: u64, draw: Draw) -> Vec<Vithi> {
        let mut out = Vec::new();
        if self.is_over() {
            return out;
        }
        let limit = now.min(self.ends_at());
        let mut rng = draw.value;
        let mut used_draw = false;
        loop {
            // A breath's rhythm is fixed when it begins, so the tick rate
            // does not bias its length.
            let rec = match self.next_breath.take() {
                Some(rec) => rec,
                None => {
                    used_draw = true;
                    let v = splitmix(&mut rng);
                    BreathRecord {
                        n: self.breaths + 1,
                        started_at: self.breath_cursor,
                        inhale_secs: 3 + (v % 5) as u32,
                        exhale_secs: 3 + ((v >> 32) % 5) as u32,
                        source: draw.source,
                    }
                }
            };
            if rec.ends_at() > limit {
                self.next_breath = Some(rec);
                break;
            }
            // A pull that arose before this breath ended returns with it.
            let wandering = self.return_before(rec.ends_at()) > 0;
            out.push(rec.vithi(&self.object, wandering));
            self.breaths += 1;
            self.breath_cursor = rec.ends_at();
            self.breath_log.push(rec);
        }
        if used_draw {
            match draw.source {
                EntropyKind::Radio => self.radio_draws += 1,
                EntropyKind::Os => self.os_draws += 1,
            }
        }
        out
    }

    /// A thought arises at the mind door. `judge_p` is the judge's
    /// probability that it pulls attention from the object (`None` when it
    /// abstained). Returns the mind-door vīthi as determined now; the return
    /// to the object is recorded later on the thought itself.
    pub fn arise(&mut self, text: impl Into<String>, judge_p: Option<f32>, now: u64) -> Vithi {
        let determination = Determination::from_judge(judge_p);
        let thought = ArisenThought {
            text: text.into(),
            arisen_at: now,
            judge_p: judge_p.filter(|p| p.is_finite()).map(|p| p.clamp(0.0, 1.0)),
            determination,
            pulled_away: determination == Determination::PulledAway,
            returned_to_object: false,
            returned_at: None,
        };
        let v = thought.vithi(&self.object);
        self.thoughts.push(thought);
        v
    }

    /// Sati notices and brings attention back: every thought still holding
    /// attention returns at `now`. Returns how many returned.
    pub fn return_to_object(&mut self, now: u64) -> u32 {
        self.return_before(now)
    }

    /// Return every pull that arose at or before `at`, at time `at`.
    fn return_before(&mut self, at: u64) -> u32 {
        let mut n = 0;
        for t in self.thoughts.iter_mut().filter(|t| t.pulled_away && !t.returned_to_object && t.arisen_at <= at) {
            t.returned_to_object = true;
            t.returned_at = Some(at);
            n += 1;
        }
        self.returns += n;
        n
    }

    /// Rings the end bell once, when the duration has elapsed.
    pub fn bell(&mut self, now: u64) -> Option<Bell> {
        if self.is_over() || now < self.ends_at() {
            return None;
        }
        self.return_to_object(self.ends_at());
        self.bell_end = Some(self.ends_at());
        Some(Bell::End)
    }

    /// End the sit early (the operator spoke, or the sit was cancelled).
    pub fn interrupt(&mut self, now: u64) {
        if !self.is_over() {
            let end = now.clamp(self.started_at, self.ends_at());
            self.return_to_object(end);
            self.ended_early = end < self.ends_at();
            self.bell_end = Some(end);
            self.next_breath = None;
        }
    }

    pub fn summary(&self) -> MeditationSummary {
        let count = |d: Determination| self.thoughts.iter().filter(|t| t.determination == d).count() as u32;
        let end = self.bell_end.unwrap_or(self.breath_cursor);
        MeditationSummary {
            object: self.object.clone(),
            minutes: end.saturating_sub(self.started_at) as f32 / 60.0,
            breaths: self.breaths,
            thoughts_arisen: self.thoughts.len() as u32,
            pulled_away: count(Determination::PulledAway),
            returned: self.returns,
            let_go: count(Determination::LetGo),
            doubted: count(Determination::Doubt),
            entropy_source: match (self.radio_draws > 0, self.os_draws > 0) {
                (true, true) => "radio+os",
                (true, false) => "radio",
                (false, true) => "os",
                (false, false) => "none",
            }
            .into(),
            ended_by_bell: self.bell_end.is_some() && !self.ended_early,
        }
    }
}

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

fn short(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life::{step, DriveConfig, Drives, Phase};
    use crate::vithi::Close;

    const T0: u64 = 1_790_000_000;

    fn radio(value: u64) -> Draw {
        Draw { value, source: EntropyKind::Radio }
    }

    /// A 5-minute sit ticked every second with fixed draws.
    fn sit_five_minutes(seed: u64) -> MeditationSession {
        let mut s = MeditationSession::begin(MeditationObject::Breath, 5, T0);
        for t in 1..=300 {
            s.tick(T0 + t, radio(seed.wrapping_add(t)));
        }
        s
    }

    #[test]
    fn five_minute_sit_ends_by_bell_and_drives_follow() {
        let cfg = DriveConfig::default();
        let mut d = Drives::new(T0);
        let mut s = MeditationSession::begin(MeditationObject::Breath, 5, T0);
        step(&mut d, &cfg, &Bell::Start.stimulus(), T0);
        assert_eq!(d.phase, Phase::Meditating);
        let mut bell = None;
        for t in (10..=400).step_by(10) {
            s.tick(T0 + t, radio(t));
            if let Some(b) = s.bell(T0 + t) {
                bell = Some((t, b));
                break;
            }
        }
        assert_eq!(bell, Some((300, Bell::End)));
        assert_eq!(s.bell_end, Some(T0 + 300));
        assert_eq!(s.bell(T0 + 310), None, "the bell rings once");
        assert!(s.tick(T0 + 900, radio(1)).is_empty(), "no breaths after the bell");
        step(&mut d, &cfg, &Bell::End.stimulus(), T0 + 300);
        assert_eq!(d.phase, Phase::Resting);
        let sum = s.summary();
        assert!(sum.ended_by_bell);
        assert!((sum.minutes - 5.0).abs() < 1e-6);
        // 6–14 s per breath over 300 s.
        assert!((21..=50).contains(&sum.breaths), "{}", sum.breaths);
        assert_eq!(sum.entropy_source, "radio");
    }

    #[test]
    fn breath_count_is_deterministic_for_fixed_draws() {
        let a = sit_five_minutes(7);
        let b = sit_five_minutes(7);
        assert_eq!(a, b);
        assert_eq!(a.breaths as usize, a.breath_log.len());
        for (i, r) in a.breath_log.iter().enumerate() {
            assert_eq!(r.n as usize, i + 1);
            assert!((3..=7).contains(&r.inhale_secs) && (3..=7).contains(&r.exhale_secs));
            assert!(r.ends_at() <= a.ends_at());
            if i > 0 {
                assert_eq!(r.started_at, a.breath_log[i - 1].ends_at());
            }
        }
        // A single tick with one draw is deterministic too.
        let mut c = MeditationSession::begin(MeditationObject::Breath, 5, T0);
        c.tick(T0 + 300, radio(99));
        let mut e = MeditationSession::begin(MeditationObject::Breath, 5, T0);
        e.tick(T0 + 300, radio(99));
        assert_eq!(c, e);
        assert!(c.breaths > 0);
    }

    #[test]
    fn breath_vithi_is_a_body_door_process() {
        let mut s = MeditationSession::begin(MeditationObject::Breath, 1, T0);
        let v = s.tick(T0 + 60, Draw { value: 3, source: EntropyKind::Os });
        assert!(!v.is_empty());
        let v = &v[0];
        assert_eq!(v.door, SenseDoor::Body);
        let stages: Vec<Stage> = v.stages.iter().map(|r| r.stage).collect();
        assert_eq!(stages, vec![Stage::Contact, Stage::Investigating, Stage::Registration]);
        for c in [Cetasika::Phassa, Cetasika::Manasikara, Cetasika::Ekaggata] {
            assert!(v.factors.iter().any(|f| f.cetasika == c), "{c:?}");
        }
        assert!(v.stages[0].by.contains("os"));
        assert_eq!(s.summary().entropy_source, "os");
    }

    #[test]
    fn three_thoughts_record_pull_and_return_with_timestamps() {
        let mut s = MeditationSession::begin(MeditationObject::Breath, 5, T0);
        s.tick(T0 + 30, radio(1));
        let v1 = s.arise("Did I leave the garage light on?", Some(0.92), T0 + 40);
        let v2 = s.arise("the room is quiet", Some(0.1), T0 + 90);
        s.tick(T0 + 120, radio(2));
        let v3 = s.arise("What should I say to Paul tomorrow?", Some(0.8), T0 + 150);
        s.tick(T0 + 200, radio(3));

        let t = &s.thoughts;
        assert_eq!(t.len(), 3);
        assert_eq!((t[0].pulled_away, t[1].pulled_away, t[2].pulled_away), (true, false, true));
        assert!(t[0].returned_to_object && t[2].returned_to_object && !t[1].returned_to_object);
        assert_eq!(t[1].determination, Determination::LetGo);
        let r0 = t[0].returned_at.unwrap();
        let r2 = t[2].returned_at.unwrap();
        // Returned on the next breath after arising (a breath is at most 14 s).
        assert!(r0 >= T0 + 40 && r0 <= T0 + 40 + 14, "{r0}");
        assert!(r2 >= T0 + 150 && r2 <= T0 + 150 + 14, "{r2}");
        assert_eq!(s.returns, 2);

        // Mind-door vīthis: determined by the judge, never impulsion.
        for v in [&v1, &v2, &v3] {
            assert_eq!(v.door, SenseDoor::Mind);
            assert_eq!(v.close, Close::DeterminedOnly);
            assert!(!v.factors.iter().any(|f| f.cetasika == Cetasika::Cetana));
        }
        assert!(v1.factors.iter().any(|f| f.cetasika == Cetasika::Uddhacca));
        assert!(!v2.factors.iter().any(|f| f.cetasika == Cetasika::Uddhacca));
        // After the return, the thought's own vīthi carries sati.
        assert!(t[0].vithi(&s.object).factors.iter().any(|f| f.cetasika == Cetasika::Sati));

        let sum = s.summary();
        assert_eq!((sum.thoughts_arisen, sum.pulled_away, sum.returned, sum.let_go), (3, 2, 2, 1));
        assert!(sum.journal_line().contains("3 thoughts arose"));
    }

    #[test]
    fn a_breath_taken_while_wandering_is_less_one_pointed() {
        let mut s = MeditationSession::begin(MeditationObject::Breath, 5, T0);
        s.arise("plans", Some(0.99), T0);
        let v = s.tick(T0 + 30, radio(5));
        let ek = |v: &Vithi| v.factors.iter().find(|f| f.cetasika == Cetasika::Ekaggata).unwrap().strength;
        assert!(ek(&v[0]) < ek(&v[1]));
        assert_eq!(s.thoughts[0].returned_at, Some(s.breath_log[0].ends_at()));
    }

    #[test]
    fn abstention_is_doubt_not_no() {
        let mut s = MeditationSession::begin(MeditationObject::Mantra("buddho".into()), 5, T0);
        let v = s.arise("is this working?", None, T0 + 5);
        let unsure = s.arise("a car outside", Some(0.5), T0 + 6);
        let t = &s.thoughts[0];
        assert_eq!(t.determination, Determination::Doubt);
        assert!(!t.pulled_away && t.judge_p.is_none());
        assert_ne!(t.determination, Determination::LetGo);
        assert!(v.factors.iter().any(|f| f.cetasika == Cetasika::Vicikiccha));
        assert!(!v.factors.iter().any(|f| f.cetasika == Cetasika::Adhimokkha));
        assert!(unsure.factors.iter().any(|f| f.cetasika == Cetasika::Vicikiccha));
        let sum = s.summary();
        assert_eq!((sum.doubted, sum.let_go, sum.pulled_away), (2, 0, 0));
    }

    #[test]
    fn a_pull_still_wandering_at_the_bell_returns_with_the_bell() {
        let mut s = MeditationSession::begin(MeditationObject::Open, 1, T0);
        s.arise("lunch", Some(0.9), T0 + 58);
        assert_eq!(s.bell(T0 + 61), Some(Bell::End));
        assert_eq!(s.thoughts[0].returned_at, Some(T0 + 60));
    }

    #[test]
    fn interrupt_ends_early_without_a_bell() {
        let mut s = MeditationSession::begin(MeditationObject::Sound, 20, T0);
        s.tick(T0 + 120, radio(4));
        s.interrupt(T0 + 120);
        assert_eq!(s.bell(T0 + 20 * 60), None);
        let sum = s.summary();
        assert!(!sum.ended_by_bell);
        assert!((sum.minutes - 2.0).abs() < 1e-6);
    }

    #[test]
    fn session_round_trips_through_json() {
        let mut s = MeditationSession::begin(MeditationObject::Mantra("let go".into()), 5, T0);
        s.tick(T0 + 33, radio(11));
        s.arise("a thought", Some(0.9), T0 + 34);
        s.arise("doubt", None, T0 + 35);
        let json = serde_json::to_string(&s).unwrap();
        let back: MeditationSession = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
        // The pending breath survives, so the rhythm continues unchanged.
        let (mut a, mut b) = (s.clone(), back);
        a.tick(T0 + 300, radio(12));
        b.tick(T0 + 300, radio(12));
        assert_eq!(a, b);
        let sum: MeditationSummary = serde_json::from_str(&serde_json::to_string(&a.summary()).unwrap()).unwrap();
        assert_eq!(sum, a.summary());
    }

    #[test]
    fn objects_parse_and_the_judge_question_names_the_object() {
        assert_eq!(MeditationObject::parse("breath"), Some(MeditationObject::Breath));
        assert_eq!(MeditationObject::parse("mantra: buddho"), Some(MeditationObject::Mantra("buddho".into())));
        assert_eq!(MeditationObject::parse("tv"), None);
        let q = judge_question(&MeditationObject::Breath, " the garage ");
        assert!(q.contains("the breath") && q.contains("\"the garage\"") && q.ends_with("yes or no."));
    }
}
