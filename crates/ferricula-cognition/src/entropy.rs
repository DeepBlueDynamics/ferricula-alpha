//! Where randomness comes from, recorded with every draw.
//!
//! Physical noise from the radio (gnosis-radio / sdr-rand `/api/entropy`)
//! when one is reachable; the operating system's generator otherwise. The
//! source travels with the value so a dream or a curiosity pick can always
//! say whether the universe or the kernel chose it. After a failed radio
//! read the source backs off instead of paying the timeout on every draw.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::clock::fetch_radio_entropy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntropyKind {
    Radio,
    Os,
}

impl EntropyKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EntropyKind::Radio => "radio",
            EntropyKind::Os => "os",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draw {
    pub value: u64,
    pub source: EntropyKind,
}

pub struct EntropySource {
    radio_url: Option<String>,
    backoff: Duration,
    radio_down_until: Mutex<Option<Instant>>,
}

impl EntropySource {
    /// `radio_url` like `http://127.0.0.1:9080`; `None` uses the OS only.
    pub fn new(radio_url: Option<String>) -> Self {
        Self {
            radio_url: radio_url.filter(|u| !u.trim().is_empty()),
            backoff: Duration::from_secs(60),
            radio_down_until: Mutex::new(None),
        }
    }

    /// Radio from `RADIO_URL` if set.
    pub fn from_env() -> Self {
        Self::new(std::env::var("RADIO_URL").ok())
    }

    pub fn draw(&self) -> Draw {
        if let Some(bytes) = self.radio_bytes(8) {
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&bytes[..8]);
            return Draw { value: u64::from_le_bytes(buf), source: EntropyKind::Radio };
        }
        Draw { value: os_u64(), source: EntropyKind::Os }
    }

    fn radio_bytes(&self, n: usize) -> Option<Vec<u8>> {
        let url = self.radio_url.as_ref()?;
        let mut down = self.radio_down_until.lock().expect("entropy lock poisoned");
        if down.is_some_and(|until| Instant::now() < until) {
            return None;
        }
        let fetched = if url.trim_start().starts_with("https://") {
            fetch_https(url, n)
        } else {
            fetch_radio_entropy(url, n)
        };
        match fetched.filter(|b| b.len() >= n) {
            Some(bytes) => {
                *down = None;
                Some(bytes)
            }
            None => {
                *down = Some(Instant::now() + self.backoff);
                None
            }
        }
    }
}

/// `GET {url}/api/entropy?bytes=n&format=json` over TLS (e.g. the public
/// sdr-rand relay). The plain-socket client in `clock` only speaks HTTP.
fn fetch_https(url: &str, n: usize) -> Option<Vec<u8>> {
    let endpoint = format!("{}/api/entropy?bytes={n}&format=json", url.trim().trim_end_matches('/'));
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(3)).build();
    let body: serde_json::Value = agent.get(&endpoint).call().ok()?.into_json().ok()?;
    let bytes = crate::clock::hex_decode(body.get("entropy_hex")?.as_str()?);
    (!bytes.is_empty()).then_some(bytes)
}

fn os_u64() -> u64 {
    let mut buf = [0u8; 8];
    getrandom::getrandom(&mut buf).expect("operating system random source unavailable");
    u64::from_le_bytes(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_radio_draws_come_from_the_os() {
        let source = EntropySource::new(None);
        let a = source.draw();
        let b = source.draw();
        assert_eq!(a.source, EntropyKind::Os);
        assert_ne!(a.value, b.value);
    }

    #[test]
    fn unreachable_radio_falls_back_and_backs_off() {
        let source = EntropySource::new(Some("http://127.0.0.1:9".into()));
        assert_eq!(source.draw().source, EntropyKind::Os);
        let started = Instant::now();
        for _ in 0..50 {
            assert_eq!(source.draw().source, EntropyKind::Os);
        }
        // Backed off: no further 500 ms connection attempts.
        assert!(started.elapsed() < Duration::from_millis(400));
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;

    /// `RADIO_URL=https://sdrrand.nuts.services cargo test -p ferricula-cognition -- --ignored radio`
    #[test]
    #[ignore]
    fn radio_draw_from_env() {
        let draw = EntropySource::from_env().draw();
        println!("{draw:?}");
        assert_eq!(draw.source, EntropyKind::Radio);
    }
}
