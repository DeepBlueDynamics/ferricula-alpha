//! The ledger: one JSON line per measured result in `<out>/ledger.jsonl`,
//! plus a human-readable markdown report per run.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Facts shared by every ledger row of one run.
#[derive(Debug, Clone)]
pub struct RunContext {
    pub run_id: String,
    pub suite: String,
    pub started_at: String,
    pub date: String,
    pub git_commit: String,
    pub git_dirty: bool,
    pub command: String,
    pub out_dir: PathBuf,
    pub seed: u64,
}

impl RunContext {
    pub fn new(suite: &str, out_dir: &Path, seed: u64) -> Self {
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let started_at = rfc3339(secs);
        let date = started_at[..10].to_string();
        let git_commit = git(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into());
        let git_dirty = git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty());
        let command = std::iter::once("ferricula-bench".to_string())
            .chain(std::env::args().skip(1))
            .collect::<Vec<_>>()
            .join(" ");
        Self {
            run_id: uuid::Uuid::new_v4().to_string(),
            suite: suite.to_string(),
            started_at,
            date,
            git_commit,
            git_dirty,
            command,
            out_dir: out_dir.to_path_buf(),
            seed,
        }
    }

    /// Append one row. `body` fields are merged over the shared run fields.
    pub fn append(&self, body: Value) -> Result<()> {
        fs::create_dir_all(&self.out_dir)?;
        let mut row = serde_json::json!({
            "ts": rfc3339(SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)),
            "run_id": self.run_id,
            "suite": self.suite,
            "git_commit": self.git_commit,
            "git_dirty": self.git_dirty,
            "command": self.command,
            "seed": self.seed,
        });
        if let (Some(row), Value::Object(body)) = (row.as_object_mut(), body) {
            row.extend(body);
        }
        let path = self.out_dir.join("ledger.jsonl");
        let mut f = OpenOptions::new().create(true).append(true).open(&path)
            .with_context(|| format!("open {}", path.display()))?;
        writeln!(f, "{}", serde_json::to_string(&row)?)?;
        Ok(())
    }

    /// Write `<out>/<suite>-<date>.md`, adding `-2`, `-3`… if taken.
    pub fn write_report(&self, markdown: &str) -> Result<PathBuf> {
        fs::create_dir_all(&self.out_dir)?;
        let mut path = self.out_dir.join(format!("{}-{}.md", self.suite, self.date));
        let mut n = 2;
        while path.exists() {
            path = self.out_dir.join(format!("{}-{}-{n}.md", self.suite, self.date));
            n += 1;
        }
        fs::write(&path, markdown)?;
        Ok(path)
    }

    /// Markdown header shared by all reports.
    pub fn report_header(&self, title: &str) -> String {
        format!(
            "# {title}\n\n| | |\n|---|---|\n| run_id | `{}` |\n| started | {} |\n| git commit | `{}`{} |\n| command | `{}` |\n| seed | {} |\n\n",
            self.run_id, self.started_at, self.git_commit,
            if self.git_dirty { " (dirty tree)" } else { "" },
            self.command, self.seed,
        )
    }
}

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha256_file(path: &Path) -> Result<String> {
    Ok(sha256_hex(&fs::read(path).with_context(|| format!("read {}", path.display()))?))
}

/// Forward-slash path for portable ledger rows.
pub fn display_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// UTC RFC 3339 from unix seconds (civil-from-days, no dependency).
pub fn rfc3339(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, (rem % 3600) / 60, rem % 60)
}

/// Deterministic PRNG (splitmix64) so samples are reproducible from a seed.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = (self.next_u64() % (i as u64 + 1)) as usize;
            v.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_known_dates() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339(1_790_380_800 + 3661), "2026-09-26T01:01:01Z");
    }

    #[test]
    fn rng_is_deterministic() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        assert_eq!(a.next_u64(), b.next_u64());
        let mut v: Vec<u32> = (0..10).collect();
        let mut w = v.clone();
        Rng::new(1).shuffle(&mut v);
        Rng::new(1).shuffle(&mut w);
        assert_eq!(v, w);
    }
}
