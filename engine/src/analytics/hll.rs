//! Minimal HyperLogLog implementation for distributed unique user counting.
//!
//! Precision p=14: 16,384 registers, ~0.8% error, 16KB per sketch.
//! Uses SHA-256 for hashing (already a dependency via `sha2` crate).

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use sha2::{Digest, Sha256};

/// HLL precision: 14 bits for register index = 16384 registers.
const PRECISION: u32 = 14;
const NUM_REGISTERS: usize = 1 << PRECISION; // 16384

/// Alpha constant for bias correction at m=16384.
const ALPHA: f64 = 0.7213 / (1.0 + 1.079 / 16384.0);

/// A HyperLogLog sketch with p=14.
#[derive(Clone)]
pub struct HllSketch {
    registers: Vec<u8>,
}

impl HllSketch {
    /// Create an empty sketch.
    pub fn new() -> Self {
        Self {
            registers: vec![0u8; NUM_REGISTERS],
        }
    }

    /// Add an item to the sketch.
    pub fn add(&mut self, item: &str) {
        let hash = hash_to_u64(item);
        let index = (hash >> (64 - PRECISION)) as usize;
        let remaining = (hash << PRECISION) | (1 << (PRECISION - 1)); // ensure non-zero
        let rho = remaining.leading_zeros() as u8 + 1;
        if rho > self.registers[index] {
            self.registers[index] = rho;
        }
    }

    /// Create a sketch from an iterator of string items.
    pub fn from_items<'a>(items: impl Iterator<Item = &'a str>) -> Self {
        let mut sketch = Self::new();
        for item in items {
            sketch.add(item);
        }
        sketch
    }

    /// Merge another sketch into this one (union).
    pub fn merge(&mut self, other: &HllSketch) {
        for i in 0..NUM_REGISTERS {
            if other.registers[i] > self.registers[i] {
                self.registers[i] = other.registers[i];
            }
        }
    }

    /// Merge multiple sketches into a new sketch.
    pub fn merge_all(sketches: &[HllSketch]) -> Self {
        let mut result = Self::new();
        for sketch in sketches {
            result.merge(sketch);
        }
        result
    }

    /// Estimate the cardinality (number of distinct elements).
    pub fn cardinality(&self) -> u64 {
        let m = NUM_REGISTERS as f64;

        // Harmonic mean of 2^(-register[i])
        let sum: f64 = self
            .registers
            .iter()
            .map(|&r| 2.0_f64.powi(-(r as i32)))
            .sum();

        let raw_estimate = ALPHA * m * m / sum;

        // Small range correction (linear counting)
        if raw_estimate <= 2.5 * m {
            let zeros = self.registers.iter().filter(|&&r| r == 0).count() as f64;
            if zeros > 0.0 {
                return (m * (m / zeros).ln()) as u64;
            }
        }

        // Large range correction (not needed for typical analytics workloads < 2^32)
        raw_estimate as u64
    }

    /// Serialize to bytes (raw register array).
    pub fn to_bytes(&self) -> Vec<u8> {
        self.registers.clone()
    }

    /// Deserialize from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != NUM_REGISTERS {
            return None;
        }
        Some(Self {
            registers: bytes.to_vec(),
        })
    }

    /// Serialize to base64 string for JSON transport.
    pub fn to_base64(&self) -> String {
        B64.encode(&self.registers)
    }

    /// Deserialize from base64 string.
    pub fn from_base64(s: &str) -> Option<Self> {
        let bytes = B64.decode(s).ok()?;
        Self::from_bytes(&bytes)
    }

    /// Check if the sketch is empty (no items added).
    pub fn is_empty(&self) -> bool {
        self.registers.iter().all(|&r| r == 0)
    }
}

/// Hash a string to u64 using SHA-256 (truncated).
fn hash_to_u64(item: &str) -> u64 {
    let hash = Sha256::digest(item.as_bytes());
    u64::from_be_bytes(hash[0..8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_sketch() {
        let sketch = HllSketch::new();
        assert_eq!(sketch.cardinality(), 0);
        assert!(sketch.is_empty());
    }

    #[test]
    fn test_single_item() {
        let mut sketch = HllSketch::new();
        sketch.add("hello");
        assert!(sketch.cardinality() >= 1);
        assert!(!sketch.is_empty());
    }

    #[test]
    fn test_accuracy_10k() {
        let sketch = HllSketch::from_items((0..10000).map(|i| {
            // leak the string so we get &str — only in tests
            let s: &str = Box::leak(format!("user_{}", i).into_boxed_str());
            s
        }));
        let estimate = sketch.cardinality();
        // p=14 should be within ~2% of 10000
        let error = (estimate as f64 - 10000.0).abs() / 10000.0;
        assert!(
            error < 0.03,
            "estimate {} has {}% error (expected <3%)",
            estimate,
            error * 100.0
        );
    }

    #[test]
    fn test_merge_disjoint() {
        let s1 = HllSketch::from_items((0..5000).map(|i| {
            let s: &str = Box::leak(format!("a_{}", i).into_boxed_str());
            s
        }));
        let s2 = HllSketch::from_items((5000..10000).map(|i| {
            let s: &str = Box::leak(format!("b_{}", i).into_boxed_str());
            s
        }));
        let merged = HllSketch::merge_all(&[s1, s2]);
        let estimate = merged.cardinality();
        let error = (estimate as f64 - 10000.0).abs() / 10000.0;
        assert!(
            error < 0.03,
            "merged disjoint estimate {} has {}% error",
            estimate,
            error * 100.0
        );
    }

    #[test]
    fn test_merge_identical() {
        let items: Vec<String> = (0..5000).map(|i| format!("user_{}", i)).collect();
        let s1 = HllSketch::from_items(items.iter().map(|s| s.as_str()));
        let s2 = HllSketch::from_items(items.iter().map(|s| s.as_str()));
        let merged = HllSketch::merge_all(&[s1, s2]);
        let estimate = merged.cardinality();
        let error = (estimate as f64 - 5000.0).abs() / 5000.0;
        assert!(
            error < 0.03,
            "merged identical estimate {} has {}% error",
            estimate,
            error * 100.0
        );
    }

    #[test]
    fn test_merge_50_percent_overlap() {
        let s1 = HllSketch::from_items((0..6000).map(|i| {
            let s: &str = Box::leak(format!("user_{}", i).into_boxed_str());
            s
        }));
        let s2 = HllSketch::from_items((3000..9000).map(|i| {
            let s: &str = Box::leak(format!("user_{}", i).into_boxed_str());
            s
        }));
        let merged = HllSketch::merge_all(&[s1, s2]);
        let estimate = merged.cardinality();
        // True unique count = 9000
        let error = (estimate as f64 - 9000.0).abs() / 9000.0;
        assert!(
            error < 0.03,
            "merged overlap estimate {} has {}% error (expected ~9000)",
            estimate,
            error * 100.0
        );
    }

    #[test]
    fn test_serialize_roundtrip() {
        let items: Vec<String> = (0..1000).map(|i| format!("item_{}", i)).collect();
        let sketch = HllSketch::from_items(items.iter().map(|s| s.as_str()));
        let original_card = sketch.cardinality();

        // Bytes roundtrip
        let bytes = sketch.to_bytes();
        assert_eq!(bytes.len(), 16384);
        let restored = HllSketch::from_bytes(&bytes).unwrap();
        assert_eq!(restored.cardinality(), original_card);

        // Base64 roundtrip
        let b64 = sketch.to_base64();
        let restored2 = HllSketch::from_base64(&b64).unwrap();
        assert_eq!(restored2.cardinality(), original_card);
    }
}
