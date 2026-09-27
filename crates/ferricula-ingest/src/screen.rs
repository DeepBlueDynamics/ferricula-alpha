//! Junk-page screen: refuse pages that are an access gate (paywall, login
//! wall, cookie wall) or too thin to be worth remembering, before they reach
//! the evidence plane.
//!
//! [`extract`](crate::extract) applies [`screen`] to web pages read through
//! grub. The error it returns wraps a typed [`Rejected`]; callers find it
//! with [`rejection`] (or `err.downcast_ref::<Rejected>()`). Inline text and
//! PDFs are not screened by `extract` (an operator handed them over on
//! purpose); call [`screen`] directly, or use
//! [`extract_screened`](crate::extract::extract_screened), to screen them.

use serde::{Deserialize, Serialize};

use crate::extract::Extracted;

/// Pages with less prose than this (words, after nav/link/citation lines
/// and gate phrases are stripped) are rejected.
pub const MIN_PROSE_WORDS: usize = 150;

/// A page carrying strong paywall markers is rejected unless it still has
/// at least this much prose (a full article behind a "subscribe" footer
/// passes; an abstract plus "Access options" does not).
pub const PAYWALL_PROSE_WORDS: usize = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectReason {
    /// Subscribe / buy / "Access options" gate in front of the content.
    Paywall,
    /// Sign-in or registration form in front of the content.
    LoginWall,
    /// Cookie-consent interstitial in place of the content.
    CookieWall,
    /// Not enough prose to be a document.
    TooThin,
}

impl RejectReason {
    pub fn as_str(self) -> &'static str {
        match self {
            RejectReason::Paywall => "paywall",
            RejectReason::LoginWall => "login_wall",
            RejectReason::CookieWall => "cookie_wall",
            RejectReason::TooThin => "too_thin",
        }
    }
}

/// A page the screen refused, with the evidence for the decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rejected {
    pub reason: RejectReason,
    pub title: String,
    pub origin: String,
    /// Words of prose left after stripping nav, links, citations and gates.
    pub prose_words: usize,
    /// Gate phrases found, per kind.
    pub paywall_hits: usize,
    pub login_hits: usize,
    pub cookie_hits: usize,
}

impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "rejected: {} ({:?} at {}: {} prose words; gate phrases paywall={} login={} cookie={})",
            self.reason.as_str(), self.title, self.origin, self.prose_words,
            self.paywall_hits, self.login_hits, self.cookie_hits
        )
    }
}

impl std::error::Error for Rejected {}

/// The typed rejection inside an ingest/extract error, if that is what it was.
pub fn rejection(error: &anyhow::Error) -> Option<&Rejected> {
    error.chain().find_map(|e| e.downcast_ref::<Rejected>())
}

/// Strong paywall markers: phrases that appear on gate pages, rarely in prose.
const PAYWALL: &[&str] = &[
    "access options",
    "subscribe to this journal",
    "subscribe to read",
    "subscribe to continue",
    "subscribe now",
    "rent or buy this article",
    "buy this article",
    "buy article",
    "purchase this article",
    "purchase access",
    "access through your institution",
    "get full access",
    "get access",
    "prices may be subject to local taxes",
    "already a subscriber",
    "this is a preview of subscription content",
    "to read the full article",
    "to continue reading",
    "unlock this article",
    "subscribers only",
    "you've reached your limit",
    "free articles remaining",
];

const LOGIN: &[&str] = &[
    "sign in",
    "sign-in",
    "log in",
    "log-in",
    "login",
    "create an account",
    "create account",
    "forgot password",
    "forgot your password",
    "password",
    "remember me",
    "continue with google",
    "continue with apple",
    "don't have an account",
    "register for free",
    "sign up",
];

const COOKIE: &[&str] = &[
    "cookie",
    "cookies",
    "accept all",
    "reject all",
    "manage preferences",
    "cookie settings",
    "consent",
    "we value your privacy",
    "privacy preferences",
    "necessary cookies",
];

