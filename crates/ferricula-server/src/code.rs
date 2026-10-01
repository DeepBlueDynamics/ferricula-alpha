//! Read-only access to source code for the agent: its own engine
//! (Ferricula), mounted read-only at `[code] root`, and the project's pull
//! requests on GitHub (public API, no token).
//!
//! Tools: `code_tree`, `code_search`, `code_read`, `pr_list`, `pr_diff`.
//! Nothing here writes: no edits, no commits, no PR comments. A review the
//! agent writes is returned in conversation; posting it anywhere is the
//! operator's call.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CodeConfig {
    /// Read-only checkout of the source (empty turns the code tools off).
    pub root: String,
    /// `owner/name` on GitHub for `pr_list` and `pr_diff` (empty turns them off).
    pub github_repo: String,
    pub github_api: String,
}

impl Default for CodeConfig {
    fn default() -> Self {
        Self { root: String::new(), github_repo: String::new(), github_api: "https://api.github.com".into() }
    }
}

const SKIP_DIRS: &[&str] = &[".git", "target", "node_modules", ".claude", ".runtime", "x", "D:"];
const TEXT_EXT: &[&str] = &["rs", "md", "toml", "html", "py", "sh", "yml", "yaml", "json", "txt", "mjs", "js", "ts", "css", "Dockerfile"];
const MAX_FILE_BYTES: u64 = 400_000;

fn err(message: impl Into<String>) -> Value {
    json!({ "error": message.into() })
}

fn is_text(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    TEXT_EXT.contains(&ext) || matches!(name, "Dockerfile" | "LICENSE.md" | "README" | ".gitignore" | ".dockerignore")
}

/// Every text file under `root`, relative paths, skipping build and VCS dirs.
fn walk(root: &Path, start: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(start) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if !SKIP_DIRS.contains(&name.as_str()) {
                walk(root, &path, out);
            }
        } else if is_text(&path) && entry.metadata().map(|m| m.len() <= MAX_FILE_BYTES).unwrap_or(false) {
            if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_path_buf());
            }
        }
    }
}

