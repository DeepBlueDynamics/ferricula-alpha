//! Source extraction: text, web pages (through grub), and PDF bytes become
//! plain text, one entry per page when the source has pages.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

/// Something the agent was handed to read.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Source {
    /// Inline text or markdown.
    Text { title: Option<String>, text: String },
    /// A web page or remote PDF.
    Url { url: String },
    /// PDF bytes (the API layer decodes base64).
    Pdf { name: String, bytes: Vec<u8> },
}

impl Source {
    pub fn kind(&self) -> &'static str {
        match self {
            Source::Text { .. } => "text",
            Source::Url { .. } => "url",
            Source::Pdf { .. } => "pdf",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractConfig {
    /// grub crawler base URL (renders JS pages and returns markdown).
    pub grub_base_url: String,
    pub timeout_secs: u64,
    /// Upper bound on fetched or supplied bytes.
    pub max_bytes: usize,
}

impl Default for ExtractConfig {
    fn default() -> Self {
        Self {
            grub_base_url: "http://127.0.0.1:6792".into(),
            timeout_secs: 90,
            max_bytes: 32 * 1024 * 1024,
        }
    }
}

/// Extracted text. `pages` has one entry per PDF page, or a single entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extracted {
    pub title: String,
    /// Where it came from: the final URL or the supplied file name.
    pub origin: String,
    pub pages: Vec<String>,
    pub paged: bool,
}

pub fn extract(source: &Source, config: &ExtractConfig) -> Result<Extracted> {
    match source {
        Source::Text { title, text } => {
            if text.len() > config.max_bytes {
                bail!("text exceeds {} bytes", config.max_bytes);
            }
            let title = title.clone().filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| first_heading(text).unwrap_or_else(|| "Untitled text".into()));
            Ok(Extracted { title, origin: "inline".into(), pages: vec![text.clone()], paged: false })
        }
        Source::Pdf { name, bytes } => {
            if bytes.len() > config.max_bytes {
                bail!("pdf exceeds {} bytes", config.max_bytes);
            }
            let pages = pdf_pages(bytes)?;
            Ok(Extracted { title: pdf_title(name, &pages), origin: name.clone(), pages, paged: true })
        }
        Source::Url { url } => {
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                bail!("url must be http(s)");
            }
            if looks_like_pdf_url(url) {
                let bytes = fetch_bytes(url, config)?;
                let pages = pdf_pages(&bytes)?;
                let name = url.rsplit('/').next().unwrap_or(url).to_string();
                return Ok(Extracted { title: pdf_title(&name, &pages), origin: url.clone(), pages, paged: true });
            }
            let (markdown, final_url) = grub_markdown(url, config)?;
            let title = first_heading(&markdown).unwrap_or_else(|| final_url.clone());
            Ok(Extracted { title, origin: final_url, pages: vec![markdown], paged: false })
        }
    }
}

/// PDF text per page. pdf-extract can panic on malformed files, so the
/// call is isolated and a panic becomes an error.
pub fn pdf_pages(bytes: &[u8]) -> Result<Vec<String>> {
    if !bytes.starts_with(b"%PDF") {
        bail!("not a PDF (missing %PDF header)");
    }
    let owned = bytes.to_vec();
    let result = std::panic::catch_unwind(move || pdf_extract::extract_text_from_mem_by_pages(&owned));
    let pages = match result {
        Ok(Ok(pages)) => pages,
        Ok(Err(error)) => bail!("pdf extraction failed: {error}"),
        Err(_) => bail!("pdf extraction failed: malformed document"),
    };
    if pages.iter().all(|p| p.trim().is_empty()) {
        bail!("pdf has no extractable text (scanned images need OCR)");
    }
    Ok(pages)
}

fn pdf_title(name: &str, pages: &[String]) -> String {
    // The first non-trivial line of page one is usually the title.
    pages.first()
        .and_then(|p| p.lines().map(str::trim).find(|l| l.len() >= 8 && l.len() <= 200))
        .map(str::to_string)
        .unwrap_or_else(|| name.to_string())
}

fn first_heading(text: &str) -> Option<String> {
    text.lines().map(str::trim).find(|l| l.starts_with('#'))
        .map(|l| l.trim_start_matches('#').trim().to_string())
        .filter(|l| !l.is_empty())
}

fn looks_like_pdf_url(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or(url).to_ascii_lowercase();
    path.ends_with(".pdf") || path.contains("arxiv.org/pdf/")
}

fn agent(config: &ExtractConfig) -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(Duration::from_secs(config.timeout_secs)).build()
}

fn fetch_bytes(url: &str, config: &ExtractConfig) -> Result<Vec<u8>> {
    let response = agent(config).get(url).call().with_context(|| format!("fetch {url}"))?;
    let mut bytes = Vec::new();
    use std::io::Read;
    response.into_reader().take(config.max_bytes as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > config.max_bytes {
        bail!("{url} exceeds {} bytes", config.max_bytes);
    }
    Ok(bytes)
}

fn grub_markdown(url: &str, config: &ExtractConfig) -> Result<(String, String)> {
    let endpoint = format!("{}/api/markdown", config.grub_base_url.trim_end_matches('/'));
    let body: serde_json::Value = agent(config).post(&endpoint)
        .send_json(serde_json::json!({ "url": url }))
        .with_context(|| format!("grub {endpoint}"))?
        .into_json()?;
    if body.get("success").and_then(|v| v.as_bool()) != Some(true) {
        let reason = body.get("block_reason").and_then(|v| v.as_str())
            .or_else(|| body.get("error").and_then(|v| v.as_str()))
            .unwrap_or("unknown");
        bail!("grub could not read {url}: {reason}");
    }
    let markdown = body.get("markdown").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    if markdown.trim().is_empty() {
        bail!("grub returned no text for {url}");
    }
    if markdown.len() > config.max_bytes {
        bail!("{url} exceeds {} bytes", config.max_bytes);
    }
    let final_url = body.get("final_url").and_then(|v| v.as_str()).unwrap_or(url).to_string();
    Ok((markdown, final_url))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_title_falls_back_to_heading() {
        let src = Source::Text { title: None, text: "intro\n# The Title\nbody".into() };
        let out = extract(&src, &ExtractConfig::default()).unwrap();
        assert_eq!(out.title, "The Title");
        assert!(!out.paged);
    }

    #[test]
    fn rejects_non_pdf_bytes() {
        assert!(pdf_pages(b"hello").is_err());
    }

    #[test]
    fn pdf_url_detection() {
        assert!(looks_like_pdf_url("https://arxiv.org/pdf/2609.26550"));
        assert!(looks_like_pdf_url("https://x.org/a/paper.PDF?download=1"));
        assert!(!looks_like_pdf_url("https://x.org/a/page.html"));
    }

    #[test]
    fn rejects_non_http_url() {
        let src = Source::Url { url: "file:///etc/passwd".into() };
        assert!(extract(&src, &ExtractConfig::default()).is_err());
    }
}
