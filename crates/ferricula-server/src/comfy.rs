//! ComfyUI client for dream images (X4, client half).
//!
//! Fills an API-format workflow template (`config/comfyui/*.api.json`),
//! submits it (`POST /prompt`), polls `GET /history/{id}` until a deadline and
//! downloads the first output image (`GET /view`). Blocking HTTP: async
//! callers use `spawn_blocking`.
//!
//! Placeholders (see `config/comfyui/README.md`): `{{prompt}}`,
//! `{{negative}}`, `{{prefix}}` inside strings, and a string that is exactly
//! `"{{seed}}"`, which becomes an integer. The template is parsed first and
//! placeholders are substituted in the parsed values, so prompt text never
//! needs manual JSON escaping.

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail, ensure};
use reqwest::blocking::{Client, Response};
use serde_json::{Map, Value, json};

/// Primary: Qwen Image 2.1 (int8) with the qwen3vl-8b text encoder.
pub const QWEN_IMAGE_TEMPLATE: &str = include_str!("../../../config/comfyui/qwen_image_2_1.api.json");
/// Fallback: SDXL Turbo, 4 steps.
pub const SDXL_TURBO_TEMPLATE: &str = include_str!("../../../config/comfyui/sdxl_turbo.api.json");

pub const DEFAULT_COMFY_URL: &str = "http://127.0.0.1:8188";
/// Whole-render deadline (queue wait + first model load + sampling). A cold
/// Qwen Image 2.1 render (both int8 models loading) measured 367 s on the
/// operator's GPU; warm renders are far shorter.
pub const DEFAULT_RENDER_TIMEOUT: Duration = Duration::from_secs(600);
/// Longest single HTTP exchange.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_POLL: Duration = Duration::from_millis(750);

/// A named workflow template (API format, with placeholders).
#[derive(Debug, Clone, PartialEq)]
pub struct ComfyTemplate {
    pub name: String,
    pub json: String,
}

impl ComfyTemplate {
    pub fn qwen_image() -> Self {
        Self { name: "qwen_image_2_1".into(), json: QWEN_IMAGE_TEMPLATE.into() }
    }

    pub fn sdxl_turbo() -> Self {
        Self { name: "sdxl_turbo".into(), json: SDXL_TURBO_TEMPLATE.into() }
    }

    /// Load an override template from disk; validated by a trial fill.
    pub fn from_path(path: &Path) -> Result<Self> {
        let json = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read ComfyUI template {}", path.display()))?;
        fill_template(&json, "probe", "probe", 0, "probe")
            .with_context(|| format!("invalid ComfyUI template {}", path.display()))?;
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("custom");
        Ok(Self { name: name.trim_end_matches(".api").to_string(), json })
    }
}

/// Primary and optional fallback template.
#[derive(Debug, Clone, PartialEq)]
pub struct ComfyTemplates {
    pub primary: ComfyTemplate,
    pub fallback: Option<ComfyTemplate>,
}

impl Default for ComfyTemplates {
    fn default() -> Self {
        Self { primary: ComfyTemplate::qwen_image(), fallback: Some(ComfyTemplate::sdxl_turbo()) }
    }
}

impl ComfyTemplates {
    /// Embedded defaults, with either side replaced by a file when given.
    pub fn load(primary: Option<&Path>, fallback: Option<&Path>) -> Result<Self> {
        let mut t = Self::default();
        if let Some(p) = primary {
            t.primary = ComfyTemplate::from_path(p)?;
        }
        if let Some(p) = fallback {
            t.fallback = Some(ComfyTemplate::from_path(p)?);
        }
        Ok(t)
    }
}

