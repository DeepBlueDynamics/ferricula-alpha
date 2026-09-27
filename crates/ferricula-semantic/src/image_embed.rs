//! Image embedding backends behind one trait, available without the `ml`
//! feature (HTTP only).
//!
//! Like [`crate::text_embed`], a backend names its **space**
//! (`"<model>@<dim>"`, e.g. `siglip-base-patch16-224@768`). An image space is
//! never comparable with a text space even when the dimensions agree: callers
//! tag every stored vector with the space that produced it.

use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail, ensure};
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::text_embed::{NORM_TOLERANCE, parse_space};

/// Default image space: shivvr's SigLIP vision tower.
pub const DEFAULT_IMAGE_SPACE: &str = "siglip-base-patch16-224@768";
/// Largest raw image accepted. shivvr's JSON body limit is axum's default
/// 2 MiB; base64 inflates by 4/3, so 1.5 MB raw stays under it. Larger images
/// should be re-encoded (e.g. a JPEG preview) before embedding.
pub const SHIVVR_MAX_IMAGE_BYTES: usize = 1_500_000;

/// Remote or local image embedding.
pub trait ImageEmbedder: Send + Sync {
    /// Embedding space id, `"<model>@<dim>"`.
    fn space(&self) -> &str;
    /// Output dimension.
    fn dim(&self) -> usize;
    /// One unit vector for an encoded image (PNG, JPEG or WebP bytes).
    fn embed(&self, image: &[u8]) -> Result<Vec<f32>>;
}

/// Sniff the container format from magic bytes.
pub fn image_format(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpeg")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

#[derive(Serialize)]
struct ImageEmbedRequest<'a> {
    image_base64: &'a str,
}

#[derive(Deserialize)]
struct ImageEmbedResponse {
    embedding: Vec<f32>,
    dimension: usize,
    model: String,
}

/// shivvr `POST {url}/image/embed` over blocking HTTP. No retries.
pub struct ShivvrImageEmbedder {
    url: String,
    space: String,
    model: String,
    dim: usize,
    agent: ureq::Agent,
}

impl ShivvrImageEmbedder {
    /// `url` is the service base (e.g. `http://127.0.0.1:8085`); `space` is
    /// `"<model>@<dim>"` and must match what shivvr reports.
    pub fn new(url: &str, space: &str, timeout: Duration) -> Result<Self> {
        let url = url.trim().trim_end_matches('/').to_string();
        ensure!(
            url.starts_with("http://") || url.starts_with("https://"),
            "shivvr url must be http(s)"
        );
        let (model, dim) = parse_space(space)?;
        ensure!(!timeout.is_zero(), "shivvr timeout must be positive");
        let agent = ureq::AgentBuilder::new().timeout(timeout).build();
        Ok(Self { url, space: space.to_string(), model: model.to_string(), dim, agent })
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}

impl ImageEmbedder for ShivvrImageEmbedder {
    fn space(&self) -> &str {
        &self.space
    }

    fn dim(&self) -> usize {
        self.dim
    }

    fn embed(&self, image: &[u8]) -> Result<Vec<f32>> {
        ensure!(!image.is_empty(), "image is empty");
        ensure!(
            image_format(image).is_some(),
            "image is not PNG, JPEG or WebP (magic bytes {:02x?})",
            &image[..image.len().min(8)]
        );
        ensure!(
            image.len() <= SHIVVR_MAX_IMAGE_BYTES,
            "image is {} bytes (shivvr limit {SHIVVR_MAX_IMAGE_BYTES}); re-encode smaller",
            image.len()
        );
        let encoded = base64::engine::general_purpose::STANDARD.encode(image);
        let endpoint = format!("{}/image/embed", self.url);
        let body = ImageEmbedRequest { image_base64: &encoded };
        let response = match self.agent.post(&endpoint).send_json(&body) {
            Ok(response) => response,
            Err(ureq::Error::Status(code, response)) => {
                let detail = response.into_string().unwrap_or_default();
                let detail: String = detail.chars().take(300).collect();
                bail!("shivvr /image/embed returned HTTP {code}: {detail}");
            }
            Err(err) => return Err(anyhow!("shivvr /image/embed unreachable at {endpoint}: {err}")),
        };
        let parsed: ImageEmbedResponse = response
            .into_json()
            .context("shivvr /image/embed returned malformed JSON")?;
        ensure!(
            parsed.model == self.model,
            "shivvr /image/embed answered with model {:?}, expected {:?}",
            parsed.model,
            self.model
        );
        ensure!(
            parsed.dimension == self.dim && parsed.embedding.len() == self.dim,
            "shivvr /image/embed reports dim {} ({} values), space {} expects {}",
            parsed.dimension,
            parsed.embedding.len(),
            self.space,
            self.dim
        );
        let v = parsed.embedding;
        ensure!(v.iter().all(|x| x.is_finite()), "image vector has non-finite values");
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        ensure!((norm - 1.0).abs() <= NORM_TOLERANCE, "image vector is not unit length (norm {norm})");
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake-png-body";

    fn unit(seed: usize, dim: usize) -> Vec<f32> {
        let mut v: Vec<f32> = (0..dim).map(|i| ((seed * 31 + i * 7) % 17) as f32 + 1.0).collect();
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter_mut().for_each(|x| *x /= n);
        v
    }

    #[derive(Clone, Copy)]
    enum Mode {
        Good,
        WrongModel,
        WrongDim,
        NotUnit,
        Status503,
    }

    /// Tiny shivvr: `POST /image/embed`, records the decoded image bytes.
    fn fake_shivvr(mode: Mode) -> (String, Arc<Mutex<Vec<(String, Vec<u8>)>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut length = 0usize;
                let mut path = String::new();
                let mut line = String::new();
                loop {
                    line.clear();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        break;
                    }
                    let l = line.trim_end();
                    if l.is_empty() {
                        break;
                    }
                    if path.is_empty() {
                        path = l.split_whitespace().nth(1).unwrap_or("").to_string();
                    }
                    if let Some(v) = l.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0u8; length];
                let _ = reader.read_exact(&mut body);
                let request: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                let image = base64::engine::general_purpose::STANDARD
                    .decode(request["image_base64"].as_str().unwrap_or(""))
                    .unwrap_or_default();
                let seed = image.len();
                log.lock().unwrap().push((path, image));
                let (status, payload) = match mode {
                    Mode::Status503 => ("503 Service Unavailable", json!({"error": "Vision embedder not loaded"})),
                    _ => {
                        let dim = if matches!(mode, Mode::WrongDim) { 4 } else { 8 };
                        let mut v = unit(seed, dim);
                        if matches!(mode, Mode::NotUnit) {
                            v.iter_mut().for_each(|x| *x *= 2.0);
                        }
                        let model = if matches!(mode, Mode::WrongModel) { "clip" } else { "tiny-vision" };
                        ("200 OK", json!({"embedding": v, "dimension": dim, "model": model}))
                    }
                };
                let payload = payload.to_string();
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    payload.len(),
                    payload
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        (format!("http://{addr}"), seen)
    }

