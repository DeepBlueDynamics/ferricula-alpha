# Episode causal execution — Outrageous Deer

**Date:** 2026-09-26
**Lane:** Causal Dynamics & Logic (`crates/ferricula-episode/**`)
**Status:** Slice 1 is in the crate. Slices 2–4 are specified and not applied.
**Test (after the clatter evidence gate):** from the workspace root, `cargo test -p ferricula-episode --offline` finished exit 0. Lib 34 passed, integration 8 passed, parity 29 passed, doc-tests 0. The unused-variable warning in `tests/parity.rs` (`let lexical`) was already there; this lane did not introduce it.

## Clatter evidence gate — landed

Verified against `audit/clatter-live-failure.md` and the episode code.

Measured failure (that audit, not this run): Steve's model said the under-couch miss meant the clatter was not the vape. Retrieval had succeeded. The stored observation stayed unresolved. That was a generation error.

Store gap that made the same mistake committable: `EpisodeAdapter` accepted any observation as status evidence. A `NotSeenInScope` report, or the unexplained report itself, could Support or Disconfirm a hypothesis.

Gate now in `adapter.rs` and `projection.rs`, predicate in `causal.rs`: a status change needs an independent observation. A scoped miss, an inconclusive search, or the unexplained target alone is rejected. The original report is not rewritten. A separate unscoped or `Found` observation can still Support, which is what the existing clatter test does with the corner finding. Regression: `scoped_miss_cannot_support_or_disconfirm_a_cause`.

Not measured here: a fresh Steve container, benchmark scores, or the chat model obeying the rejection. Those are other lanes. This crate does not call a model and recorded no token cost.

## What is already true in code

Verified by reading the files named here.

- `ferricula-episode` is a versioned episode prototype: observations, goals, hypotheses, status transitions, and `ExplicitLink { relation: String, weight }` (`model.rs`).
- `overlay.rs` already separates planes. The recovered base is never opened for write. Overlay events are an append-only hash chain. Destruction and promotion stay ineffective until an operator `Approval`. Base artifact names are listed and refused.
- `projection.rs` lines 281–282 insert every `ExplicitLink` in both directions under the same relation string. Hypothesis links at lines 162–164 are different: `explains` one way, `explained_by` the other.
- `ferricula-core` `graph.rs` `EdgeKind` is only `Semantic` (both ways), `Causal` (from sees to), and `Structural` (both ways, dream-immune). There is no Paṭṭhāna type in core. Core is Major Wren's tree. This lane does not edit it.
- Episode tests store legacy labels `investigates`, `followed_by`, `pertains_to`, `relates_to`. Those strings stay valid.

## What the 2026-09-26 research changes for this lane

- Addendum A §A.1.1: ojā / jarā / vigata change retrieval priority. They do not delete text. Text leaves only by deliberate upekkhā or nirodha.
- Addendum A §A.5.3: a recall write-back is an overlay tagged as a reconstruction, linked to the raw parent. Raw akkhara is not modified. The curator must see that label. Overlays are not training data (Rule 9). Rule 10: no drift without sati. The sati monitors themselves belong to Burning Dingo.
- Plan §4.3: a cluster merges only when `same_truth` is high and `contradiction` is low. A contradiction is its own memory event. The gate is Splendid Angelfish's. The hold rule is this lane's.
- Decision H (approved 2026-09-19 in `ARCHITECTURE.md`) and research note 18 use "two truths" for different assignments. See the open item below. The storage fact both require is already the overlay invariant: cognitive writes do not rewrite the base ledger.

## Slice 1 — landed

`ferricula-episode/src/causal.rs`, exported as `ferricula_episode::causal`.

