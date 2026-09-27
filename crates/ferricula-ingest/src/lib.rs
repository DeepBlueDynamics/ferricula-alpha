//! Document sense door for Ferricula.
//!
//! A source (inline text, a web page read through grub, or a PDF) is
//! extracted to text, cut into verbatim sections, and kept in a
//! [`DocumentStore`]: the evidence plane, which never decays. The runtime
//! records the *experience* of reading in memory; the words themselves stay
//! here so every answer can quote them exactly.

pub mod extract;
pub mod sections;
pub mod store;

pub use extract::{ExtractConfig, Extracted, Source, extract};
pub use sections::DocSection;
pub use store::{DocumentMeta, DocumentRecord, DocumentStore, SectionHit};