    fn embedder(url: &str) -> ShivvrImageEmbedder {
        ShivvrImageEmbedder::new(url, "tiny-vision@8", Duration::from_secs(5)).unwrap()
    }

    #[test]
    fn embeds_and_sends_base64_image() {
        let (url, seen) = fake_shivvr(Mode::Good);
        let e = embedder(&format!("{url}/"));
        assert_eq!(e.space(), "tiny-vision@8");
        assert_eq!(e.dim(), 8);
        let v = e.embed(PNG).unwrap();
        assert_eq!(v.len(), 8);
        assert!(crate::text_embed::cosine(&v, &unit(PNG.len(), 8)) > 0.9999);
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, "/image/embed");
        assert_eq!(seen[0].1, PNG, "server must receive the exact bytes");
    }

    #[test]
    fn rejects_bad_responses() {
        for (mode, needle) in [
            (Mode::WrongModel, "model"),
            (Mode::WrongDim, "dim"),
            (Mode::NotUnit, "unit length"),
            (Mode::Status503, "HTTP 503"),
        ] {
            let (url, _) = fake_shivvr(mode);
            let err = embedder(&url).embed(PNG).unwrap_err().to_string();
            assert!(err.contains(needle), "{needle}: {err}");
        }
    }

    #[test]
    fn validates_input_before_calling() {
        let err = embedder("http://127.0.0.1:9").embed(PNG).unwrap_err().to_string();
        assert!(err.contains("unreachable"), "{err}");
        let (url, seen) = fake_shivvr(Mode::Good);
        let e = embedder(&url);
        assert!(e.embed(b"").is_err());
        assert!(e.embed(b"GIF89a....").is_err());
        let mut big = PNG.to_vec();
        big.resize(SHIVVR_MAX_IMAGE_BYTES + 1, 0);
        assert!(e.embed(&big).unwrap_err().to_string().contains("limit"));
        assert!(seen.lock().unwrap().is_empty(), "invalid input must not reach the service");
        assert!(ShivvrImageEmbedder::new("ftp://x", "a@8", Duration::from_secs(1)).is_err());
        assert!(ShivvrImageEmbedder::new(&url, "no-dim", Duration::from_secs(1)).is_err());
        assert!(ShivvrImageEmbedder::new(&url, "a@8", Duration::ZERO).is_err());
    }

    #[test]
    fn sniffs_formats() {
        assert_eq!(image_format(PNG), Some("png"));
        assert_eq!(image_format(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpeg"));
        assert_eq!(image_format(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(image_format(b"GIF89a"), None);
    }

    /// Live: `cargo test -p ferricula-semantic --no-default-features -- --ignored live_shivvr_image`
    /// Set `FERRICULA_TEST_IMAGE` to a PNG/JPEG path, else a 1x1 PNG is used.
    #[test]
    #[ignore]
    fn live_shivvr_image_embed() {
        const ONE_PX_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
        let bytes = match std::env::var("FERRICULA_TEST_IMAGE") {
            Ok(path) => std::fs::read(path).unwrap(),
            Err(_) => base64::engine::general_purpose::STANDARD.decode(ONE_PX_PNG).unwrap(),
        };
        let e = ShivvrImageEmbedder::new("http://127.0.0.1:8085", DEFAULT_IMAGE_SPACE, Duration::from_secs(60))
            .unwrap();
        let v = e.embed(&bytes).unwrap();
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        println!("space={} dim={} norm={norm:.6}", e.space(), v.len());
        assert_eq!(v.len(), 768);
    }
}
