//! Dream → picture: the instruction that turns dream prose into a single
//! visual scene, and the sanitizer that runs on the model's answer before it
//! reaches an image model. Pure; no I/O.
//!
//! The picture must not name real people (the image model would try to draw a
//! likeness) and must not contain words (Qwen Image renders legible text, so
//! prose in the prompt leaks onto the canvas).

/// Negative prompt for every dream render.
pub const DREAM_NEGATIVE_PROMPT: &str = "text, letters, writing, caption, watermark, logo, signature";
/// Target length of a visual prompt, in words.
pub const VISUAL_PROMPT_MIN_WORDS: usize = 40;
pub const VISUAL_PROMPT_MAX_WORDS: usize = 60;
/// Hard cap applied by [`sanitize_visual_prompt`].
pub const VISUAL_PROMPT_MAX_CHARS: usize = 480;
/// What a removed name becomes.
const NAME_STAND_IN: &str = "someone";

/// System instruction for the model that turns a dream into a picture prompt.
pub const DREAM_VISUAL_SYSTEM: &str = "You turn a dream into one picture. \
Answer with a single image-generation prompt of 40 to 60 words describing ONE still scene: \
a concrete setting, the light (time of day, source, colour), the key objects and their materials, \
and at most one or two figures described only by pose and clothing. \
Rules: no names of any people, places that identify a person, brands or companies; \
no written words, signs, labels, captions, screens with text, letters or numbers anywhere in the scene; \
no quotation marks; no story, no feelings, no explanation. \
Present tense, comma-separated visual phrases. Output only the prompt.";

/// The user message for [`DREAM_VISUAL_SYSTEM`]: the dream prose, fenced.
pub fn dream_visual_prompt(dream: &str) -> String {
    format!(
        "Dream:\n<<<\n{}\n>>>\n\nWrite the {VISUAL_PROMPT_MIN_WORDS}-{VISUAL_PROMPT_MAX_WORDS} word visual prompt for the single most vivid scene in this dream.",
        dream.trim()
    )
}

/// System instruction and user message combined, for single-string callers.
pub fn dream_visual_instruction(dream: &str) -> String {
    format!("{DREAM_VISUAL_SYSTEM}\n\n{}", dream_visual_prompt(dream))
}

/// Clean a model-written visual prompt before rendering:
/// - every name in `forbidden_names` (case-insensitive, whole words, with a
///   possessive `'s`) becomes "someone"; the capitalised parts of multi-word
///   names ("Steve Jobs" → "Steve", "Jobs") are removed too;
/// - quotation marks are dropped (apostrophes inside words are kept), as are
///   control characters and a leading "Prompt:" label;
/// - whitespace is collapsed and the result is capped at
///   [`VISUAL_PROMPT_MAX_WORDS`] words and [`VISUAL_PROMPT_MAX_CHARS`] chars.
pub fn sanitize_visual_prompt(prompt: &str, forbidden_names: &[&str]) -> String {
    let mut text: String = prompt.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let trimmed = text.trim_start();
    for label in ["prompt:", "visual prompt:", "image prompt:"] {
        if trimmed.get(..label.len()).is_some_and(|head| head.eq_ignore_ascii_case(label)) {
            text = trimmed[label.len()..].to_string();
            break;
        }
    }
    text = strip_quotes(&text);

    // Full names first (longest first so "Steve Jobs" wins over "Steve"),
    // then the capitalised parts of multi-word names.
    let mut names: Vec<&str> = forbidden_names.iter().map(|n| n.trim()).filter(|n| !n.is_empty()).collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.chars().count()));
    for name in &names {
        text = replace_word(&text, name, false, NAME_STAND_IN);
    }
    for name in &names {
        let parts: Vec<&str> = name.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        for part in parts {
            if part.chars().count() >= 2 && part.chars().next().is_some_and(char::is_uppercase) {
                text = replace_word(&text, part, true, NAME_STAND_IN);
            }
        }
    }
    text = collapse_stand_ins(&text);

    let words: Vec<&str> = text.split_whitespace().take(VISUAL_PROMPT_MAX_WORDS).collect();
    let mut out = String::new();
    for w in words {
        let extra = if out.is_empty() { w.chars().count() } else { w.chars().count() + 1 };
        if out.chars().count() + extra > VISUAL_PROMPT_MAX_CHARS {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(w);
    }
    out.trim_end_matches([',', ';', ':', '-', ' ']).to_string()
}

/// Capitalised words that do not start a sentence: candidate proper nouns
/// in persona files and traces, for [`sanitize_visual_prompt`]'s list.
/// Deliberately over-inclusive (a stray capitalised noun only becomes
/// "someone"); common sentence words are skipped.
pub fn candidate_names(text: &str) -> Vec<String> {
    const SKIP: &[&str] = &[
        "I", "I'm", "I've", "I'd", "I'll", "The", "A", "An", "And", "But", "Or", "If", "In", "On", "At", "To", "Of",
        "It", "He", "She", "They", "We", "You", "My", "His", "Her", "Their", "Our", "Your", "This", "That", "These",
        "Those", "When", "Then", "There", "Here", "What", "Why", "How", "Who", "Yes", "No", "Not",
    ];
    let mut out: Vec<String> = Vec::new();
    let mut sentence_start = true;
    for raw in text.split_whitespace() {
        let word: String = raw
            .trim_matches(|c: char| !c.is_alphanumeric())
            .trim_end_matches("'s")
            .trim_end_matches("\u{2019}s")
            .to_string();
        let ends_sentence = raw.ends_with(['.', '!', '?', ':', '\n']) || raw.ends_with(".\"");
        let capitalised = word.chars().next().is_some_and(char::is_uppercase)
            && word.chars().skip(1).any(char::is_lowercase)
            && word.chars().count() >= 2;
        if capitalised && !sentence_start && !SKIP.contains(&word.as_str()) && !out.contains(&word) {
            out.push(word);
        }
        sentence_start = ends_sentence;
    }
    out
}