/// One rendered image.
#[derive(Debug, Clone)]
pub struct RenderedImage {
    /// Encoded image as ComfyUI saved it (PNG for `SaveImage`).
    pub bytes: Vec<u8>,
    pub filename: String,
    pub subfolder: String,
    pub prompt_id: String,
    /// Checkpoint/UNet file the graph loaded.
    pub model: String,
    /// Template name that produced it.
    pub template: String,
    pub seed: u64,
    /// Wall-clock seconds from submit to download.
    pub secs: f64,
    /// Why the primary template failed, when this came from the fallback.
    pub fallback_reason: Option<String>,
}

/// Parse `template_json` and substitute placeholders. Errors if the template
/// has no `{{prompt}}` or keeps an unknown `{{…}}` placeholder.
pub fn fill_template(template_json: &str, prompt: &str, negative: &str, seed: u64, prefix: &str) -> Result<Value> {
    let mut graph: Value = serde_json::from_str(template_json).context("ComfyUI template is not JSON")?;
    ensure!(graph.is_object(), "ComfyUI template must be an API-format object of nodes");
    ensure!(template_json.contains("{{prompt}}"), "ComfyUI template has no {{{{prompt}}}} placeholder");
    let mut unknown = Vec::new();
    fill_value(&mut graph, prompt, negative, seed, prefix, &mut unknown);
    ensure!(unknown.is_empty(), "ComfyUI template has unknown placeholders: {}", unknown.join(", "));
    Ok(graph)
}

