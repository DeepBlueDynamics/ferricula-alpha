//! Metric math. Kept small and hand-checkable; every function has a
//! fixture test with the arithmetic written out.

/// Expected calibration error with `bins` equal-width confidence bins on
/// [0, 1]. `items` are (confidence of the predicted label, was it correct).
/// A confidence of exactly 1.0 falls in the last bin. NaN when empty.
pub fn ece(items: &[(f64, bool)], bins: usize) -> f64 {
    if items.is_empty() || bins == 0 {
        return f64::NAN;
    }
    let mut count = vec![0usize; bins];
    let mut conf = vec![0.0f64; bins];
    let mut hits = vec![0.0f64; bins];
    for &(c, ok) in items {
        let c = c.clamp(0.0, 1.0);
        let b = ((c * bins as f64).floor() as usize).min(bins - 1);
        count[b] += 1;
        conf[b] += c;
        if ok {
            hits[b] += 1.0;
        }
    }
    let total = items.len() as f64;
    (0..bins)
        .filter(|&b| count[b] > 0)
        .map(|b| {
            let n = count[b] as f64;
            (n / total) * (hits[b] / n - conf[b] / n).abs()
        })
        .sum()
}

/// Top-label Brier score: mean of (confidence - 1[correct])^2. Usable when
/// a backend exposes only the winning label's probability.
pub fn brier_top(items: &[(f64, bool)]) -> f64 {
    if items.is_empty() {
        return f64::NAN;
    }
    items.iter().map(|&(c, ok)| (c - if ok { 1.0 } else { 0.0 }).powi(2)).sum::<f64>() / items.len() as f64
}

/// Multiclass Brier score: mean over items of sum_k (p_k - 1[k = label])^2.
/// For a binary task given as [p(no), p(yes)] this is twice the classic
/// binary Brier; use [`brier_binary`] for the classic form.
pub fn brier_multi(probs: &[Vec<f64>], labels: &[usize]) -> f64 {
    if probs.is_empty() {
        return f64::NAN;
    }
    probs.iter().zip(labels)
        .map(|(p, &y)| p.iter().enumerate().map(|(k, &pk)| (pk - if k == y { 1.0 } else { 0.0 }).powi(2)).sum::<f64>())
        .sum::<f64>() / probs.len() as f64
}

/// Classic binary Brier: mean (p(yes) - y)^2.
pub fn brier_binary(items: &[(f64, bool)]) -> f64 {
    if items.is_empty() {
        return f64::NAN;
    }
    items.iter().map(|&(p, y)| (p - if y { 1.0 } else { 0.0 }).powi(2)).sum::<f64>() / items.len() as f64
}

/// Mean reciprocal rank. `ranks` are 1-based ranks of the gold item, None
/// when it was not retrieved within the cutoff (contributes 0).
pub fn mrr(ranks: &[Option<usize>]) -> f64 {
    if ranks.is_empty() {
        return f64::NAN;
    }
    ranks.iter().map(|r| r.map(|r| 1.0 / r as f64).unwrap_or(0.0)).sum::<f64>() / ranks.len() as f64
}

/// Fraction of queries whose gold item is at rank <= k.
pub fn recall_at_k(ranks: &[Option<usize>], k: usize) -> f64 {
    if ranks.is_empty() {
        return f64::NAN;
    }
    ranks.iter().filter(|r| r.is_some_and(|r| r <= k)).count() as f64 / ranks.len() as f64
}

/// Nearest-rank percentile (p in (0, 100]). NaN when empty.
pub fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    let rank = ((p / 100.0) * v.len() as f64).ceil().max(1.0) as usize;
    v[rank.min(v.len()) - 1]
}

pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// Round for JSON output; NaN becomes null.
pub fn j(x: f64) -> serde_json::Value {
    if x.is_finite() {
        serde_json::json!((x * 10_000.0).round() / 10_000.0)
    } else {
        serde_json::Value::Null
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    #[test]
    fn ece_hand_computed() {
        // 10 bins. Bin 9 holds (0.9, T), (0.9, F): acc 0.5, conf 0.9, gap 0.4, weight 2/4.
        // Bin 2 holds (0.2, F), (0.2, F): acc 0, conf 0.2, gap 0.2, weight 2/4.
        // ECE = 0.5*0.4 + 0.5*0.2 = 0.3
        let items = [(0.9, true), (0.9, false), (0.2, false), (0.2, false)];
        close(ece(&items, 10), 0.3);
        // 15 bins: 0.9 -> bin 13, 0.2 -> bin 3; same grouping, same answer.
        close(ece(&items, 15), 0.3);
    }

    #[test]
    fn ece_perfect_and_edge_cases() {
        // conf 1.0 lands in the last bin and is right: zero error.
        close(ece(&[(1.0, true), (1.0, true)], 15), 0.0);
        // 0.75 conf with 3/4 correct in one bin: zero error.
        close(ece(&[(0.75, true), (0.75, true), (0.75, true), (0.75, false)], 15), 0.0);
        // single bin mixing confidences: bin covers [0.6, 0.6667) at 15 bins.
        // (0.61,T),(0.65,F): acc 0.5, mean conf 0.63 -> 0.13
        close(ece(&[(0.61, true), (0.65, false)], 15), 0.13);
        assert!(ece(&[], 15).is_nan());
    }

    #[test]
    fn brier_hand_computed() {
        // (0.9-1)^2=0.01, (0.9-0)^2=0.81, (0.2)^2=0.04, 0.04 -> 0.90/4 = 0.225
        let items = [(0.9, true), (0.9, false), (0.2, false), (0.2, false)];
        close(brier_top(&items), 0.225);
        // multiclass: [0.7,0.2,0.1] label 0 -> 0.09+0.04+0.01 = 0.14;
        // [0.5,0.5,0.0] label 2 -> 0.25+0.25+1 = 1.5; mean 0.82
        close(brier_multi(&[vec![0.7, 0.2, 0.1], vec![0.5, 0.5, 0.0]], &[0, 2]), 0.82);
        // binary: p=0.8,y=1 -> 0.04; p=0.3,y=0 -> 0.09; mean 0.065
        close(brier_binary(&[(0.8, true), (0.3, false)]), 0.065);
    }

    #[test]
    fn mrr_and_recall_hand_computed() {
        let ranks = [Some(1), Some(2), None, Some(5)];
        // (1 + 0.5 + 0 + 0.2) / 4 = 0.425
        close(mrr(&ranks), 0.425);
        close(recall_at_k(&ranks, 1), 0.25);
        close(recall_at_k(&ranks, 2), 0.5);
        close(recall_at_k(&ranks, 5), 0.75);
        close(recall_at_k(&ranks, 4), 0.5);
    }

    #[test]
    fn percentile_nearest_rank() {
        let v: Vec<f64> = (1..=10).map(f64::from).collect();
        close(percentile(&v, 50.0), 5.0);
        close(percentile(&v, 95.0), 10.0);
        close(percentile(&[3.0], 95.0), 3.0);
        close(mean(&v), 5.5);
    }
}
