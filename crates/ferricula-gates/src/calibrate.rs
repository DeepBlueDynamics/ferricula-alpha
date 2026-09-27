//! Temperature scaling for gate probabilities (plan §6.2), and the
//! calibration-file format read by [`Calibration::load`].
//!
//! A backend returns a distribution p over K options (K = 2 for yes/no:
//! [1 - p, p]). Temperature scaling treats log p as logits and rescales:
//! p'_k = softmax(log p_k / T). T > 1 flattens, T < 1 sharpens, T = 1 is the
//! identity. The argmax never changes, so accuracy is unchanged; only the
//! confidence moves. T is fitted by minimising the mean negative
//! log-likelihood of the gold labels on a train split; the NLL is convex in
//! 1/T, so a log-spaced grid followed by golden-section refinement finds the
//! global minimum within the bounds.

use serde::{Deserialize, Serialize};

use crate::Calibration;

pub const T_MIN: f64 = 0.05;
pub const T_MAX: f64 = 20.0;
const P_FLOOR: f64 = 1e-6;

/// p' = softmax(log p / t). Inputs need not be normalised; zeros are floored.
pub fn temper(probs: &[f64], t: f64) -> Vec<f64> {
    let t = t.clamp(T_MIN, T_MAX);
    let z: Vec<f64> = probs.iter().map(|p| p.max(P_FLOOR).ln() / t).collect();
    let max = z.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let e: Vec<f64> = z.iter().map(|x| (x - max).exp()).collect();
    let s: f64 = e.iter().sum();
    e.into_iter().map(|x| x / s).collect()
}

/// Binary form: p(yes)' = temper([1 - p, p], t)[1].
pub fn temper_binary(p: f64, t: f64) -> f64 {
    temper(&[1.0 - p, p], t)[1]
}

/// Mean negative log-likelihood of `labels` under tempered `probs`.
pub fn nll(probs: &[Vec<f64>], labels: &[usize], t: f64) -> f64 {
    if probs.is_empty() {
        return f64::NAN;
    }
    probs.iter().zip(labels)
        .map(|(p, &y)| -temper(p, t)[y].max(1e-12).ln())
        .sum::<f64>() / probs.len() as f64
}

/// Fit T in [T_MIN, T_MAX] minimising [`nll`]. Returns 1.0 for empty input.
pub fn fit_temperature(probs: &[Vec<f64>], labels: &[usize]) -> f64 {
    if probs.is_empty() {
        return 1.0;
    }
    let f = |log_t: f64| nll(probs, labels, log_t.exp());
    let (lo, hi) = (T_MIN.ln(), T_MAX.ln());
    let steps = 120;
    let grid: Vec<f64> = (0..=steps).map(|i| lo + (hi - lo) * i as f64 / steps as f64).collect();
    let best = grid.iter().cloned().enumerate()
        .min_by(|a, b| f(a.1).partial_cmp(&f(b.1)).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i).unwrap_or(0);
    let (mut a, mut b) = (grid[best.saturating_sub(1)], grid[(best + 1).min(steps)]);
    let g = (5f64.sqrt() - 1.0) / 2.0;
    let (mut c, mut d) = (b - g * (b - a), a + g * (b - a));
    for _ in 0..60 {
        if f(c) < f(d) { b = d; } else { a = c; }
        c = b - g * (b - a);
        d = a + g * (b - a);
    }
    ((a + b) / 2.0).exp()
}

/// Top-label ECE with `bins` equal-width bins (same definition as
/// `ferricula-bench` metrics::ece): items are (confidence, correct).
pub fn ece(items: &[(f64, bool)], bins: usize) -> f64 {
    if items.is_empty() || bins == 0 {
        return f64::NAN;
    }
    let mut count = vec![0usize; bins];
    let mut conf = vec![0.0; bins];
    let mut hits = vec![0.0; bins];
    for &(c, ok) in items {
        let c = c.clamp(0.0, 1.0);
        let b = ((c * bins as f64).floor() as usize).min(bins - 1);
        count[b] += 1;
        conf[b] += c;
        hits[b] += ok as u8 as f64;
    }
    (0..bins).filter(|&b| count[b] > 0)
        .map(|b| count[b] as f64 / items.len() as f64 * (hits[b] - conf[b]).abs() / count[b] as f64)
        .sum()
}