/// Lines that are bibliography, not prose.
const CITATION: &[&str] = &["google scholar", "pubmed", "crossref", "doi.org", "article cas", "mathscinet"];

fn count(haystack: &str, needles: &[&str]) -> usize {
    needles.iter().filter(|n| contains_phrase(haystack, n)).count()
}

/// Phrase match on word boundaries ("login" must not match "blogin").
fn contains_phrase(haystack: &str, needle: &str) -> bool {
    let mut from = 0;
    while let Some(i) = haystack[from..].find(needle) {
        let start = from + i;
        let end = start + needle.len();
        let before = haystack[..start].chars().next_back();
        let after = haystack[end..].chars().next();
        let boundary = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric());
        if boundary(before) && boundary(after) {
            return true;
        }
        from = start + needle.len().max(1);
        while !haystack.is_char_boundary(from) {
            from += 1;
        }
    }
    false
}

/// Replace markdown links/images with their text; return (text, link chars).
fn strip_links(line: &str) -> (String, usize) {
    let mut out = String::with_capacity(line.len());
    let mut link_chars = 0;
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        let Some(close_rel) = rest[open..].find("](") else { break };
        let close = open + close_rel;
        let Some(paren_rel) = rest[close + 2..].find(')') else { break };
        let end = close + 2 + paren_rel + 1;
        let prefix = &rest[..open];
        out.push_str(prefix.strip_suffix('!').unwrap_or(prefix));
        let text = &rest[open + 1..close];
        out.push_str(text);
        link_chars += end - open;
        rest = &rest[end..];
    }
    out.push_str(rest);
    (out, link_chars)
}

fn words(s: &str) -> usize {
    s.split_whitespace().filter(|w| w.chars().any(char::is_alphanumeric)).count()
}

/// Screen extracted text. `Ok(())` means it reads like a document.
pub fn screen(doc: &Extracted) -> Result<(), Rejected> {
    let title = doc.title.to_lowercase();
    let mut prose_words = 0;
    let (mut paywall, mut login, mut cookie) = (0, 0, 0);
    for page in &doc.pages {
        for raw in page.lines() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            let lower = line.to_lowercase();
            let (p, l, c) = (count(&lower, PAYWALL), count(&lower, LOGIN), count(&lower, COOKIE));
            paywall += p;
            login += l;
            cookie += c;
            if p + l + c > 0 && words(line) < 40 {
                continue; // a gate line, not prose
            }
            if line.starts_with('#') || line.starts_with("![") || line.starts_with('|') {
                continue;
            }
            if CITATION.iter().any(|m| lower.contains(m)) {
                continue;
            }
            let (text, link_chars) = strip_links(line);
            if link_chars * 2 > line.len() {
                continue; // navigation: mostly links
            }
            let n = words(&text);
            if n < 5 {
                continue; // menu items, bylines, buttons
            }
            prose_words += n;
        }
    }
    let title_gate = |list: &[&str]| list.iter().any(|p| title.trim() == *p || title.starts_with(p));
    let reject = |reason| Rejected {
        reason,
        title: doc.title.clone(),
        origin: doc.origin.clone(),
        prose_words,
        paywall_hits: paywall,
        login_hits: login,
        cookie_hits: cookie,
    };

    if title_gate(PAYWALL) || (paywall >= 2 && prose_words < PAYWALL_PROSE_WORDS) {
        return Err(reject(RejectReason::Paywall));
    }
    if title_gate(&["sign in", "log in", "login", "sign up", "create an account"]) && prose_words < PAYWALL_PROSE_WORDS {
        return Err(reject(RejectReason::LoginWall));
    }
    if prose_words < MIN_PROSE_WORDS {
        // Name the gate that dominates the page, if any.
        let reason = [(paywall, RejectReason::Paywall), (login, RejectReason::LoginWall), (cookie, RejectReason::CookieWall)]
            .into_iter()
            .filter(|(hits, _)| *hits >= 2)
            .max_by_key(|(hits, _)| *hits)
            .map(|(_, r)| r)
            .unwrap_or(RejectReason::TooThin);
        return Err(reject(reason));
    }
    Ok(())
}
