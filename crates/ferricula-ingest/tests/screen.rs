//! The junk-page screen on realistic web-page markdown (as grub returns it).

use ferricula_ingest::{
    ExtractConfig, Extracted, RejectReason, Rejected, Source, extract, extract_screened, rejection, screen,
};

fn page(fixture: &str) -> Extracted {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(fixture);
    let text = std::fs::read_to_string(path).unwrap();
    extract(&Source::Text { title: None, text, origin: None }, &ExtractConfig::default()).unwrap()
}

fn rejected(fixture: &str) -> Rejected {
    screen(&page(fixture)).expect_err("fixture should be rejected")
}

#[test]
fn nature_access_options_page_is_a_paywall() {
    let r = rejected("nature_access_options.md");
    assert_eq!(r.reason, RejectReason::Paywall, "{r}");
    assert!(r.paywall_hits >= 3, "{r}");
    assert!(r.prose_words < 400, "{r}");
}

#[test]
fn login_wall_is_rejected() {
    let r = rejected("login_wall.md");
    assert_eq!(r.reason, RejectReason::LoginWall, "{r}");
}

#[test]
fn real_article_passes_despite_subscribe_and_cookie_chrome() {
    let doc = page("real_article.md");
    assert_eq!(doc.title, "Why sleep keeps some memories and lets others go");
    screen(&doc).unwrap();
}

#[test]
fn cookie_interstitial_is_rejected() {
    let text = "# We value your privacy\n\nWe and our partners use cookies to store and access information on your device.\n\n\
                [Accept all](#a) [Reject all](#r)\n\nManage preferences\n\nNecessary cookies are always on.\n";
    let doc = extract(&Source::Text { title: None, text: text.into(), origin: None }, &ExtractConfig::default()).unwrap();
    assert_eq!(screen(&doc).unwrap_err().reason, RejectReason::CookieWall);
}

#[test]
fn thin_page_is_too_thin() {
    let text = "# Home\n\n- [About](/about)\n- [Blog](/blog)\n\nWelcome to my site. More soon.\n";
    let doc = extract(&Source::Text { title: None, text: text.into(), origin: None }, &ExtractConfig::default()).unwrap();
    assert_eq!(screen(&doc).unwrap_err().reason, RejectReason::TooThin);
}

#[test]
fn rejection_is_typed_through_anyhow() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/nature_access_options.md");
    let text = std::fs::read_to_string(path).unwrap();
    let err = extract_screened(&Source::Text { title: None, text, origin: None }, &ExtractConfig::default()).unwrap_err();
    let r = rejection(&err).expect("typed rejection");
    assert_eq!(r.reason, RejectReason::Paywall);
    assert!(err.to_string().starts_with("rejected: paywall"), "{err}");
    // Serialized form for journals / API responses.
    let json = serde_json::to_value(r).unwrap();
    assert_eq!(json["reason"], "paywall");
}

#[test]
fn inline_text_is_not_screened_by_plain_extract() {
    // Operators hand over short notes on purpose; only web pages are screened.
    let src = Source::Text { title: None, text: "# Note\nshort".into(), origin: None };
    assert!(extract(&src, &ExtractConfig::default()).is_ok());
    assert!(extract_screened(&src, &ExtractConfig::default()).is_err());
}