fn fill_value(v: &mut Value, prompt: &str, negative: &str, seed: u64, prefix: &str, unknown: &mut Vec<String>) {
    match v {
        Value::String(s) if s.as_str() == "{{seed}}" => *v = json!(seed),
        Value::String(s) => {
            if s.contains("{{") {
                *s = substitute(s, prompt, negative, prefix, unknown);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|x| fill_value(x, prompt, negative, seed, prefix, unknown)),
        Value::Object(map) => map.values_mut().for_each(|x| fill_value(x, prompt, negative, seed, prefix, unknown)),
        _ => {}
    }
}

/// Single left-to-right pass, so substituted text is never re-scanned (a
/// prompt containing "{{negative}}" stays literal).
fn substitute(s: &str, prompt: &str, negative: &str, prefix: &str, unknown: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(s.len() + prompt.len());
    let mut rest = s;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[start..]);
            return out;
        };
        match &after[..end] {
            "prompt" => out.push_str(prompt),
            "negative" => out.push_str(negative),
            "prefix" => out.push_str(prefix),
            other => {
                unknown.push(format!("{{{{{other}}}}}"));
                out.push_str(&rest[start..start + 4 + end]);
            }
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

/// The model file a filled graph loads (first loader input found).
pub fn graph_model(graph: &Value) -> String {
    let Some(nodes) = graph.as_object() else { return String::new() };
    let mut ids: Vec<&String> = nodes.keys().collect();
    ids.sort_by_key(|k| (k.parse::<u64>().unwrap_or(u64::MAX), k.to_string()));
    for id in ids {
        let inputs = &nodes[id]["inputs"];
        for key in ["unet_name", "ckpt_name"] {
            if let Some(name) = inputs[key].as_str() {
                return name.to_string();
            }
        }
    }
    String::new()
}

/// Blocking ComfyUI HTTP client.
#[derive(Debug, Clone)]
pub struct ComfyClient {
    base_url: String,
    timeout: Duration,
    poll: Duration,
    client_id: String,
    http: Client,
}

impl ComfyClient {
    /// `base_url` like `http://127.0.0.1:8188`; `timeout` bounds a whole render.
    pub fn new(base_url: &str, timeout: Duration) -> Result<Self> {
        let base_url = base_url.trim().trim_end_matches('/').to_string();
        ensure!(
            base_url.starts_with("http://") || base_url.starts_with("https://"),
            "ComfyUI url must be http(s)"
        );
        ensure!(!timeout.is_zero(), "ComfyUI timeout must be positive");
        let http = Client::builder()
            .timeout(timeout.min(REQUEST_TIMEOUT))
            .build()
            .context("failed to build ComfyUI HTTP client")?;
        Ok(Self { base_url, timeout, poll: DEFAULT_POLL, client_id: format!("ferricula-{}", uuid::Uuid::new_v4()), http })
    }

    /// Change the history poll interval (tests use a short one).
    pub fn with_poll_interval(mut self, poll: Duration) -> Self {
        self.poll = poll;
        self
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Fill `template_json`, render, and download the first output image.
    pub fn render(&self, template_json: &str, prompt: &str, negative: &str, seed: u64, prefix: &str) -> Result<RenderedImage> {
        ensure!(!prompt.trim().is_empty(), "visual prompt is empty");
        let graph = fill_template(template_json, prompt, negative, seed, prefix)?;
        let model = graph_model(&graph);
        let started = Instant::now();
        let deadline = started + self.timeout;
        let prompt_id = self.submit(&graph)?;
        let (filename, subfolder, kind) = match self.wait(&prompt_id, deadline) {
            Ok(found) => found,
            Err(err) => {
                self.cancel(&prompt_id);
                return Err(err);
            }
        };
        let bytes = self.view(&filename, &subfolder, &kind, None)?;
        Ok(RenderedImage {
            bytes,
            filename,
            subfolder,
            prompt_id,
            model,
            template: String::new(),
            seed,
            secs: started.elapsed().as_secs_f64(),
            fallback_reason: None,
        })
    }

    /// Render with `templates.primary`; on any failure (node error, OOM,
    /// timeout, unreachable model) try `templates.fallback` once.
    pub fn render_with_fallback(
        &self,
        templates: &ComfyTemplates,
        prompt: &str,
        negative: &str,
        seed: u64,
        prefix: &str,
    ) -> Result<RenderedImage> {
        let primary = self.render(&templates.primary.json, prompt, negative, seed, prefix);
        let err = match primary {
            Ok(mut image) => {
                image.template = templates.primary.name.clone();
                return Ok(image);
            }
            Err(err) => err,
        };
        let Some(fallback) = &templates.fallback else { return Err(err) };
        let reason = format!("{}: {err:#}", templates.primary.name);
        let mut image = self
            .render(&fallback.json, prompt, negative, seed, prefix)
            .with_context(|| format!("fallback {} also failed after primary {reason}", fallback.name))?;
        image.template = fallback.name.clone();
        image.fallback_reason = Some(reason);
        Ok(image)
    }

    /// Re-fetch a rendered image as a ComfyUI preview (`"jpeg;90"`,
    /// `"webp;85"`): smaller bytes for embedding services with body limits.
    pub fn preview(&self, image: &RenderedImage, format: &str) -> Result<Vec<u8>> {
        self.view(&image.filename, &image.subfolder, "output", Some(format))
    }

    fn submit(&self, graph: &Value) -> Result<String> {
        let url = format!("{}/prompt", self.base_url);
        let body = json!({"prompt": graph, "client_id": self.client_id});
        let response = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| anyhow!("ComfyUI unreachable at {url}: {e}"))?;
        let status = response.status();
        let payload = read_json(response, "/prompt")?;
        if !status.is_success() {
            bail!("ComfyUI /prompt rejected the graph (HTTP {status}): {}", describe_prompt_error(&payload));
        }
        if let Some(errors) = payload.get("node_errors").and_then(Value::as_object).filter(|m| !m.is_empty()) {
            bail!("ComfyUI node errors: {}", describe_node_errors(errors));
        }
        payload["prompt_id"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| anyhow!("ComfyUI /prompt answered without prompt_id: {}", clip(&payload.to_string())))
    }

    /// Poll history until the prompt finishes; returns (filename, subfolder, type).
    fn wait(&self, prompt_id: &str, deadline: Instant) -> Result<(String, String, String)> {
        let url = format!("{}/history/{prompt_id}", self.base_url);
        loop {
            let response = self.http.get(&url).send();
            match response {
                Ok(r) if r.status().is_success() => {
                    let history = read_json(r, "/history")?;
                    if let Some(entry) = history.get(prompt_id) {
                        if let Some(done) = finished(entry)? {
                            return Ok(done);
                        }
                    }
                }
                Ok(r) => bail!("ComfyUI /history returned HTTP {}", r.status()),
                Err(e) if Instant::now() >= deadline => bail!("ComfyUI /history unreachable: {e}"),
                Err(_) => {}
            }
            if Instant::now() + self.poll > deadline {
                bail!("ComfyUI render {prompt_id} timed out after {:.0}s", self.timeout.as_secs_f64());
            }
            std::thread::sleep(self.poll);
        }
    }

    /// Best effort: drop the prompt from the queue, or interrupt it if it is
    /// the one running (ComfyUI only interrupts a matching prompt_id).
    fn cancel(&self, prompt_id: &str) {
        let _ = self.http.post(format!("{}/queue", self.base_url)).json(&json!({"delete": [prompt_id]})).send();
        let _ = self.http.post(format!("{}/interrupt", self.base_url)).json(&json!({"prompt_id": prompt_id})).send();
    }

    fn view(&self, filename: &str, subfolder: &str, kind: &str, preview: Option<&str>) -> Result<Vec<u8>> {
        let url = format!("{}/view", self.base_url);
        let mut query = vec![("filename", filename), ("subfolder", subfolder), ("type", kind)];
        if let Some(p) = preview {
            query.push(("preview", p));
        }
        let response = self
            .http
            .get(&url)
            .query(&query)
            .send()
            .map_err(|e| anyhow!("ComfyUI /view unreachable: {e}"))?;
        let status = response.status();
        ensure!(status.is_success(), "ComfyUI /view {filename} returned HTTP {status}");
        let bytes = response.bytes().context("ComfyUI /view body")?.to_vec();
        ensure!(!bytes.is_empty(), "ComfyUI /view {filename} returned no bytes");
        Ok(bytes)
    }
}

/// `Some(image)` when the history entry is complete with an output image,
/// `None` while running, an error when execution failed.
fn finished(entry: &Value) -> Result<Option<(String, String, String)>> {
    let status = &entry["status"];
    if status["status_str"].as_str() == Some("error") {
        bail!("ComfyUI execution failed: {}", describe_execution_error(status));
    }
    let outputs = entry["outputs"].as_object();
    let image = outputs.and_then(|o| {
        let mut ids: Vec<&String> = o.keys().collect();
        ids.sort();
        ids.into_iter().find_map(|id| o[id]["images"].as_array().and_then(|a| a.first()).cloned())
    });
    match image {
        Some(img) => {
            let filename = img["filename"].as_str().unwrap_or_default().to_string();
            ensure!(!filename.is_empty(), "ComfyUI output image has no filename");
            Ok(Some((
                filename,
                img["subfolder"].as_str().unwrap_or_default().to_string(),
                img["type"].as_str().unwrap_or("output").to_string(),
            )))
        }
        None if status["completed"].as_bool() == Some(true) => bail!("ComfyUI finished without an output image"),
        None => Ok(None),
    }
}

fn read_json(response: Response, what: &str) -> Result<Value> {
    let text = response.text().with_context(|| format!("ComfyUI {what} body"))?;
    serde_json::from_str(&text).with_context(|| format!("ComfyUI {what} returned non-JSON: {}", clip(&text)))
}

fn describe_prompt_error(payload: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(e) = payload.get("error") {
        let msg = e["message"].as_str().or_else(|| e.as_str()).unwrap_or_default();
        let details = e["details"].as_str().unwrap_or_default();
        parts.push(format!("{msg} {details}").trim().to_string());
    }
    if let Some(errors) = payload.get("node_errors").and_then(Value::as_object).filter(|m| !m.is_empty()) {
        parts.push(describe_node_errors(errors));
    }
    if parts.is_empty() { clip(&payload.to_string()) } else { parts.join("; ") }
}

fn describe_node_errors(errors: &Map<String, Value>) -> String {
    let mut out = Vec::new();
    for (node, info) in errors {
        let class = info["class_type"].as_str().unwrap_or("?");
        let reasons: Vec<String> = info["errors"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|e| {
                        let msg = e["message"].as_str().unwrap_or_default();
                        let details = e["details"].as_str().unwrap_or_default();
                        format!("{msg} {details}").trim().to_string()
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.push(format!("node {node} ({class}): {}", reasons.join(", ")));
    }
    clip(&out.join("; "))
}

fn describe_execution_error(status: &Value) -> String {
    let messages = status["messages"].as_array().cloned().unwrap_or_default();
    for m in messages.iter().rev() {
        if m[0].as_str() == Some("execution_error") {
            let d = &m[1];
            return clip(&format!(
                "node {} ({}): {} {}",
                d["node_id"].as_str().unwrap_or("?"),
                d["node_type"].as_str().unwrap_or("?"),
                d["exception_type"].as_str().unwrap_or_default(),
                d["exception_message"].as_str().unwrap_or_default().trim()
            ));
        }
        if m[0].as_str() == Some("execution_interrupted") {
            return "interrupted".into();
        }
    }
    "status error (no message)".into()
}

fn clip(s: &str) -> String {
    s.chars().take(400).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake";

    #[test]
    fn fills_placeholders_with_real_json_escaping() {
        let prompt = "a \"quoted\" stone,\nback\\slash, {{negative}} stays, {{seed}}";
        let graph = fill_template(QWEN_IMAGE_TEMPLATE, prompt, "text, letters", 18_446_744_073_709_551_615, "dream/x").unwrap();
        assert_eq!(graph["4"]["inputs"]["prompt"], json!(prompt));
        assert_eq!(graph["4"]["inputs"]["negative_prompt"], json!("text, letters"));
        assert_eq!(graph["6"]["inputs"]["seed"], json!(u64::MAX));
        assert!(graph["6"]["inputs"]["seed"].is_u64());
        assert_eq!(graph["8"]["inputs"]["filename_prefix"], json!("dream/x"));
        assert_eq!(graph_model(&graph), "qwen_image_2.1_int8_convrot.safetensors");

        let graph = fill_template(SDXL_TURBO_TEMPLATE, "p", "n", 7, "pre").unwrap();
        assert_eq!(graph["2"]["inputs"]["text"], json!("p"));
        assert_eq!(graph["3"]["inputs"]["text"], json!("n"));
        assert_eq!(graph["5"]["inputs"]["seed"], json!(7));
        assert_eq!(graph_model(&graph), "sd_xl_turbo_1.0_fp16.safetensors");
        // Round-trips as valid JSON with no placeholders left.
        let text = graph.to_string();
        assert!(!text.contains("{{"));
    }

    #[test]
    fn rejects_bad_templates() {
        assert!(fill_template("not json", "p", "n", 1, "x").is_err());
        assert!(fill_template("[1]", "p", "n", 1, "x").is_err());
        assert!(fill_template(r#"{"1":{"inputs":{"text":"hi"}}}"#, "p", "n", 1, "x").is_err());
        let err = fill_template(r#"{"1":{"inputs":{"text":"{{prompt}} {{style}}"}}}"#, "p", "n", 1, "x").unwrap_err();
        assert!(err.to_string().contains("{{style}}"), "{err}");
    }

    #[test]
    fn override_template_from_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mine.api.json");
        std::fs::write(&path, SDXL_TURBO_TEMPLATE).unwrap();
        let t = ComfyTemplates::load(Some(&path), None).unwrap();
        assert_eq!(t.primary.name, "mine");
        assert_eq!(t.fallback.unwrap().name, "sdxl_turbo");
        std::fs::write(&path, "{}").unwrap();
        assert!(ComfyTemplate::from_path(&path).is_err());
    }

    /// Fake ComfyUI behaviour, keyed by the model a submitted graph loads.
    #[derive(Clone, Copy, PartialEq)]
    enum Behave {
        Ok,
        NodeErrors,
        ExecError,
        Never,
    }

    struct Fake {
        url: String,
        submitted: Arc<Mutex<Vec<Value>>>,
        paths: Arc<Mutex<Vec<String>>>,
    }

    fn fake_comfy(by_model: HashMap<&'static str, Behave>) -> Fake {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let submitted = Arc::new(Mutex::new(Vec::<Value>::new()));
        let paths = Arc::new(Mutex::new(Vec::<String>::new()));
        let (sub, log) = (submitted.clone(), paths.clone());
        std::thread::spawn(move || {
            let mut jobs: HashMap<String, Behave> = HashMap::new();
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let (mut method, mut path, mut length) = (String::new(), String::new(), 0usize);
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
                    if method.is_empty() {
                        let mut it = l.split_whitespace();
                        method = it.next().unwrap_or("").into();
                        path = it.next().unwrap_or("").into();
                    }
                    if let Some(v) = l.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0u8; length];
                let _ = reader.read_exact(&mut body);
                log.lock().unwrap().push(format!("{method} {path}"));
                let (status, ctype, payload): (&str, &str, Vec<u8>) = if method == "POST" && path == "/prompt" {
                    let req: Value = serde_json::from_slice(&body).unwrap();
                    let graph = req["prompt"].clone();
                    let behave = *by_model.get(graph_model(&graph).as_str()).unwrap_or(&Behave::Ok);
                    sub.lock().unwrap().push(graph);
                    if behave == Behave::NodeErrors {
                        let p = json!({"error": {"type": "prompt_outputs_failed_validation", "message": "Prompt outputs failed validation", "details": ""},
                            "node_errors": {"1": {"class_type": "UNETLoader", "errors": [{"message": "Value not in list", "details": "unet_name: 'missing.safetensors' not in []"}]}}});
                        ("400 Bad Request", "application/json", p.to_string().into_bytes())
                    } else {
                        let id = format!("pid-{}", jobs.len());
                        jobs.insert(id.clone(), behave);
                        ("200 OK", "application/json", json!({"prompt_id": id, "number": 1, "node_errors": {}}).to_string().into_bytes())
                    }
                } else if let Some(id) = path.strip_prefix("/history/") {
                    let entry = match jobs.get(id) {
                        Some(Behave::Ok) => json!({id: {"outputs": {"8": {"images": [{"filename": "dream_00001_.png", "subfolder": "", "type": "output"}]}},
                            "status": {"status_str": "success", "completed": true, "messages": []}}}),
                        Some(Behave::ExecError) => json!({id: {"outputs": {},
                            "status": {"status_str": "error", "completed": false, "messages": [["execution_start", {}],
                                ["execution_error", {"node_id": "6", "node_type": "KSampler", "exception_type": "torch.OutOfMemoryError", "exception_message": "CUDA out of memory."}]]}}}),
                        _ => json!({}),
                    };
                    ("200 OK", "application/json", entry.to_string().into_bytes())
                } else if method == "GET" && path.starts_with("/view?") {
                    ("200 OK", "image/png", PNG.to_vec())
                } else {
                    ("200 OK", "application/json", b"{}".to_vec())
                };
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    payload.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&payload);
            }
        });
        Fake { url, submitted, paths }
    }

    const QWEN: &str = "qwen_image_2.1_int8_convrot.safetensors";

    fn client(url: &str, timeout_ms: u64) -> ComfyClient {
        ComfyClient::new(url, Duration::from_millis(timeout_ms)).unwrap().with_poll_interval(Duration::from_millis(20))
    }

    #[test]
    fn renders_and_downloads() {
        let fake = fake_comfy(HashMap::new());
        let c = client(&format!("{}/", fake.url), 5_000);
        let img = c.render(QWEN_IMAGE_TEMPLATE, "a \"stone\"", "text", 42, "dream/abc").unwrap();
        assert_eq!(img.bytes, PNG);
        assert_eq!(img.filename, "dream_00001_.png");
        assert_eq!(img.prompt_id, "pid-0");
        assert_eq!(img.model, QWEN);
        assert_eq!(img.seed, 42);
        let graph = &fake.submitted.lock().unwrap()[0];
        assert_eq!(graph["4"]["inputs"]["prompt"], json!("a \"stone\""));
        assert_eq!(graph["6"]["inputs"]["seed"], json!(42));
        let paths = fake.paths.lock().unwrap();
        assert!(paths.iter().any(|p| p.starts_with("GET /view?filename=dream_00001_.png")), "{paths:?}");
    }

    #[test]
    fn surfaces_node_errors_and_execution_errors() {
        let fake = fake_comfy(HashMap::from([(QWEN, Behave::NodeErrors)]));
        let err = client(&fake.url, 5_000).render(QWEN_IMAGE_TEMPLATE, "p", "n", 1, "x").unwrap_err().to_string();
        assert!(err.contains("HTTP 400") && err.contains("UNETLoader") && err.contains("missing.safetensors"), "{err}");

        let fake = fake_comfy(HashMap::from([(QWEN, Behave::ExecError)]));
        let err = client(&fake.url, 5_000).render(QWEN_IMAGE_TEMPLATE, "p", "n", 1, "x").unwrap_err().to_string();
        assert!(err.contains("KSampler") && err.contains("out of memory"), "{err}");
    }

    #[test]
    fn times_out_and_cancels() {
        let fake = fake_comfy(HashMap::from([(QWEN, Behave::Never)]));
        let started = Instant::now();
        let err = client(&fake.url, 300).render(QWEN_IMAGE_TEMPLATE, "p", "n", 1, "x").unwrap_err().to_string();
        assert!(err.contains("timed out"), "{err}");
        assert!(started.elapsed() < Duration::from_secs(3));
        let paths = fake.paths.lock().unwrap();
        assert!(paths.contains(&"POST /queue".to_string()) && paths.contains(&"POST /interrupt".to_string()), "{paths:?}");
    }

    #[test]
    fn falls_back_once_when_primary_fails() {
        for behave in [Behave::NodeErrors, Behave::ExecError, Behave::Never] {
            let fake = fake_comfy(HashMap::from([(QWEN, behave)]));
            let img = client(&fake.url, 400)
                .render_with_fallback(&ComfyTemplates::default(), "p", "n", 9, "x")
                .unwrap();
            assert_eq!(img.template, "sdxl_turbo");
            assert_eq!(img.model, "sd_xl_turbo_1.0_fp16.safetensors");
            assert!(img.fallback_reason.as_deref().unwrap().starts_with("qwen_image_2_1:"));
            assert_eq!(fake.submitted.lock().unwrap().len(), 2);
        }
        // Primary success: no fallback, template recorded.
        let fake = fake_comfy(HashMap::new());
        let img = client(&fake.url, 2_000).render_with_fallback(&ComfyTemplates::default(), "p", "n", 9, "x").unwrap();
        assert_eq!((img.template.as_str(), img.fallback_reason), ("qwen_image_2_1", None));
        // Both fail: one error naming both.
        let fake = fake_comfy(HashMap::from([(QWEN, Behave::ExecError), ("sd_xl_turbo_1.0_fp16.safetensors", Behave::ExecError)]));
        let err = client(&fake.url, 2_000)
            .render_with_fallback(&ComfyTemplates::default(), "p", "n", 9, "x")
            .unwrap_err();
        let err = format!("{err:#}");
        assert!(err.contains("fallback sdxl_turbo also failed") && err.contains("qwen_image_2_1"), "{err}");
        assert_eq!(fake.submitted.lock().unwrap().len(), 2);
    }

    #[test]
    fn unreachable_and_input_validation() {
        let err = client("http://127.0.0.1:9", 1_000).render(SDXL_TURBO_TEMPLATE, "p", "n", 1, "x").unwrap_err();
        assert!(err.to_string().contains("unreachable"), "{err}");
        assert!(client("http://127.0.0.1:9", 1_000).render(SDXL_TURBO_TEMPLATE, "  ", "n", 1, "x").is_err());
        assert!(ComfyClient::new("ftp://x", Duration::from_secs(1)).is_err());
        assert!(ComfyClient::new("http://x", Duration::ZERO).is_err());
    }

    /// Live (Kord's native ComfyUI + shivvr): renders one dream frame, saves
    /// it to `audit/life/dream-image-sample.png`, embeds it with SigLIP.
    /// `cargo test -p ferricula-server --lib comfy::tests::live -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_render_and_embed_dream_sample() {
        use ferricula_cognition::dream_image::{DREAM_NEGATIVE_PROMPT, candidate_names, sanitize_visual_prompt};
        use ferricula_semantic::image_embed::{DEFAULT_IMAGE_SPACE, SHIVVR_MAX_IMAGE_BYTES};
        use ferricula_semantic::{ImageEmbedder, ShivvrImageEmbedder};

        let dream = "Jony is in the garage on Crist Drive. He is holding a stone. Pocket-sized, warm, no screen. \
            He holds it to his ear the way you hold a shell. On Dad's workbench the proteins are folding.";
        // What the visual-prompt model would answer (written by hand here),
        // deliberately leaking names to exercise the sanitizer.
        let llm_answer = "\"Jony stands in a cluttered suburban garage at dusk, warm tungsten work light, \
            holding a smooth pocket-sized river stone to his ear like a seashell, old wooden workbench with vise and hand tools, \
            translucent ribbon-like protein structures folding and glowing softly above the bench, dust in the light, \
            open garage door, quiet blue evening outside\"";
        let mut forbidden = vec!["Jony", "Steve Jobs", "Steve"];
        let harvested = candidate_names(dream);
        forbidden.extend(harvested.iter().map(String::as_str));
        let prompt = sanitize_visual_prompt(llm_answer, &forbidden);
        for n in ["Jony", "Steve", "Jobs", "Crist", "\""] {
            assert!(!prompt.contains(n), "{n} leaked: {prompt}");
        }
        println!("visual prompt ({} words): {prompt}", prompt.split_whitespace().count());

        let comfy = ComfyClient::new(DEFAULT_COMFY_URL, Duration::from_secs(600)).unwrap();
        let seed = 0x5eed_d8ea_u64;
        let img = comfy
            .render_with_fallback(&ComfyTemplates::default(), &prompt, DREAM_NEGATIVE_PROMPT, seed, "ferricula/dream-sample")
            .unwrap();
        println!(
            "rendered {} via {} ({}) in {:.1}s, {} bytes, fallback={:?}",
            img.filename,
            img.template,
            img.model,
            img.secs,
            img.bytes.len(),
            img.fallback_reason
        );
        let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../audit/life/dream-image-sample.png");
        std::fs::write(&out, &img.bytes).unwrap();

        let embed_bytes =
            if img.bytes.len() > SHIVVR_MAX_IMAGE_BYTES { comfy.preview(&img, "jpeg;92").unwrap() } else { img.bytes.clone() };
        let embedder = ShivvrImageEmbedder::new("http://127.0.0.1:8085", DEFAULT_IMAGE_SPACE, Duration::from_secs(60)).unwrap();
        let t = Instant::now();
        let v = embedder.embed(&embed_bytes).unwrap();
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        println!(
            "embedded {} bytes in {:.2}s: space={} dim={} norm={norm:.6}",
            embed_bytes.len(),
            t.elapsed().as_secs_f64(),
            embedder.space(),
            v.len()
        );
        assert_eq!(v.len(), 768);
    }
}
