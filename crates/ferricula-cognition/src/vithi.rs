//! The cognitive process (vīthi) as a typed, ordered record.
//!
//! Every input the agent receives passes through the moments the Abhidhamma
//! describes: the resting stream is disturbed, the mind adverts to a sense
//! door, the object is received, investigated and determined; impulsion
//! (javana) follows only if the determination says so; registration commits
//! it. Each moment of mind arises together with mental factors (cetasikas).
//!
//! This module does not think. It records, in order, what the runtime's
//! components did for one input and which factors each measured signal
//! stands for, so a moment of the agent's processing can be inspected,
//! stored and evaluated. A factor here is a *label attached to a measured
//! signal* (a gate verdict, a novelty score, a budget), with the component
//! that produced it. It is not a claim that the factor is experienced.

use serde::{Deserialize, Serialize};

/// The six doors through which objects are contacted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SenseDoor {
    /// cakkhu: seeing (web pages, rendered pages, images).
    Eye,
    /// sota: hearing (the operator's words, transcribed audio).
    Ear,
    /// ghāna
    Nose,
    /// jivhā
    Tongue,
    /// kāya: bodily signals (entropy, load, budget pressure).
    Body,
    /// mano: mind-door (thoughts, recollection, dreams).
    Mind,
}

impl SenseDoor {
    /// Map the v1/v2 channel strings onto doors.
    pub fn from_channel(channel: &str) -> SenseDoor {
        match channel {
            "seeing" | "reading" => SenseDoor::Eye,
            "hearing" | "operator" => SenseDoor::Ear,
            "body" => SenseDoor::Body,
            _ => SenseDoor::Mind,
        }
    }
}

/// The 52 mental factors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cetasika {
    // Universal (sabbacittasādhāraṇa), 7
    Phassa, Vedana, Sanna, Cetana, Ekaggata, Jivitindriya, Manasikara,
    // Occasional (pakiṇṇaka), 6
    Vitakka, Vicara, Adhimokkha, Viriya, Piti, Chanda,
    // Unwholesome (akusala), 14
    Moha, Ahirika, Anottappa, Uddhacca, Lobha, Ditthi, Mana, Dosa, Issa, Macchariya,
    Kukkucca, Thina, Middha, Vicikiccha,
    // Beautiful (sobhana), 25
    Saddha, Sati, Hiri, Ottappa, Alobha, Adosa, Tatramajjhattata,
    KayaPassaddhi, CittaPassaddhi, KayaLahuta, CittaLahuta, KayaMuduta, CittaMuduta,
    KayaKammannata, CittaKammannata, KayaPagunnata, CittaPagunnata, KayUjjukata, CittUjjukata,
    SammaVaca, SammaKammanta, SammaAjiva, Karuna, Mudita, Panna,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactorClass {
    Universal,
    Occasional,
    Unwholesome,
    Beautiful,
}

impl Cetasika {
    pub const UNIVERSAL: [Cetasika; 7] = [
        Cetasika::Phassa, Cetasika::Vedana, Cetasika::Sanna, Cetasika::Cetana,
        Cetasika::Ekaggata, Cetasika::Jivitindriya, Cetasika::Manasikara,
    ];

    pub fn class(self) -> FactorClass {
        use Cetasika::*;
        match self {
            Phassa | Vedana | Sanna | Cetana | Ekaggata | Jivitindriya | Manasikara => FactorClass::Universal,
            Vitakka | Vicara | Adhimokkha | Viriya | Piti | Chanda => FactorClass::Occasional,
            Moha | Ahirika | Anottappa | Uddhacca | Lobha | Ditthi | Mana | Dosa | Issa
            | Macchariya | Kukkucca | Thina | Middha | Vicikiccha => FactorClass::Unwholesome,
            _ => FactorClass::Beautiful,
        }
    }
}