- `PatthanaRelation`: the 24 conditions, ASCII snake_case serde.
- `Walk::{Directed, Mutual, Qualifier}` from the operators in research note 08 §2.1. Directed means the reverse is a different relation, not a cloned label. This is an engineering reading. Note 08 does not publish a walk table.
- `root()` returns a value only for the four roots named in note 08 §2.2: ārammaṇa, upanissaya, kamma, atthi. The note says the other twenty reduce to those four and does not list the fold. This module does not invent one.
- `transition_consensus`: a transition is admitted only when all four roots are witnessed. Otherwise `Hold(MissingRoot)`.
- `goal_observation_consensus`: contradiction is held apart; an unanchored goal–observation edge is held (note 18 §7.2.3).
- `decay_may_erase_text()` is `false`.
- `parse_relation` accepts the 24 names only. Legacy labels parse as `None`.

No projection behavior changed in this slice.

## Slice 2 — direction in projection (landed)

`projection.rs` now walks typed relations by `Walk`. Directed inserts one arc. Mutual inserts both. Qualifier inserts none. A legacy string (`parse_relation` returns `None`) stays bidirectional.

`Pacchājāta` is directed. It is not a second copy of `kamma`.

Test: `typed_kamma_link_is_one_way_and_legacy_labels_stay_both_ways`. A `kamma` link is visible from the source and not from the target. `followed_by` stays both ways. `vigata` creates no neighbor.

An evidence id does not entail a claim. Support, Disconfirm, and Supersede also require an already-recorded link in {vipaka, kamma, purejata, pacchajata, upanissaya} between that observation and the hypothesis or its target. See `audit/2026-09-26/episode-review.md`.

Latest full run: `cargo test -p ferricula-episode --offline` from the workspace root, exit 0. Lib 35, integration 8, parity 31.

## Slice 3 — reconstruction links (not applied)

Add an episode item, or a typed link plane, with:

- parent id (raw observation or base ref)
- overlay event id
- curator version
- task id that triggered the write-back

Invariants to test:

- inserting one does not change the parent observation text
- the query view can tell a reconstruction from the raw report
- this crate does not emit a training row

Burning Dingo owns the curator that writes these. This lane owns the record shape and the "raw text unchanged" check.

## Slice 4 — handoff, not a second graph

Core keeps `Edge { label, weight, kind }`. When Wren wants typed edges, the mapping is:

| `Walk` | suggested `EdgeKind` |
|---|---|
| Directed | Causal |
| Mutual | Semantic |
| Qualifier | do not insert a neighbor |

`Anantara` is directed here. Core's `Structural` is bidirectional and dream-immune. Those are different contracts. Do not alias them until Wren agrees.

Spreading activation from note 10 §4.1 stays out of this crate until the walk in slice 2 exists. The formula's relation weights are not in the source as numbers. No default coefficients will be invented.

## Consensus rules this lane will enforce

1. Four roots present, or the transition does not fire.
2. Contradiction is held apart. It is not averaged into one edge.
3. A goal–observation edge names a real observation. Scoped `NotSeenInScope` stays a negative record. The overlay does not replace it with a positive object (note 18 §7.2.2, already represented by `ScopedSearchResult`).
4. Decay does not erase text, edges, or parent akkhara.
5. Base volume stays read-only. Causal commits are overlay events.

## Open item for Eldest Dog

`ARCHITECTURE.md` Decision H (approved): conventional = Lume plus named evidence; ultimate = Ferricula's dependent lifecycle.

`ORDERS.md` 2026-09-26 and research note 18: Shentong = read-only ground, Rangtong = writable overlay.

Those sentences assign the words differently. Code in this lane uses the existing names `MemoryRef::Base` and `MemoryRef::Overlay`. It does not add `Shentong` or `Rangtong` types until the coordinator picks the public vocabulary.

## Asks of the other lanes

- **Angelfish:** `sankhara-merge.contradiction` and `same_truth` probabilities are inputs to `goal_observation_consensus`. This crate will not threshold them until the gate schema is frozen.
- **Dingo:** reconstruction records need a curator version and a task id. Sati monitors stay in cognition.
- **Wren:** no core edit requested. Slice 4 is a mapping proposal.
- **Viper:** slice 2 needs the directed-link projection test above. Tier 6 "custom Abhidhamma" cases can call `transition_consensus` and `decay_may_erase_text` directly.
- **Penguin:** retrieval priority after vigata is yours. This lane only guarantees that vigata is not a delete.
