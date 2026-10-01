//! Geometric trust: vector encryption via seed-derived permutation + sign flip.
//!
//! Preserves cosine similarity when the same key is used for both vectors.
//! Destroys similarity when keys differ. O(n) space and time.

use anyhow::{Result, anyhow};

/// A lightweight orthogonal transform: permutation + sign flips.
/// Equivalent to a sparse orthogonal matrix but O(n) not O(n^2).
#[derive(Debug, Clone)]
pub struct VectorTransform {
    /// Permutation: position i maps to perm[i]
    pub perm: Vec<usize>,
    /// Sign flips: +1.0 or -1.0 per dimension
    pub signs: Vec<f64>,
}

impl VectorTransform {
    /// Build a transform from a 32-byte seed for the given dimension.
    /// Uses Fisher-Yates shuffle seeded by the key bytes.
    pub fn from_seed(seed: &[u8; 32], dim: usize) -> Result<Self> {
        if dim == 0 {
            return Err(anyhow!("dimension must be > 0"));
        }

        // Expand seed into a deterministic byte stream via repeated hashing
        let mut rng_state = *seed;
        let mut next_byte = move || -> u8 {
            // Simple deterministic PRNG: rotate and mix
            let out = rng_state[0];
            // Rotate left by 1 and XOR with a constant
            rng_state.rotate_left(1);
            for i in 0..32 {
                rng_state[i] = rng_state[i].wrapping_add(rng_state[(i + 7) % 32]).wrapping_mul(31);
            }
            out
        };

        // Fisher-Yates shuffle for permutation
        let mut perm: Vec<usize> = (0..dim).collect();
        for i in (1..dim).rev() {
            // Generate index in [0, i] from two random bytes
            let hi = next_byte() as usize;
            let lo = next_byte() as usize;
            let j = ((hi << 8) | lo) % (i + 1);
            perm.swap(i, j);
        }

        // Sign flips from random bits
        let signs: Vec<f64> = (0..dim)
            .map(|_| if next_byte() & 1 == 0 { 1.0 } else { -1.0 })
            .collect();

        Ok(Self { perm, signs })
    }

    /// Encrypt a vector: permute then flip signs.
    ///
    /// `vec.len()` must equal the transform dimension. A longer vector is not
    /// truncated and a shorter one is not zero-filled; either is an error.
    pub fn encrypt(&self, vec: &[f32]) -> Result<Vec<f32>> {
        let dim = self.perm.len();
        if vec.len() != dim {
            return Err(anyhow!(
                "vector length {} != transform dimension {}",
                vec.len(),
                dim
            ));
        }
        let mut out = vec![0.0f32; dim];
        for i in 0..dim {
            out[self.perm[i]] = vec[i] * self.signs[i] as f32;
        }
        Ok(out)
    }

    /// Decrypt a vector: reverse sign flips then unpermute.
    ///
    /// `vec.len()` must equal the transform dimension. A mismatch is an error,
    /// not a truncated or zero-filled result.
    pub fn decrypt(&self, vec: &[f32]) -> Result<Vec<f32>> {
        let dim = self.perm.len();
        if vec.len() != dim {
            return Err(anyhow!(
                "vector length {} != transform dimension {}",
                vec.len(),
                dim
            ));
        }
        let mut out = vec![0.0f32; dim];
        for i in 0..dim {
            out[i] = vec[self.perm[i]] * self.signs[i] as f32;
        }
        Ok(out)
    }
}

// Keep the old API for backward compat but delegate to VectorTransform
pub fn orthogonal_from_seed(seed: &[u8; 32], dim: usize) -> Result<Vec<Vec<f64>>> {
    // Legacy: returns a permutation matrix as dense Vec<Vec<f64>>
    let t = VectorTransform::from_seed(seed, dim)?;
    let mut m = vec![vec![0.0f64; dim]; dim];
    for i in 0..dim {
        m[t.perm[i]][i] = t.signs[i];
    }
    Ok(m)
}

pub fn warp(vec: &[f64], ortho: &[Vec<f64>]) -> Vec<f64> {
    let dim = ortho.len();
    let mut out = vec![0.0f64; dim];
    for i in 0..dim {
        out[i] = dot(&ortho[i], vec);
    }
    out
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let seed = [42u8; 32];
        let t = VectorTransform::from_seed(&seed, 768).unwrap();
        let original: Vec<f32> = (0..768).map(|i| (i as f32) * 0.01).collect();
        let encrypted = t.encrypt(&original).unwrap();
        let decrypted = t.decrypt(&encrypted).unwrap();
        for i in 0..768 {
            assert!((original[i] - decrypted[i]).abs() < 1e-6,
                "mismatch at {}: {} vs {}", i, original[i], decrypted[i]);
        }
    }

    #[test]
    fn length_mismatch_errors() {
        let t = VectorTransform::from_seed(&[3u8; 32], 8).unwrap();
        let short = vec![1.0f32; 7];
        let long = vec![1.0f32; 9];
        assert!(t.encrypt(&short).is_err());
        assert!(t.decrypt(&short).is_err());
        assert!(t.encrypt(&long).is_err());
        assert!(t.decrypt(&long).is_err());
    }

    #[test]
    fn same_key_preserves_cosine() {
        let seed = [99u8; 32];
        let t = VectorTransform::from_seed(&seed, 768).unwrap();
        let a: Vec<f32> = (0..768).map(|i| ((i as f32) * 0.1).sin()).collect();
        let b: Vec<f32> = (0..768).map(|i| ((i as f32) * 0.1 + 0.5).sin()).collect();
        let cos_plain = cosine_f32(&a, &b);
        let cos_enc = cosine_f32(&t.encrypt(&a).unwrap(), &t.encrypt(&b).unwrap());
        assert!((cos_plain - cos_enc).abs() < 1e-5,
            "cosine diverged: {} vs {}", cos_plain, cos_enc);
    }

    #[test]
    fn different_key_destroys_cosine() {
        let t1 = VectorTransform::from_seed(&[1u8; 32], 768).unwrap();
        let t2 = VectorTransform::from_seed(&[2u8; 32], 768).unwrap();
        let a: Vec<f32> = (0..768).map(|i| ((i as f32) * 0.1).sin()).collect();
        let b: Vec<f32> = (0..768).map(|i| ((i as f32) * 0.1 + 0.5).sin()).collect();
        let cos_plain = cosine_f32(&a, &b);
        // a encrypted with key1, b encrypted with key2
        let cos_cross = cosine_f32(&t1.encrypt(&a).unwrap(), &t2.encrypt(&b).unwrap());
        // Should be very different from plain cosine
        assert!((cos_plain - cos_cross).abs() > 0.1,
            "cross-key cosine too similar: {} vs {}", cos_plain, cos_cross);
    }

    #[test]
    fn dimension_768() {
        let seed = [7u8; 32];
        let t = VectorTransform::from_seed(&seed, 768).unwrap();
        assert_eq!(t.perm.len(), 768);
        assert_eq!(t.signs.len(), 768);
        // Permutation is valid: all indices present
        let mut sorted = t.perm.clone();
        sorted.sort();
        assert_eq!(sorted, (0..768).collect::<Vec<_>>());
    }

    fn cosine_f32(a: &[f32], b: &[f32]) -> f32 {
        let mut dot = 0.0f32;
        let mut na = 0.0f32;
        let mut nb = 0.0f32;
        for (x, y) in a.iter().zip(b.iter()) {
            dot += x * y;
            na += x * x;
            nb += y * y;
        }
        dot / (na.sqrt() * nb.sqrt())
    }
}
