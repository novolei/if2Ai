//! HRR operations: bind, unbind, bundle, similarity
//!
//! Implements circular convolution for binding and its inverse for unbinding.
//! Bundle performs weighted superposition with normalization.

use serde::{Deserialize, Serialize};

/// HRR vector using phase encoding
///
/// Dimension matches FastEmbed (384d) for compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HRRVector(pub Vec<f32>);

impl HRRVector {
    /// Create a zero vector of the given dimension
    #[must_use]
    pub fn new(dimension: usize) -> Self {
        Self(vec![0.0; dimension])
    }

    /// Returns the vector dimension
    #[must_use]
    pub fn dimension(&self) -> usize {
        self.0.len()
    }

    /// Consume and return the inner vector
    #[must_use]
    pub fn into_inner(self) -> Vec<f32> {
        self.0
    }

    /// Normalize to unit length in-place
    fn normalize(&mut self) {
        let norm: f32 = self.0.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            self.0.iter_mut().for_each(|x| *x /= norm);
        }
    }
}

/// Bind two vectors using circular convolution
///
/// Wraps `value` into `key`'s binding space. To retrieve:
/// `unbind(&bind(key, value), key)` ≈ `value`.
#[must_use]
pub fn bind(key: &HRRVector, value: &HRRVector) -> HRRVector {
    circular_convolution(key, value)
}

/// Unbind a value from a composite using inverse circular convolution
///
/// Retrieves the bound value: `unbind(&bind(key, value), key)` ≈ `value`.
/// For HRR: inverse = reverse the key (conjugate in Fourier domain).
#[must_use]
pub fn unbind(composite: &HRRVector, key: &HRRVector) -> HRRVector {
    circular_convolution_inverse(composite, key)
}

/// Superpose multiple vectors with optional weights
///
/// Returns a normalized weighted sum. If no weights provided,
/// all vectors contribute equally.
#[must_use]
pub fn bundle(vectors: &[HRRVector], weights: Option<&[f32]>) -> HRRVector {
    let dim = vectors.first().map(|v| v.dimension()).unwrap_or(0);
    if dim == 0 {
        return HRRVector::new(0);
    }

    let default_weights = vec![1.0; vectors.len()];
    let w = weights.unwrap_or(&default_weights);

    let mut result = vec![0.0; dim];
    for (v, weight) in vectors.iter().zip(w.iter()) {
        for (i, val) in v.0.iter().enumerate() {
            result[i] += val * weight;
        }
    }

    let mut vec = HRRVector(result);
    vec.normalize();
    vec
}

/// Cosine similarity between two HRR vectors
///
/// Returns a value in [-1, 1]. Values near 1 indicate high similarity,
/// values near -1 indicate opposition, and values near 0 indicate
/// orthogonality (no relation).
#[must_use]
pub fn similarity(a: &HRRVector, b: &HRRVector) -> f32 {
    let dot: f32 = a.0.iter().zip(b.0.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.0.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.0.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// Circular convolution (bind operation)
///
/// O(n²) complexity. For 384d, this is ~147K operations — acceptable
/// for interactive use but not for high-throughput batch processing.
fn circular_convolution(a: &HRRVector, b: &HRRVector) -> HRRVector {
    let dim = a.dimension();
    let mut result = vec![0.0; dim];

    for (i, res) in result.iter_mut().enumerate() {
        for j in 0..dim {
            *res += a.0[j] * b.0[(i + dim - j) % dim];
        }
    }

    let mut vec = HRRVector(result);
    vec.normalize();
    vec
}

/// Inverse circular convolution (unbind operation)
///
/// Key inversion: reverse the key for conjugate in the Fourier domain.
/// This allows retrieving the bound value from the composite.
fn circular_convolution_inverse(composite: &HRRVector, key: &HRRVector) -> HRRVector {
    let dim = composite.dimension();

    // Key inversion: reverse order
    let key_inv: Vec<f32> = (0..dim).map(|i| key.0[(dim - i) % dim]).collect();

    let mut result = vec![0.0; dim];
    for (i, res) in result.iter_mut().enumerate() {
        for j in 0..dim {
            *res += composite.0[j] * key_inv[(i + dim - j) % dim];
        }
    }

    HRRVector(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_vec(values: &[f32]) -> HRRVector {
        HRRVector(values.to_vec())
    }

    #[test]
    fn hrr_vector_new() {
        let v = HRRVector::new(384);
        assert_eq!(v.dimension(), 384);
        assert!(v.0.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn hrr_vector_into_inner() {
        let v = make_vec(&[1.0, 2.0, 3.0]);
        let inner = v.into_inner();
        assert_eq!(inner, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn similarity_identical_is_one() {
        let v = make_vec(&[1.0, 0.0, 0.0]);
        let sim = similarity(&v, &v);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn similarity_opposite_is_negative() {
        let a = make_vec(&[1.0, 0.0, 0.0]);
        let b = make_vec(&[-1.0, 0.0, 0.0]);
        let sim = similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn similarity_orthogonal_is_zero() {
        let a = make_vec(&[1.0, 0.0]);
        let b = make_vec(&[0.0, 1.0]);
        let sim = similarity(&a, &b);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn similarity_zero_vectors_returns_zero() {
        let a = HRRVector::new(4);
        let b = make_vec(&[1.0, 0.0, 0.0, 0.0]);
        let sim = similarity(&a, &b);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn bind_unbind_roundtrip() {
        let key = make_vec(&[0.6, 0.8, 0.0, 0.0]);
        let value = make_vec(&[0.0, 0.0, 0.7, 0.7]);
        let composite = bind(&key, &value);
        let retrieved = unbind(&composite, &key);
        let sim = similarity(&retrieved, &value);
        // HRR is lossy — similarity should be > 0.5 for simple vectors
        assert!(sim > 0.5, "roundtrip similarity {sim} should be > 0.5");
    }

    #[test]
    fn bundle_preserves_components() {
        let a = make_vec(&[1.0, 0.0, 0.0, 0.0]);
        let b = make_vec(&[0.0, 1.0, 0.0, 0.0]);
        let bundled = bundle(&[a, b], None);
        // Both components should have positive similarity with the bundle
        let sim_a = similarity(&bundled, &make_vec(&[1.0, 0.0, 0.0, 0.0]));
        let sim_b = similarity(&bundled, &make_vec(&[0.0, 1.0, 0.0, 0.0]));
        assert!(sim_a > 0.0, "bundle should contain a: sim={sim_a}");
        assert!(sim_b > 0.0, "bundle should contain b: sim={sim_b}");
    }

    #[test]
    fn bundle_with_weights() {
        let a = make_vec(&[1.0, 0.0]);
        let b = make_vec(&[0.0, 1.0]);
        let bundled = bundle(&[a, b], Some(&[1.0, 0.1]));
        // a should have higher similarity than b
        let sim_a = similarity(&bundled, &make_vec(&[1.0, 0.0]));
        let sim_b = similarity(&bundled, &make_vec(&[0.0, 1.0]));
        assert!(sim_a > sim_b, "weighted a ({sim_a}) should > b ({sim_b})");
    }

    #[test]
    fn bundle_empty_returns_zero() {
        let bundled = bundle(&[], None);
        assert_eq!(bundled.dimension(), 0);
    }

    #[test]
    fn bind_dimension_preserved() {
        let a = HRRVector::new(384);
        let b = HRRVector::new(384);
        let bound = bind(&a, &b);
        assert_eq!(bound.dimension(), 384);
    }
}
