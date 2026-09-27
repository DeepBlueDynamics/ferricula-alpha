//! Text embedding backends behind one trait, available without the `ml`
//! feature so the server image stays ONNX-free.
//!
//! A backend names its **space** (`"<model>@<dim>"`, e.g. `gtr-t5-base@768`).
//! Vectors from different spaces must never be compared; callers tag every
//! stored vector with the space that produced it.

use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail, ensure};
use serde::{Deserialize, Serialize};

/// Default text space: the space of the recovered v1/v2 memory and of
/// shivvr's `organize` embedder.
pub const DEFAULT_SPACE: &str = "gtr-t5-base@768";
/// shivvr `/embed` per-request text limit.
pub const SHIVVR_MAX_BATCH: usize = 256;
/// shivvr `/embed` per-text size limit (bytes).
pub const SHIVVR_MAX_TEXT_BYTES: usize = 32 * 1024;
/// Allowed deviation of a returned vector's L2 norm from 1.
pub const NORM_TOLERANCE: f32 = 1e-3;

/// Remote or local text embedding. Named `TextEmbedder` so it does not clash
/// with the ONNX [`crate::embedder::Embedder`] struct (feature `ml`).
pub trait TextEmbedder: Send + Sync {
    /// Embedding space id, `"<model>@<dim>"`.
    fn space(&self) -> &str;
    /// Output dimension (0 for [`NoEmbedder`]).
    fn dim(&self) -> usize;
    /// One unit vector per input text, in input order.
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
}

/// Split `"<model>@<dim>"` into its parts.
pub fn parse_space(space: &str) -> Result<(&str, usize)> {
    let (model, dim) = space
        .rsplit_once('@')
        .ok_or_else(|| anyhow!("embedding space {space:?} must look like <model>@<dim>"))?;
    let dim: usize = dim
        .parse()
        .with_context(|| format!("embedding space {space:?} has a non-numeric dimension"))?;
    ensure!(!model.trim().is_empty(), "embedding space {space:?} has an empty model");
    ensure!(dim > 0, "embedding space {space:?} has zero dimension");
    Ok((model, dim))
}

/// Cosine similarity in f64 (vectors of different length compare as 0).
pub fn cosine(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let (mut dot, mut na, mut nb) = (0f64, 0f64, 0f64);
    for (x, y) in a.iter().zip(b) {
        let (x, y) = (*x as f64, *y as f64);
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na.sqrt() * nb.sqrt()) }
}

/// The absent backend: reports space `"none"` and refuses to embed.
#[derive(Debug, Default, Clone)]
pub struct NoEmbedder;

impl TextEmbedder for NoEmbedder {
    fn space(&self) -> &str {
        "none"
    }
    fn dim(&self) -> usize {
        0
    }
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        bail!("no text embedder configured")
    }
}

#[derive(Serialize)]
struct EmbedRequest<'a> {
    texts: &'a [&'a str],
    model: &'a str,
}

#[derive(Deserialize)]
struct EmbedResponse {
    model: String,
    dim: usize,
    vectors: Vec<Vec<f32>>,
}

/// shivvr `POST {url}/embed` over blocking HTTP. No retries: a failure is
/// reported to the caller, which decides whether to degrade.
pub struct ShivvrEmbedder {
    url: String,
    space: String,
    model: String,
    dim: usize,
    batch: usize,
    agent: ureq::Agent,
}

