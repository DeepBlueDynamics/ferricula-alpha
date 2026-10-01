//! The 24 conditions (paccaya) of the Paṭṭhāna as typed graph-edge labels.
//!
//! An edge in the memory graph should say *why* two things are connected,
//! not just that they are. The runtime creates edges at specific events;
//! each event maps to the condition it instantiates (research/10). Edges
//! proposed from similarity alone are hypotheses until a gate or the
//! operator confirms them.
//!
//! `LinkEvent::condition` and `DreamPool::link_event` apply only when a
//! new link is created. An edge or memory row already stored keeps the
//! label it was written with. Nothing here rewrites either.

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

/// Which pool a new dream link is drawn from.
///
/// A distant recovered trace and today's residue each name a condition.
/// An unresolved observation names none, so no paccaya edge is created.
/// Picking a pool does not rewrite an edge or a memory row already stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DreamPool {
    /// A distant recovered trace.
    Distant,
    /// Today's residue.
    Residue,
    /// An unresolved observation.
    Unresolved,
}

impl DreamPool {
    /// Event for a new dream link from this pool.
    ///
    /// `None` means write no paccaya edge.
    pub fn link_event(self) -> Option<LinkEvent> {
        match self {
            DreamPool::Distant => Some(LinkEvent::DreamedFromDistant),
            DreamPool::Residue => Some(LinkEvent::DreamedFromResidue),
            DreamPool::Unresolved => None,
        }
    }
}

/// Runtime events that create edges, and the condition each instantiates.
///
/// `condition` is the mapping for a new link. It does not rewrite an edge
/// or a memory row that is already stored; those keep the label they were
/// written with.
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
    /// A dream built from a distant recovered trace.
    DreamedFromDistant,
    /// A dream built from today's residue.
    DreamedFromResidue,
    /// The next turn of a conversation.
    NextTurn,
    /// Cosine-near but never co-recalled: a hypothesis, not a fact.
    SimilarityHypothesis,
    /// A consolidation cluster member and its index entry.
    ConsolidatedInto,
    /// A memory written after another was deliberately released.
    AfterRelease,
    /// A verdict that takes a memory as its object and marks it disputed
    /// (a conflict flagged, no winner yet).
    Disputes,
    /// A verdict, settled by evidence, that replaces a memory's claim; it
    /// predominates over the memory wherever the two meet.
    Supersedes,
}

impl LinkEvent {
    pub fn condition(self) -> Paccaya {
        use LinkEvent::*;
        match self {
            CuriosityFromThread => Paccaya::Hetu,
            ReadFrom => Paccaya::Arammana,
            ReflectedOn => Paccaya::Purejata,
            RecalledTogether => Paccaya::Atthi,
            RecalledTogetherAgain => Paccaya::Asevana,
            DreamedFromDistant => Paccaya::Upanissaya,
            DreamedFromResidue => Paccaya::Purejata,
            NextTurn => Paccaya::Anantara,
            SimilarityHypothesis => Paccaya::Sampayutta,
            ConsolidatedInto => Paccaya::Annamanna,
            AfterRelease => Paccaya::Vigata,
            Disputes => Paccaya::Arammana,
            // TODO(Kord): research/2026-10-01-abhidhamma-fidelity.md §7.2
            // says a verdict superseding a memory is UNSUPPORTED and must
            // not emit adhipati. What Supersedes should emit instead is
            // undecided. This arm stays Adhipati until that decision.
            Supersedes => Paccaya::Adhipati,
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

    #[test]
    fn new_links_map_reflections_and_dream_pools_supersedes_unchanged() {
        assert_eq!(LinkEvent::ReflectedOn.condition(), Paccaya::Purejata);

        assert_eq!(LinkEvent::DreamedFromDistant.condition(), Paccaya::Upanissaya);
        assert_eq!(
            DreamPool::Distant.link_event().map(LinkEvent::condition),
            Some(Paccaya::Upanissaya),
        );

        assert_eq!(LinkEvent::DreamedFromResidue.condition(), Paccaya::Purejata);
        assert_eq!(
            DreamPool::Residue.link_event().map(LinkEvent::condition),
            Some(Paccaya::Purejata),
        );

        assert_eq!(DreamPool::Unresolved.link_event(), None);

        assert_eq!(LinkEvent::Supersedes.condition(), Paccaya::Adhipati);
    }
}
