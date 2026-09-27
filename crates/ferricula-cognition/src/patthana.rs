//! The 24 conditions (paccaya) of the Paṭṭhāna as typed graph-edge labels.
//!
//! An edge in the memory graph should say *why* two things are connected,
//! not just that they are. The runtime creates edges at specific events;
//! each event maps to the condition it instantiates (research/10). Edges
//! proposed from similarity alone are hypotheses until a gate or the
//! operator confirms them.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Paccaya {
    Hetu, Arammana, Adhipati, Anantara, Samanantara, Sahajata, Annamanna, Nissaya,
    Upanissaya, Purejata, Pacchajata, Asevana, Kamma, Vipaka, Ahara, Indriya,
    Jhana, Magga, Sampayutta, Vippayutta, Atthi, Natthi, Vigata, Avigata,
}

impl Paccaya {
    pub const ALL: [Paccaya; 24] = [
        Paccaya::Hetu, Paccaya::Arammana, Paccaya::Adhipati, Paccaya::Anantara,
        Paccaya::Samanantara, Paccaya::Sahajata, Paccaya::Annamanna, Paccaya::Nissaya,
        Paccaya::Upanissaya, Paccaya::Purejata, Paccaya::Pacchajata, Paccaya::Asevana,
        Paccaya::Kamma, Paccaya::Vipaka, Paccaya::Ahara, Paccaya::Indriya,
        Paccaya::Jhana, Paccaya::Magga, Paccaya::Sampayutta, Paccaya::Vippayutta,
        Paccaya::Atthi, Paccaya::Natthi, Paccaya::Vigata, Paccaya::Avigata,
    ];

    /// Edge label written into the graph: `paccaya:<name>`.
    pub fn label(self) -> String {
        format!("paccaya:{}", serde_json::to_value(self).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default())
    }

    pub fn from_label(label: &str) -> Option<Paccaya> {
        let name = label.strip_prefix("paccaya:")?;
        serde_json::from_value(serde_json::Value::String(name.to_string())).ok()
    }

    pub fn gloss(self) -> &'static str {
        use Paccaya::*;
        match self {
            Hetu => "root: the drive or goal that produced it",
            Arammana => "object: what it was about",
            Adhipati => "predominance: an overriding goal",
            Anantara => "proximity: immediately preceded it",
            Samanantara => "contiguity: in an uninterrupted sequence with it",
            Sahajata => "co-nascence: arose together (same moment)",
            Annamanna => "mutuality: each supports the other",
            Nissaya => "dependence: built on it",
            Upanissaya => "decisive support: strongly drew it forth",
            Purejata => "pre-nascence: prior context it relied on",
            Pacchajata => "post-nascence: later outcome feeding back",
            Asevana => "repetition: strengthened by recurring together",
            Kamma => "action: produced by a volitional act",
            Vipaka => "result: the consequence of an act",
            Ahara => "nutriment: resources that sustained it",
            Indriya => "faculty: the capability that made it possible",
            Jhana => "absorption: sustained focus",
            Magga => "path: a deliberate route taken",
            Sampayutta => "association: fused in one representation",
            Vippayutta => "dissociation: separate, non-interfering",
            Atthi => "presence: present together in working memory",
            Natthi => "absence: arose because the other was gone",
            Vigata => "disappearance: followed its release",
            Avigata => "non-disappearance: persists alongside it",
        }
    }
}

/// Runtime events that create edges, and the condition each instantiates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkEvent {
    /// A reading/reflection produced by curiosity about a conversation thread.
    CuriosityFromThread,
    /// A document section and the memory of reading it.
    ReadFrom,
    /// A reflection written about something read.
    ReflectedOn,
    /// Memories recalled together to answer one question.
    RecalledTogether,
    /// Two memories recalled together again (repetition).
    RecalledTogetherAgain,
    /// A dream built from these traces.
    DreamedFrom,
    /// The next turn of a conversation.
    NextTurn,
    /// Cosine-near but never co-recalled: a hypothesis, not a fact.
    SimilarityHypothesis,
    /// A consolidation cluster member and its index entry.
    ConsolidatedInto,
    /// A memory written after another was deliberately released.
    AfterRelease,
}

impl LinkEvent {
    pub fn condition(self) -> Paccaya {
        use LinkEvent::*;
        match self {
            CuriosityFromThread => Paccaya::Hetu,
            ReadFrom => Paccaya::Arammana,
            ReflectedOn => Paccaya::Nissaya,
            RecalledTogether => Paccaya::Atthi,
            RecalledTogetherAgain => Paccaya::Asevana,
            DreamedFrom => Paccaya::Upanissaya,
            NextTurn => Paccaya::Anantara,
            SimilarityHypothesis => Paccaya::Sampayutta,
            ConsolidatedInto => Paccaya::Annamanna,
            AfterRelease => Paccaya::Vigata,
        }
    }

    /// Only similarity proposals need confirmation before they are facts.
    pub fn is_hypothesis(self) -> bool {
        matches!(self, LinkEvent::SimilarityHypothesis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twenty_four_distinct_labels_round_trip() {
        let labels: std::collections::HashSet<String> = Paccaya::ALL.iter().map(|p| p.label()).collect();
        assert_eq!(labels.len(), 24);
        for p in Paccaya::ALL {
            assert_eq!(Paccaya::from_label(&p.label()), Some(p));
            assert!(!p.gloss().is_empty());
        }
        assert_eq!(Paccaya::Asevana.label(), "paccaya:asevana");
        assert_eq!(Paccaya::from_label("similar"), None);
    }

    #[test]
    fn repetition_strengthens_and_similarity_is_a_hypothesis() {
        assert_eq!(LinkEvent::RecalledTogetherAgain.condition(), Paccaya::Asevana);
        assert!(LinkEvent::SimilarityHypothesis.is_hypothesis());
        assert!(!LinkEvent::ReadFrom.is_hypothesis());
    }
}