impl ShivvrEmbedder {
    /// `url` is the service base (e.g. `http://127.0.0.1:8085`); `space` is
    /// `"<model>@<dim>"`; `batch` texts per request (1..=256).
    pub fn new(url: &str, space: &str, timeout: Duration, batch: usize) -> Result<Self> {
        let url = url.trim().trim_end_matches('/').to_string();
        ensure!(
            url.starts_with("http://") || url.starts_with("https://"),
            "shivvr url must be http(s)"
        );
        let (model, dim) = parse_space(space)?;
        ensure!(
            (1..=SHIVVR_MAX_BATCH).contains(&batch),
            "shivvr batch must be in 1..={SHIVVR_MAX_BATCH}"
        );
        ensure!(!timeout.is_zero(), "shivvr timeout must be positive");
        let agent = ureq::AgentBuilder::new().timeout(timeout).build();
        Ok(Self {
            url,
            space: space.to_string(),
            model: model.to_string(),
            dim,
            batch,
            agent,
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn batch(&self) -> usize {
        self.batch
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let endpoint = format!("{}/embed", self.url);
        let body = EmbedRequest { texts, model: &self.model };
        let response = match self.agent.post(&endpoint).send_json(&body) {
            Ok(response) => response,
            Err(ureq::Error::Status(code, response)) => {
                let detail = response.into_string().unwrap_or_default();
                let detail: String = detail.chars().take(300).collect();
                bail!("shivvr /embed returned HTTP {code}: {detail}");
            }
            Err(err) => return Err(anyhow!("shivvr /embed unreachable at {endpoint}: {err}")),
        };
        let parsed: EmbedResponse = response
            .into_json()
            .context("shivvr /embed returned malformed JSON")?;
        ensure!(
            parsed.model == self.model,
            "shivvr /embed answered with model {:?}, expected {:?}",
            parsed.model,
            self.model
        );
        ensure!(
            parsed.dim == self.dim,
            "shivvr /embed reports dim {}, space {} expects {}",
            parsed.dim,
            self.space,
            self.dim
        );
        ensure!(
            parsed.vectors.len() == texts.len(),
            "shivvr /embed returned {} vectors for {} texts",
            parsed.vectors.len(),
            texts.len()
        );
        for (i, v) in parsed.vectors.iter().enumerate() {
            ensure!(v.len() == self.dim, "vector {i} has {} dims, expected {}", v.len(), self.dim);
            ensure!(v.iter().all(|x| x.is_finite()), "vector {i} has non-finite values");
            let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            ensure!(
                (norm - 1.0).abs() <= NORM_TOLERANCE,
                "vector {i} is not unit length (norm {norm})"
            );
        }
        Ok(parsed.vectors)
    }
}

impl TextEmbedder for ShivvrEmbedder {
    fn space(&self) -> &str {
        &self.space
    }
    fn dim(&self) -> usize {
        self.dim
    }
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        for (i, t) in texts.iter().enumerate() {
            ensure!(!t.trim().is_empty(), "text {i} is empty");
            ensure!(
                t.len() <= SHIVVR_MAX_TEXT_BYTES,
                "text {i} is {} bytes (shivvr limit {SHIVVR_MAX_TEXT_BYTES})",
                t.len()
            );
        }
        let mut out = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(self.batch) {
            out.extend(self.embed_batch(chunk)?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// Deterministic unit vector for a text.
    fn unit(text: &str, dim: usize) -> Vec<f32> {
        let mut v: Vec<f32> = (0..dim)
            .map(|i| ((text.len() * 31 + i * 7) % 17) as f32 + 1.0)
            .collect();
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter_mut().for_each(|x| *x /= n);
        v
    }

    #[derive(Clone, Copy)]
    enum Mode {
        Good,
        WrongDim,
        NotUnit,
        Status500,
    }

    /// Tiny shivvr: `POST /embed`, records each request's batch size.
    fn fake_shivvr(mode: Mode) -> (String, Arc<Mutex<Vec<usize>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut length = 0usize;
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
                    if let Some(v) = l.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0u8; length];
                let _ = reader.read_exact(&mut body);
                let request: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                let texts: Vec<String> = request["texts"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|t| t.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                log.lock().unwrap().push(texts.len());
                let (status, payload) = match mode {
                    Mode::Status500 => ("500 Internal Server Error", json!({"error": "boom"})),
                    Mode::Good | Mode::WrongDim | Mode::NotUnit => {
                        let dim = if matches!(mode, Mode::WrongDim) { 4 } else { 8 };
                        let vectors: Vec<Vec<f32>> = texts
                            .iter()
                            .map(|t| {
                                let mut v = unit(t, dim);
                                if matches!(mode, Mode::NotUnit) {
                                    v.iter_mut().for_each(|x| *x *= 2.0);
                                }
                                v
                            })
                            .collect();
                        ("200 OK", json!({"model": "tiny", "dim": dim, "vectors": vectors}))
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

    fn embedder(url: &str, batch: usize) -> ShivvrEmbedder {
        ShivvrEmbedder::new(url, "tiny@8", Duration::from_secs(5), batch).unwrap()
    }

    #[test]
    fn parses_space() {
        assert_eq!(parse_space("gtr-t5-base@768").unwrap(), ("gtr-t5-base", 768));
        assert!(parse_space("gtr-t5-base").is_err());
        assert!(parse_space("x@0").is_err());
        assert!(parse_space("@8").is_err());
    }

    #[test]
    fn splits_batches_and_keeps_order() {
        let (url, seen) = fake_shivvr(Mode::Good);
        let e = embedder(&format!("{url}/"), 2);
        assert_eq!(e.space(), "tiny@8");
        assert_eq!(e.dim(), 8);
        let texts = ["a", "bb", "ccc", "dddd", "eeeee"];
        let out = e.embed(&texts).unwrap();
        assert_eq!(out.len(), 5);
        for (t, v) in texts.iter().zip(&out) {
            assert!(cosine(v, &unit(t, 8)) > 0.9999);
        }
        assert_eq!(*seen.lock().unwrap(), vec![2, 2, 1]);
        assert!(e.embed(&[]).unwrap().is_empty());
    }

    #[test]
    fn rejects_wrong_dim_non_unit_and_http_errors() {
        let (url, _) = fake_shivvr(Mode::WrongDim);
        let err = embedder(&url, 4).embed(&["x"]).unwrap_err().to_string();
        assert!(err.contains("dim"), "{err}");

        let (url, _) = fake_shivvr(Mode::NotUnit);
        let err = embedder(&url, 4).embed(&["x"]).unwrap_err().to_string();
        assert!(err.contains("unit length"), "{err}");

        let (url, _) = fake_shivvr(Mode::Status500);
        let err = embedder(&url, 4).embed(&["x"]).unwrap_err().to_string();
        assert!(err.contains("HTTP 500") && err.contains("boom"), "{err}");
    }

    #[test]
    fn unreachable_and_input_validation() {
        // Port 9 (discard) on loopback: nothing listens.
        let err = embedder("http://127.0.0.1:9", 4).embed(&["x"]).unwrap_err().to_string();
        assert!(err.contains("unreachable"), "{err}");
        let (url, seen) = fake_shivvr(Mode::Good);
        let e = embedder(&url, 4);
        assert!(e.embed(&["ok", " "]).is_err());
        let big = "a".repeat(SHIVVR_MAX_TEXT_BYTES + 1);
        assert!(e.embed(&[big.as_str()]).is_err());
        assert!(seen.lock().unwrap().is_empty(), "invalid input must not reach the service");
        assert!(ShivvrEmbedder::new("ftp://x", "tiny@8", Duration::from_secs(1), 1).is_err());
        assert!(ShivvrEmbedder::new(&url, "tiny@8", Duration::from_secs(1), 0).is_err());
        assert!(ShivvrEmbedder::new(&url, "tiny@8", Duration::from_secs(1), 257).is_err());
    }

    #[test]
    fn no_embedder_refuses() {
        let e = NoEmbedder;
        assert_eq!(e.space(), "none");
        assert_eq!(e.dim(), 0);
        assert!(e.embed(&[]).unwrap().is_empty());
        assert!(e.embed(&["x"]).is_err());
    }

    #[test]
    fn cosine_basics() {
        assert!((cosine(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-12);
        assert_eq!(cosine(&[1.0, 0.0], &[0.0, 1.0]), 0.0);
        assert_eq!(cosine(&[1.0], &[1.0, 0.0]), 0.0);
    }
}