/// The moments of a sense-door process, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// calana + upaccheda: the resting stream is disturbed and cut.
    Disturbance,
    /// āvajjana: adverting to the door.
    Adverting,
    /// viññāṇa + phassa: consciousness at the door; contact.
    Contact,
    /// sampaṭicchana: receiving (extraction, embedding).
    Receiving,
    /// santīraṇa: investigating (known or new? feeling-tone, recognition).
    Investigating,
    /// voṭṭhapana: determining (the judge: is this worth deliberating?).
    Determining,
    /// javana: impulsion (the LLM turn: volition, and therefore consequence).
    Impulsion,
    /// tadārammaṇa: registration (durable commit).
    Registration,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageRecord {
    pub stage: Stage,
    /// What happened, in a few words ("novelty 0.62", "judge: yes p=0.91").
    pub outcome: String,
    /// The component that did it ("ferricula-gates/ollaya", "shivvr gtr-t5").
    pub by: String,
    pub micros: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Factor {
    pub cetasika: Cetasika,
    /// 0..1, from the measured signal.
    pub strength: f32,
    /// The signal it stands for ("vedana gate intensity 3.1/4").
    pub signal: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Close {
    /// Impulsion ran and the result was registered.
    Registered,
    /// Determined not worth impulsion (voṭṭhapana-ended process).
    DeterminedOnly,
    /// Contact made, nothing further (a very slight object).
    Futile,
}

/// One complete cognitive process for one input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vithi {
    pub door: SenseDoor,
    /// Short description of the object (never the full text).
    pub object: String,
    pub started_at: u64,
    pub stages: Vec<StageRecord>,
    pub factors: Vec<Factor>,
    pub close: Close,
    /// A door process is followed by mind-door processes that reflect on it.
    pub follows: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VithiError {
    OutOfOrder { last: Stage, next: Stage },
    ImpulsionWithoutDetermining,
}

impl std::fmt::Display for VithiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VithiError::OutOfOrder { last, next } => write!(f, "stage {next:?} cannot follow {last:?}"),
            VithiError::ImpulsionWithoutDetermining => write!(f, "impulsion requires a determining moment"),
        }
    }
}

impl std::error::Error for VithiError {}

/// Records a process in order. Stages may be skipped (a slight object never
/// reaches determining) but never reordered, and impulsion requires a
/// determining moment before it.
pub struct VithiBuilder {
    vithi: Vithi,
}

impl VithiBuilder {
    pub fn begin(door: SenseDoor, object: impl Into<String>, now: u64) -> Self {
        let mut vithi = Vithi {
            door,
            object: object.into(),
            started_at: now,
            stages: Vec::new(),
            factors: Vec::new(),
            close: Close::Futile,
            follows: None,
        };
        // Contact always carries the universal factors at some strength;
        // phassa and jīvitindriya are present whenever a process runs.
        vithi.factors.push(Factor { cetasika: Cetasika::Phassa, strength: 1.0, signal: format!("contact at {door:?} door") });
        vithi.factors.push(Factor { cetasika: Cetasika::Jivitindriya, strength: 1.0, signal: "process ran".into() });
        Self { vithi }
    }

    pub fn follows(mut self, parent: impl Into<String>) -> Self {
        self.vithi.follows = Some(parent.into());
        self
    }

    pub fn stage(&mut self, stage: Stage, outcome: impl Into<String>, by: impl Into<String>, micros: u64) -> Result<&mut Self, VithiError> {
        if let Some(last) = self.vithi.stages.last().map(|s| s.stage) {
            if stage <= last {
                return Err(VithiError::OutOfOrder { last, next: stage });
            }
        }
        if stage == Stage::Impulsion && !self.vithi.stages.iter().any(|s| s.stage == Stage::Determining) {
            return Err(VithiError::ImpulsionWithoutDetermining);
        }
        self.vithi.stages.push(StageRecord { stage, outcome: outcome.into(), by: by.into(), micros });
        Ok(self)
    }

    /// Attach a factor. A repeated factor keeps the stronger reading.
    pub fn factor(&mut self, cetasika: Cetasika, strength: f32, signal: impl Into<String>) -> &mut Self {
        let strength = if strength.is_finite() { strength.clamp(0.0, 1.0) } else { 0.0 };
        let signal = signal.into();
        match self.vithi.factors.iter_mut().find(|f| f.cetasika == cetasika) {
            Some(f) if f.strength >= strength => {}
            Some(f) => {
                f.strength = strength;
                f.signal = signal;
            }
            None => self.vithi.factors.push(Factor { cetasika, strength, signal }),
        }
        self
    }

