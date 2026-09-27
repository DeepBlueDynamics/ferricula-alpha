//! Cut extracted text into addressable sections. Every section's text is
//! an exact substring of its page, so a citation can always be quoted
//! verbatim (the evidence plane never paraphrases).

use serde::{Deserialize, Serialize};

use crate::extract::Extracted;

/// Soft upper bound on a section's size in bytes. Paragraphs are packed up
/// to this size; a single longer paragraph is split on line, then char,
/// boundaries.
pub const MAX_SECTION_BYTES: usize = 2000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocSection {
    pub index: u32,
    /// Nearest markdown heading, or "p. N" for paged sources.
    pub heading: String,
    pub text: String,
    /// 1-based page number for paged sources.
    pub page: Option<u32>,
    /// Byte offset of `text` inside its page.
    pub offset: usize,
}

pub fn sectionize(doc: &Extracted) -> Vec<DocSection> {
    let mut out = Vec::new();
    for (page_index, page) in doc.pages.iter().enumerate() {
        let page_no = doc.paged.then(|| page_index as u32 + 1);
        for (heading, start, end) in heading_blocks(page, doc.paged) {
            let heading = match page_no {
                Some(n) if heading.is_empty() => format!("p. {n}"),
                Some(n) => format!("{heading} (p. {n})"),
                None if heading.is_empty() => doc.title.clone(),
                None => heading,
            };
            for (s, e) in pack(page, start, end) {
                let text = &page[s..e];
                if text.trim().is_empty() {
                    continue;
                }
                out.push(DocSection {
                    index: out.len() as u32,
                    heading: heading.clone(),
                    text: text.to_string(),
                    page: page_no,
                    offset: s,
                });
            }
        }
    }
    out
}

/// Split a page into (heading, start, end) blocks at markdown headings.
/// Paged (PDF) text has no reliable markdown, so it is one block.
fn heading_blocks(page: &str, paged: bool) -> Vec<(String, usize, usize)> {
    if paged {
        return vec![(String::new(), 0, page.len())];
    }
    let mut blocks = Vec::new();
    let mut heading = String::new();
    let mut start = 0;
    let mut pos = 0;
    for line in page.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let text = trimmed.trim_start_matches('#').trim();
            if !text.is_empty() {
                if pos > start {
                    blocks.push((heading.clone(), start, pos));
                }
                heading = text.to_string();
                start = pos;
            }
        }
        pos += line.len();
    }
    if page.len() > start {
        blocks.push((heading, start, page.len()));
    }
    blocks
}

/// Pack paragraphs of `text[start..end]` into ranges of at most
/// MAX_SECTION_BYTES, splitting oversized paragraphs.
fn pack(text: &str, start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut cur_start = start;
    let mut cur_end = start;
    for (p_start, p_end) in paragraphs(text, start, end) {
        if p_end - p_start > MAX_SECTION_BYTES {
            if cur_end > cur_start {
                ranges.push((cur_start, cur_end));
            }
            ranges.extend(split_long(text, p_start, p_end));
            cur_start = p_end;
            cur_end = p_end;
        } else if p_end - cur_start > MAX_SECTION_BYTES && cur_end > cur_start {
            ranges.push((cur_start, cur_end));
            cur_start = p_start;
            cur_end = p_end;
        } else {
            cur_end = p_end;
        }
    }
    if cur_end > cur_start {
        ranges.push((cur_start, cur_end));
    }
    ranges
}

/// Paragraph ranges separated by blank lines; ranges tile [start, end).
fn paragraphs(text: &str, start: usize, end: usize) -> Vec<(usize, usize)> {
    let slice = &text[start..end];
    let mut out = Vec::new();
    let mut p_start = 0;
    let mut search = 0;
    while let Some(found) = slice[search..].find("\n\n") {
        let mut boundary = search + found + 2;
        while slice[boundary..].starts_with('\n') {
            boundary += 1;
        }
        out.push((start + p_start, start + boundary));
        p_start = boundary;
        search = boundary;
    }
    if p_start < slice.len() {
        out.push((start + p_start, end));
    }
    out
}

fn split_long(text: &str, start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut s = start;
    while end - s > MAX_SECTION_BYTES {
        let mut cut = s + MAX_SECTION_BYTES;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        // Prefer a line or sentence break in the back half of the window.
        let window = &text[s..cut];
        if let Some(i) = window.rfind('\n').or_else(|| window.rfind(". ").map(|i| i + 1)) {
            if i > MAX_SECTION_BYTES / 2 {
                cut = s + i + 1;
            }
        }
        out.push((s, cut));
        s = cut;
    }
    if end > s {
        out.push((s, end));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(pages: Vec<&str>, paged: bool) -> Extracted {
        Extracted {
            title: "Doc".into(),
            origin: "test".into(),
            pages: pages.into_iter().map(String::from).collect(),
            paged,
        }
    }

    #[test]
    fn markdown_sections_follow_headings_and_are_verbatim() {
        let text = "preamble line\n\n# One\nalpha beta\n\n## Two\ngamma\n";
        let d = doc(vec![text], false);
        let secs = sectionize(&d);
        assert_eq!(secs.iter().map(|s| s.heading.as_str()).collect::<Vec<_>>(), ["Doc", "One", "Two"]);
        for s in &secs {
            assert_eq!(&text[s.offset..s.offset + s.text.len()], s.text);
        }
    }

    #[test]
    fn pages_are_numbered_and_long_text_is_split() {
        let long = "word ".repeat(1000);
        let d = doc(vec!["first page", &long], true);
        let secs = sectionize(&d);
        assert_eq!(secs[0].page, Some(1));
        assert!(secs.iter().filter(|s| s.page == Some(2)).count() >= 3);
        assert!(secs.iter().all(|s| s.text.len() <= MAX_SECTION_BYTES));
        let rebuilt: String = secs.iter().filter(|s| s.page == Some(2)).map(|s| s.text.as_str()).collect();
        assert_eq!(rebuilt, long);
    }

    #[test]
    fn multibyte_text_splits_on_char_boundaries() {
        let text = "諸行無常".repeat(400);
        let d = doc(vec![&text], true);
        let rebuilt: String = sectionize(&d).iter().map(|s| s.text.as_str()).collect();
        assert_eq!(rebuilt, text);
    }
}