fn rel_str(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

impl crate::runtime::AgentRuntime {
    fn code_root(&self) -> Option<PathBuf> {
        let root = self.config.code.root.trim();
        (!root.is_empty()).then(|| PathBuf::from(root)).filter(|p| p.is_dir())
    }

    /// Tool calls allowed per chat message.
    pub(crate) fn max_tool_calls(&self) -> usize {
        self.config.max_tool_calls.unwrap_or(crate::runtime::MAX_TOOL_CALLS_DEFAULT).clamp(1, 16)
    }

    pub(crate) fn code_available(&self) -> bool {
        self.code_root().is_some()
    }

    fn github_available(&self) -> bool {
        !self.config.code.github_repo.trim().is_empty()
    }

    /// Resolve a relative path inside the root; refuses anything that
    /// escapes it.
    fn code_path(&self, rel: &str) -> Result<PathBuf, Value> {
        let root = self.code_root().ok_or_else(|| err("code access is not configured"))?;
        let rel = rel.trim().trim_start_matches(['/', '\\']);
        if rel.split(['/', '\\']).any(|seg| seg == "..") {
            return Err(err("paths may not contain `..`"));
        }
        let path = root.join(rel);
        let real = path.canonicalize().map_err(|_| err(format!("no such path `{rel}`")))?;
        let real_root = root.canonicalize().map_err(|e| err(format!("{e}")))?;
        if !real.starts_with(&real_root) {
            return Err(err("path is outside the code root"));
        }
        Ok(real)
    }

    pub(crate) fn code_prompt(&self) -> String {
        let mut out = String::new();
        if self.code_available() {
            out.push_str("\n\nCODE. You can read the source of Ferricula, the engine you run on (read-only; you cannot change it):\
                \n- code_tree(path?: string): list a directory (default: the top).\
                \n- code_search(query: string, path?: string): find lines in the source that match the words of `query`, best first, with file and line number. `path` narrows it to a directory or file.\
                \n- code_read(path: string, from_line?: int, lines?: int up to 400): read a file with line numbers. Quote exact lines when you judge code.");
        }
        if self.github_available() {
            out.push_str("\n- pr_list(state?: \"open\" | \"closed\" | \"all\"): the project's pull requests on GitHub.\
                \n- pr_diff(number: int, file?: string): a pull request's description and changed files with their diffs (`file` limits it to one file). When reviewing, read the diff, open the touched code with code_read for context, then give a verdict (ship it / redo it) quoting the lines that decide it. Your review comes back in this conversation; nothing is posted to GitHub.");
        }
        out
    }

    pub(super) fn tool_code(&self, name: &str, args: &Value, room: usize) -> Result<Value, Value> {
        let s = |k: &str| args.get(k).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty());
        match name {
            "code_tree" => {
                let dir = self.code_path(s("path").unwrap_or(""))?;
                let root = self.code_root().expect("checked").canonicalize().map_err(|e| err(format!("{e}")))?;
                let mut items: Vec<Value> = std::fs::read_dir(&dir).map_err(|e| err(format!("{e}")))?.flatten()
                    .filter(|e| !SKIP_DIRS.contains(&e.file_name().to_string_lossy().as_ref()))
                    .map(|e| {
                        let p = e.path();
                        let rel = rel_str(p.strip_prefix(&root).unwrap_or(&p));
                        if p.is_dir() { json!({ "dir": rel }) } else { json!({ "file": rel, "bytes": e.metadata().map(|m| m.len()).unwrap_or(0) }) }
                    }).collect();
                items.sort_by_key(|v| (v.get("file").is_some(), v.to_string()));
                items.truncate(300);
                Ok(json!({ "tool": "code_tree", "corpus": "code", "path": rel_str(dir.strip_prefix(&root).unwrap_or(&dir)), "entries": items }))
            }
            "code_search" => {
                let query = s("query").ok_or_else(|| err("argument `query` is required"))?;
                let root = self.code_root().ok_or_else(|| err("code access is not configured"))?.canonicalize().map_err(|e| err(format!("{e}")))?;
                let start = match s("path") { Some(p) => self.code_path(p)?, None => root.clone() };
                let mut files = Vec::new();
                if start.is_file() {
                    files.push(start.strip_prefix(&root).unwrap_or(&start).to_path_buf());
                } else {
                    walk(&root, &start, &mut files);
                }
                let terms: Vec<String> = query.to_lowercase().split(|c: char| !c.is_alphanumeric() && c != '_')
                    .filter(|t| t.len() >= 2).map(str::to_string).collect();
                if terms.is_empty() {
                    return Err(err("`query` has no searchable words"));
                }
                let mut hits: Vec<(f32, String, usize, String)> = Vec::new();
                for rel in &files {
                    let Ok(text) = std::fs::read_to_string(root.join(rel)) else { continue };
                    let path_l = rel_str(rel).to_lowercase();
                    let path_bonus = terms.iter().filter(|t| path_l.contains(t.as_str())).count() as f32 * 0.5;
                    for (n, line) in text.lines().enumerate() {
                        let l = line.to_lowercase();
                        let matched = terms.iter().filter(|t| l.contains(t.as_str())).count();
                        if matched == 0 {
                            continue;
                        }
                        let score = matched as f32 * 2.0 + if matched == terms.len() { 3.0 } else { 0.0 } + path_bonus
                            + if l.contains(&query.to_lowercase()) { 4.0 } else { 0.0 };
                        hits.push((score, rel_str(rel), n + 1, crate::recall::truncate_bytes(line.trim(), 220).to_string()));
                    }
                }
                hits.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
                let total = hits.len();
                let max = ((room / 260).clamp(5, 40)).min(total);
                let results: Vec<Value> = hits.into_iter().take(max)
                    .map(|(_, path, line, text)| json!({ "path": path, "line": line, "text": text })).collect();
                Ok(json!({ "tool": "code_search", "corpus": "code", "query": query, "matches": total, "results": results,
                    "note": "Lines matching your words; open one with code_read for its context." }))
            }
            "code_read" => {
                let rel = s("path").ok_or_else(|| err("argument `path` is required"))?;
                let path = self.code_path(rel)?;
                if path.is_dir() {
                    return Err(err(format!("`{rel}` is a directory; use code_tree")));
                }
                let text = std::fs::read_to_string(&path).map_err(|_| err(format!("`{rel}` is not a readable text file")))?;
                let lines: Vec<&str> = text.lines().collect();
                let from = args.get("from_line").and_then(Value::as_u64).unwrap_or(1).max(1) as usize;
                let want = args.get("lines").and_then(Value::as_u64).unwrap_or(200).clamp(1, 400) as usize;
                let mut body = String::new();
                let mut last = from.saturating_sub(1);
                for (i, line) in lines.iter().enumerate().skip(from - 1).take(want) {
                    let piece = format!("{:>5} {line}\n", i + 1);
                    if body.len() + piece.len() > room.saturating_sub(800).max(2000) {
                        break;
                    }
                    body.push_str(&piece);
                    last = i + 1;
                }
                Ok(json!({ "tool": "code_read", "corpus": "code", "path": rel, "from_line": from, "to_line": last,
                    "total_lines": lines.len(), "text": body, "complete": from == 1 && last >= lines.len(),
                    "next_from": (last < lines.len()).then_some(last + 1) }))
            }
            "pr_list" | "pr_diff" => {
                if !self.github_available() {
                    return Err(err("pull requests are not configured"));
                }
                let base = format!("{}/repos/{}", self.config.code.github_api.trim_end_matches('/'), self.config.code.github_repo.trim());
                let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(20))
                    .user_agent("ferricula-agent").build().map_err(|e| err(format!("{e}")))?;
                let get = |url: String| -> Result<Value, Value> {
                    let r = client.get(&url).header("Accept", "application/vnd.github+json").send().map_err(|e| err(format!("GitHub unreachable: {e}")))?;
                    let status = r.status();
                    let v: Value = r.json().map_err(|e| err(format!("GitHub reply was not JSON: {e}")))?;
                    if !status.is_success() {
                        return Err(err(format!("GitHub http {}: {}", status.as_u16(), v["message"].as_str().unwrap_or(""))));
                    }
                    Ok(v)
                };
                if name == "pr_list" {
                    let state = s("state").filter(|v| matches!(*v, "open" | "closed" | "all")).unwrap_or("open");
                    let prs = get(format!("{base}/pulls?state={state}&per_page=30"))?;
                    let rows: Vec<Value> = prs.as_array().cloned().unwrap_or_default().iter().map(|p| json!({
                        "number": p["number"], "title": p["title"], "author": p["user"]["login"], "branch": p["head"]["ref"],
                        "base": p["base"]["ref"], "draft": p["draft"], "updated": p["updated_at"], "url": p["html_url"],
                    })).collect();
                    return Ok(json!({ "tool": "pr_list", "corpus": "code", "state": state, "pull_requests": rows }));
                }
                let number = args.get("number").and_then(Value::as_u64).ok_or_else(|| err("argument `number` is required"))?;
                let pr = get(format!("{base}/pulls/{number}"))?;
                let files = get(format!("{base}/pulls/{number}/files?per_page=100"))?;
                let only = s("file");
                let mut budget = room.saturating_sub(2500).max(3000);
                let mut out = Vec::new();
                let mut cut = false;
                for f in files.as_array().cloned().unwrap_or_default() {
                    let fname = f["filename"].as_str().unwrap_or("");
                    if only.is_some_and(|o| !fname.contains(o)) {
                        continue;
                    }
                    let patch = f["patch"].as_str().unwrap_or("(no textual diff: binary or too large)");
                    let take = crate::recall::truncate_bytes(patch, budget.min(patch.len()));
                    budget = budget.saturating_sub(take.len() + 200);
                    cut |= take.len() < patch.len();
                    out.push(json!({ "file": fname, "status": f["status"], "additions": f["additions"], "deletions": f["deletions"],
                        "patch": take, "patch_cut": take.len() < patch.len() }));
                    if budget < 400 {
                        cut = true;
                        break;
                    }
                }
                Ok(json!({ "tool": "pr_diff", "corpus": "code", "number": number, "title": pr["title"], "author": pr["user"]["login"],
                    "branch": pr["head"]["ref"], "base": pr["base"]["ref"], "url": pr["html_url"],
                    "description": crate::recall::truncate_bytes(pr["body"].as_str().unwrap_or(""), 3000),
                    "changed_files": pr["changed_files"], "files": out, "fragment": cut,
                    "note": "If the diff was cut, call pr_diff again with `file` for one file, and code_read for context." }))
            }
            other => Err(err(format!("unknown code tool `{other}`"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_files_are_recognized() {
        assert!(is_text(Path::new("crates/x/src/lib.rs")));
        assert!(is_text(Path::new("Dockerfile")));
        assert!(!is_text(Path::new("audit/life/dream.png")));
    }
}