/// Lower-case hex sha256 of raw bytes (the calibration-file digest).
pub fn sha256_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// (top-label confidence, argmax == label) for each item.
pub fn top_label(probs: &[Vec<f64>], labels: &[usize]) -> Vec<(f64, bool)> {
    probs.iter().zip(labels).map(|(p, &y)| {
        let (k, c) = p.iter().cloned().enumerate()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or((0, 0.0));
        (c, k == y)
    }).collect()
}

/// A calibration file as written by the fitter. Only `temperature` and
/// `lifecycle_authorized` drive behaviour; the rest is the audit record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationFile {
    pub gate: String,
    pub temperature: f64,
    pub lifecycle_authorized: bool,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub record: serde_json::Value,
}

impl Calibration {
    /// Parse calibration-file bytes; `sha256` is the digest of exactly these
    /// bytes. Accepts the fitter's format and the older harness format
    /// (`temperature: [T, ...]`, `acceptance.lifecycle_authorized`).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let v: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| format!("calibration json: {e}"))?;
        let temperature = match &v["temperature"] {
            serde_json::Value::Number(n) => n.as_f64(),
            serde_json::Value::Array(a) => a.first().and_then(|x| x.as_f64()),
            _ => None,
        }.ok_or("calibration file has no temperature")?;
        if !(T_MIN..=T_MAX).contains(&temperature) {
            return Err(format!("temperature {temperature} outside [{T_MIN}, {T_MAX}]"));
        }
        let lifecycle_authorized = v["lifecycle_authorized"].as_bool()
            .or_else(|| v["acceptance"]["lifecycle_authorized"].as_bool())
            .unwrap_or(false);
        Ok(Calibration {
            sha256: sha256_bytes(bytes),
            temperature: temperature as f32,
            lifecycle_authorized,
        })
    }

    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, String> {
        let bytes = std::fs::read(path.as_ref()).map_err(|e| format!("read {}: {e}", path.as_ref().display()))?;
        Self::from_bytes(&bytes)
    }

    /// Rescale a distribution with this file's temperature.
    pub fn apply(&self, probs: &[f32]) -> Vec<f32> {
        let p: Vec<f64> = probs.iter().map(|&x| x as f64).collect();
        temper(&p, self.temperature as f64).into_iter().map(|x| x as f32).collect()
    }

    pub fn apply_binary(&self, p: f32) -> f32 {
        temper_binary(p as f64, self.temperature as f64) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temper_identity_sharpen_flatten() {
        let p = [0.6, 0.3, 0.1];
        let same = temper(&p, 1.0);
        for (a, b) in same.iter().zip(p) {
            assert!((a - b).abs() < 1e-12);
        }
        // T = 0.5 squares then renormalises: 0.36 / (0.36 + 0.09 + 0.01).
        assert!((temper(&p, 0.5)[0] - 0.36 / 0.46).abs() < 1e-12);
        // T = 2 takes square roots.
        let s = 0.6f64.sqrt() + 0.3f64.sqrt() + 0.1f64.sqrt();
        assert!((temper(&p, 2.0)[0] - 0.6f64.sqrt() / s).abs() < 1e-12);
    }

    #[test]
    fn temper_binary_matches_logit_scaling() {
        // sigmoid(logit(p) / T) is the binary special case.
        let (p, t) = (0.8f64, 2.0);
        let expected = 1.0 / (1.0 + (-(p / (1.0 - p)).ln() / t).exp());
        assert!((temper_binary(p, t) - expected).abs() < 1e-12);
        assert!((temper_binary(0.5, 7.0) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn nll_by_hand() {
        let probs = vec![vec![0.8, 0.2], vec![0.4, 0.6]];
        let labels = [0, 0];
        let expected = -(0.8f64.ln() + 0.4f64.ln()) / 2.0;
        assert!((nll(&probs, &labels, 1.0) - expected).abs() < 1e-12);
    }

    #[test]
    fn fit_recovers_known_temperature() {
        // Build a set whose empirical frequencies equal temper(p, 1/2): a
        // model that is under-confident by T* = 0.5. Items: raw p(yes) = 0.7;
        // the true rate is temper_binary(0.7, 0.5) = 0.49 / 0.58 ~ 0.845, so
        // 845 of 1000 are "yes". The NLL optimum is T = 0.5 (up to rounding
        // of the count).
        let mut probs = Vec::new();
        let mut labels = Vec::new();
        for i in 0..1000 {
            probs.push(vec![0.3, 0.7]);
            labels.push(if i < 845 { 1 } else { 0 });
        }
        let t = fit_temperature(&probs, &labels);
        assert!((temper_binary(0.7, t) - 0.845).abs() < 1e-4, "t = {t}");
        assert!((t - 0.5).abs() < 0.01, "t = {t}");
    }

    #[test]
    fn fit_flattens_overconfidence() {
        // Says 0.95 but is right 60% of the time: T must exceed 1.
        let mut probs = Vec::new();
        let mut labels = Vec::new();
        for i in 0..100 {
            probs.push(vec![0.05, 0.95]);
            labels.push(if i < 60 { 1 } else { 0 });
        }
        let t = fit_temperature(&probs, &labels);
        assert!(t > 1.0);
        assert!((temper_binary(0.95, t) - 0.6).abs() < 1e-3);
        let raw = ece(&top_label(&probs, &labels), 15);
        let cal: Vec<Vec<f64>> = probs.iter().map(|p| temper(p, t)).collect();
        assert!((raw - 0.35).abs() < 1e-9);
        assert!(ece(&top_label(&cal, &labels), 15) < 1e-3);
    }

    #[test]
    fn fit_is_bounded() {
        // Always right with p = 0.9: NLL keeps falling as T -> 0.
        let probs = vec![vec![0.1, 0.9]; 10];
        let t = fit_temperature(&probs, &[1; 10]);
        assert!((T_MIN..0.1).contains(&t), "t = {t}");
        assert_eq!(fit_temperature(&[], &[]), 1.0);
    }

    #[test]
    fn ece_by_hand() {
        // Two bins used: [0.9 right, 0.9 wrong] -> |0.5 - 0.9| * 0.5;
        // [0.3 right, 0.3 right] -> |1 - 0.3| * 0.5.
        let items = [(0.9, true), (0.9, false), (0.3, true), (0.3, true)];
        assert!((ece(&items, 10) - (0.4 * 0.5 + 0.7 * 0.5)).abs() < 1e-12);
        assert!(ece(&[], 10).is_nan());
    }

    #[test]
    fn calibration_file_roundtrip_and_legacy() {
        let file = CalibrationFile {
            gate: "worth_researching".into(),
            temperature: 1.7,
            lifecycle_authorized: true,
            note: String::new(),
            record: serde_json::json!({"test_ece": 0.03}),
        };
        let bytes = serde_json::to_vec_pretty(&file).unwrap();
        let c = Calibration::from_bytes(&bytes).unwrap();
        assert!((c.temperature - 1.7).abs() < 1e-6);
        assert!(c.lifecycle_authorized);
        assert_eq!(c.sha256, sha256_bytes(&bytes));
        assert_eq!(sha256_bytes(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let legacy = br#"{"temperature": [0.54, 0.54, 0.54], "acceptance": {"lifecycle_authorized": false}}"#;
        let c = Calibration::from_bytes(legacy).unwrap();
        assert!((c.temperature - 0.54).abs() < 1e-6);
        assert!(!c.lifecycle_authorized);
        assert!(Calibration::from_bytes(br#"{"temperature": 0.0}"#).is_err());
        assert!((c.apply_binary(0.5) - 0.5).abs() < 1e-6);
        let v = c.apply(&[0.7, 0.2, 0.1]);
        assert!((v.iter().sum::<f32>() - 1.0).abs() < 1e-5);
    }
}