fn is_quote(c: char) -> bool {
    matches!(c, '"' | '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{00AB}' | '\u{00BB}' | '`')
}

fn is_single_quote(c: char) -> bool {
    matches!(c, '\'' | '\u{2018}' | '\u{2019}')
}

/// Drop double quotes everywhere and single quotes unless they sit between
/// two letters (an apostrophe: "Dad's", "don't").
fn strip_quotes(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        if is_quote(c) {
            continue;
        }
        if is_single_quote(c) {
            let prev = i.checked_sub(1).and_then(|j| chars.get(j)).is_some_and(|p| p.is_alphanumeric());
            let next = chars.get(i + 1).is_some_and(|n| n.is_alphanumeric());
            if !(prev && next) {
                continue;
            }
            out.push('\'');
            continue;
        }
        out.push(c);
    }
    out
}

/// Replace whole-word occurrences of `word` (may contain spaces), with an
/// optional possessive `'s` kept on the stand-in.
fn replace_word(text: &str, word: &str, case_sensitive: bool, with: &str) -> String {
    let fold = |c: char| if case_sensitive { c } else { c.to_lowercase().next().unwrap_or(c) };
    let hay: Vec<char> = text.chars().collect();
    let needle: Vec<char> = word.chars().map(fold).collect();
    if needle.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < hay.len() {
        let fits = i + needle.len() <= hay.len()
            && hay[i..i + needle.len()].iter().zip(&needle).all(|(h, n)| fold(*h) == *n)
            && (i == 0 || !hay[i - 1].is_alphanumeric())
            && hay.get(i + needle.len()).is_none_or(|c| !c.is_alphanumeric());
        if fits {
            out.push_str(with);
            i += needle.len();
        } else {
            out.push(hay[i]);
            i += 1;
        }
    }
    out
}

/// "someone someone" / "someone and someone" → "someone" / "two figures".
fn collapse_stand_ins(text: &str) -> String {
    let mut s = text.to_string();
    loop {
        let next = s
            .replace("someone someone", NAME_STAND_IN)
            .replace("someone and someone", "two figures");
        if next == s {
            return s;
        }
        s = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DREAM: &str = "Tomas is in the boathouse on Kelp Lane. He is holding a stone. Pocket-sized, warm, no screen. \
        He holds it to his ear the way you hold a shell. On Dad's workbench the proteins are folding.";

    #[test]
    fn instruction_carries_the_dream_and_the_rules() {
        let p = dream_visual_instruction(DREAM);
        assert!(p.contains("Kelp Lane"));
        assert!(p.contains("40 to 60 words"));
        assert!(p.contains("no names"));
        assert!(p.contains("no written words"));
        assert!(dream_visual_prompt("  x  ").contains("<<<\nx\n>>>"));
    }

    #[test]
    fn removes_names_and_quotes() {
        let raw = "\"Jony and Steve Jobs stand in a garage; Jony's hand holds a warm stone, Jobs watches, 'folding proteins' glow on Dad's workbench\"";
        let out = sanitize_visual_prompt(raw, &["Steve Jobs", "Jony"]);
        for banned in ["Jony", "Steve", "Jobs", "\"", "'folding"] {
            assert!(!out.contains(banned), "{banned} in {out}");
        }
        assert!(out.starts_with("two figures stand in a garage"), "{out}");
        assert!(out.contains("someone's hand"), "{out}");
        assert!(out.contains("Dad's workbench"), "apostrophes survive: {out}");
    }

    #[test]
    fn name_match_is_whole_word_and_case_insensitive() {
        let out = sanitize_visual_prompt("JONY at dawn, Jonyesque light, odd jobs, jony's shell", &["Jony", "Steve Jobs"]);
        assert_eq!(out, "someone at dawn, Jonyesque light, odd jobs, someone's shell");
    }

    #[test]
    fn caps_words_and_chars_and_strips_labels() {
        let long = format!("Prompt: {}", "stone ".repeat(200));
        let out = sanitize_visual_prompt(&long, &[]);
        assert_eq!(out.split_whitespace().count(), VISUAL_PROMPT_MAX_WORDS);
        assert!(!out.to_lowercase().contains("prompt"));
        let wide = "abcdefghijklmnopqrstuvwxyz ".repeat(40);
        assert!(sanitize_visual_prompt(&wide, &[]).chars().count() <= VISUAL_PROMPT_MAX_CHARS);
        assert_eq!(sanitize_visual_prompt("a\nb\tc,  ", &[]), "a b c");
    }

    #[test]
    fn candidate_names_skip_sentence_starts() {
        let names = candidate_names(DREAM);
        assert!(names.contains(&"Kelp".to_string()) && names.contains(&"Lane".to_string()), "{names:?}");
        assert!(names.contains(&"Dad".to_string()), "{names:?}");
        assert!(!names.contains(&"He".to_string()) && !names.contains(&"Pocket-sized".to_string()), "{names:?}");
        let names = candidate_names("I met Steve Jobs and Jony Ive in Palo Alto.");
        for n in ["Steve", "Jobs", "Jony", "Ive", "Palo", "Alto"] {
            assert!(names.contains(&n.to_string()), "{n}: {names:?}");
        }
    }

    #[test]
    fn negative_prompt_bans_text() {
        for w in ["text", "letters", "writing", "caption", "watermark", "logo", "signature"] {
            assert!(DREAM_NEGATIVE_PROMPT.contains(w));
        }
    }
}