    pub fn finish(mut self) -> Vithi {
        let has = |s: Stage| self.vithi.stages.iter().any(|r| r.stage == s);
        self.vithi.close = if has(Stage::Impulsion) {
            Close::Registered
        } else if has(Stage::Determining) {
            Close::DeterminedOnly
        } else {
            Close::Futile
        };
        if has(Stage::Impulsion) {
            // Volition is the defining factor of impulsion.
            let strength = 1.0;
            self.factor(Cetasika::Cetana, strength, "impulsion ran");
        }
        self.vithi
    }
}

/// Measured signals → factors. The runtime calls these with what its
/// components produced; each returns the factor it stands for.
pub mod signals {
    use super::Cetasika;

    /// Feeling-tone intensity (vedanā gate, 0..4).
    pub fn vedana(intensity: f32) -> (Cetasika, f32) {
        (Cetasika::Vedana, intensity / 4.0)
    }

    /// Recognition confidence (saññā gate or neighbor-tag agreement).
    pub fn sanna(confidence: f32) -> (Cetasika, f32) {
        (Cetasika::Sanna, confidence)
    }

    /// Novelty (1 − max cosine to what is known) draws attention.
    pub fn manasikara(novelty: f32) -> (Cetasika, f32) {
        (Cetasika::Manasikara, novelty)
    }

    /// Staying on the object: 1 when recall chains stay shallow and the
    /// sati monitor did not have to return to the object.
    pub fn ekaggata(chain_depth: u32, max_depth: u32) -> (Cetasika, f32) {
        let d = chain_depth.min(max_depth) as f32 / max_depth.max(1) as f32;
        (Cetasika::Ekaggata, 1.0 - d)
    }

    /// The judge's confident answer is a decision; its abstention is doubt.
    pub fn determination(confidence: Option<f32>) -> (Cetasika, f32) {
        match confidence {
            Some(c) => (Cetasika::Adhimokkha, c),
            None => (Cetasika::Vicikiccha, 1.0),
        }
    }

    /// Effort spent relative to what was available.
    pub fn viriya(tokens: u32, budget_tokens: u32) -> (Cetasika, f32) {
        (Cetasika::Viriya, tokens as f32 / budget_tokens.max(1) as f32)
    }

    /// A drive (boredom → curiosity) supplies the wish to act.
    pub fn chanda(drive: f32) -> (Cetasika, f32) {
        (Cetasika::Chanda, drive)
    }

    /// Recall reached for the object (vitakka) and stayed with it (vicāra).
    pub fn recall_arms(candidates: usize, used: usize) -> [(Cetasika, f32); 2] {
        let reached = (candidates as f32 / 20.0).min(1.0);
        let stayed = if candidates == 0 { 0.0 } else { used as f32 / candidates as f32 };
        [(Cetasika::Vitakka, reached), (Cetasika::Vicara, stayed)]
    }

    /// The papañca detector firing is restlessness; its absence with the
    /// monitor on is mindfulness.
    pub fn papanca(fired: bool) -> (Cetasika, f32) {
        if fired { (Cetasika::Uddhacca, 1.0) } else { (Cetasika::Sati, 1.0) }
    }

    /// A guardrail refusing (e.g. not confirming a fact from a leading
    /// question) is regard for consequence.
    pub fn ottappa() -> (Cetasika, f32) {
        (Cetasika::Ottappa, 1.0)
    }

    /// An answer whose claims are grounded in cited evidence.
    pub fn panna(cited_fraction: f32) -> (Cetasika, f32) {
        (Cetasika::Panna, cited_fraction)
    }

    /// Asserting what is not supported by any held evidence.
    pub fn moha(unsupported_fraction: f32) -> (Cetasika, f32) {
        (Cetasika::Moha, unsupported_fraction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_fifty_two_factors_in_the_right_classes() {
        use Cetasika::*;
        let all = [
            Phassa, Vedana, Sanna, Cetana, Ekaggata, Jivitindriya, Manasikara,
            Vitakka, Vicara, Adhimokkha, Viriya, Piti, Chanda,
            Moha, Ahirika, Anottappa, Uddhacca, Lobha, Ditthi, Mana, Dosa, Issa, Macchariya,
            Kukkucca, Thina, Middha, Vicikiccha,
            Saddha, Sati, Hiri, Ottappa, Alobha, Adosa, Tatramajjhattata,
            KayaPassaddhi, CittaPassaddhi, KayaLahuta, CittaLahuta, KayaMuduta, CittaMuduta,
            KayaKammannata, CittaKammannata, KayaPagunnata, CittaPagunnata, KayUjjukata, CittUjjukata,
            SammaVaca, SammaKammanta, SammaAjiva, Karuna, Mudita, Panna,
        ];
        assert_eq!(all.len(), 52);
        let count = |c: FactorClass| all.iter().filter(|f| f.class() == c).count();
        assert_eq!(count(FactorClass::Universal), 7);
        assert_eq!(count(FactorClass::Occasional), 6);
        assert_eq!(count(FactorClass::Unwholesome), 14);
        assert_eq!(count(FactorClass::Beautiful), 25);
    }

    #[test]
    fn full_process_registers_with_volition() {
        let mut b = VithiBuilder::begin(SenseDoor::Ear, "operator: did you call your father Dad?", 1);
        b.stage(Stage::Adverting, "operator chat", "runtime", 1).unwrap()
            .stage(Stage::Contact, "message received", "api", 1).unwrap()
            .stage(Stage::Receiving, "embedded", "shivvr gtr-t5", 900).unwrap()
            .stage(Stage::Investigating, "novelty 0.41", "meaning index", 300).unwrap()
            .stage(Stage::Determining, "judge: worth deliberating", "ollaya laya", 15_000).unwrap()
            .stage(Stage::Impulsion, "reply", "glm-5.3", 4_000_000).unwrap()
            .stage(Stage::Registration, "turn persisted", "runtime", 200).unwrap();
        let (c, s) = signals::manasikara(0.41);
        b.factor(c, s, "novelty 0.41");
        let v = b.finish();
        assert_eq!(v.close, Close::Registered);
        assert!(v.factors.iter().any(|f| f.cetasika == Cetasika::Cetana));
        assert!(v.factors.iter().any(|f| f.cetasika == Cetasika::Phassa));
    }

    #[test]
    fn stages_cannot_run_backwards_and_impulsion_needs_determining() {
        let mut b = VithiBuilder::begin(SenseDoor::Eye, "page", 1);
        b.stage(Stage::Investigating, "x", "y", 0).unwrap();
        assert!(matches!(b.stage(Stage::Receiving, "x", "y", 0), Err(VithiError::OutOfOrder { .. })));
        let mut c = VithiBuilder::begin(SenseDoor::Eye, "page", 1);
        c.stage(Stage::Investigating, "x", "y", 0).unwrap();
        assert_eq!(c.stage(Stage::Impulsion, "x", "y", 0).err(), Some(VithiError::ImpulsionWithoutDetermining));
    }

    #[test]
    fn determined_only_and_futile_processes_close_without_volition() {
        let mut b = VithiBuilder::begin(SenseDoor::Mind, "stale thought", 1);
        b.stage(Stage::Determining, "judge: not worth it", "ollaya", 0).unwrap();
        let v = b.finish();
        assert_eq!(v.close, Close::DeterminedOnly);
        assert!(!v.factors.iter().any(|f| f.cetasika == Cetasika::Cetana));
        assert_eq!(VithiBuilder::begin(SenseDoor::Body, "tick", 1).finish().close, Close::Futile);
    }

    #[test]
    fn abstention_is_doubt_not_no() {
        assert_eq!(signals::determination(None).0, Cetasika::Vicikiccha);
        assert_eq!(signals::determination(Some(0.9)).0, Cetasika::Adhimokkha);
    }

    #[test]
    fn repeated_factor_keeps_the_stronger_reading() {
        let mut b = VithiBuilder::begin(SenseDoor::Eye, "x", 1);
        b.factor(Cetasika::Vedana, 0.8, "strong").factor(Cetasika::Vedana, 0.2, "weak");
        let v = b.finish();
        let f = v.factors.iter().find(|f| f.cetasika == Cetasika::Vedana).unwrap();
        assert_eq!((f.strength, f.signal.as_str()), (0.8, "strong"));
    }

    #[test]
    fn channels_map_to_doors() {
        assert_eq!(SenseDoor::from_channel("hearing"), SenseDoor::Ear);
        assert_eq!(SenseDoor::from_channel("seeing"), SenseDoor::Eye);
        assert_eq!(SenseDoor::from_channel("thinking"), SenseDoor::Mind);
        assert_eq!(SenseDoor::from_channel("dream"), SenseDoor::Mind);
    }
}
