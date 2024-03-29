pub mod arch;
mod bits;
mod encoded_bits;
mod template;

pub use crate::{bits::Bits, encoded_bits::EncodedBits, template::Template};
use core::slice;
use rayon::prelude::*;

pub const COLS: usize = 200;
pub const ROWS: usize = 4 * 16;
pub const BITS: usize = ROWS * COLS;

/// Generate a [`EncodedBits`] such that values are $\{-1,0,1\}$, representing
/// unset, masked and set.
pub fn encode(template: &Template) -> EncodedBits {
    // Make sure masked-out pattern bits are zero;
    let pattern = &template.pattern & &template.mask;

    // Convert to u16s
    let pattern = EncodedBits::from(&pattern);
    let mask = EncodedBits::from(&template.mask);

    // Preprocessed is (mask - 2 * pattern)
    mask - &pattern - &pattern
}

pub struct DistanceEngine {
    rotations: Box<[EncodedBits; 31]>,
}

impl DistanceEngine {
    pub fn new(query: &EncodedBits) -> Self {
        let rotations = (-15..=15)
            .map(|r| query.rotated(r))
            .collect::<Box<[EncodedBits]>>()
            .try_into()
            .unwrap();
        Self { rotations }
    }

    pub fn batch_process(&self, out: &mut [[u16; 31]], db: &[EncodedBits]) {
        assert_eq!(out.len(), db.len());
        out.par_iter_mut()
            .zip(db.par_iter())
            .for_each(|(result, entry)| {
                // Compute dot product for each rotation
                for (d, rotation) in result.iter_mut().zip(self.rotations.iter()) {
                    *d = rotation.dot(entry);
                }
            });
    }
}

pub struct MasksEngine {
    rotations: Box<[Bits; 31]>,
}

impl MasksEngine {
    pub fn new(query: &Bits) -> Self {
        let rotations = (-15..=15)
            .map(|r| query.rotated(r))
            .collect::<Box<[Bits]>>()
            .try_into()
            .unwrap();
        Self { rotations }
    }

    pub fn batch_process(&self, out: &mut [[u16; 31]], db: &[Bits]) {
        assert_eq!(out.len(), db.len());
        out.par_iter_mut()
            .zip(db.par_iter())
            .for_each(|(result, entry)| {
                // Compute dot product for each rotation
                for (d, rotation) in result.iter_mut().zip(self.rotations.iter()) {
                    *d = rotation.dot(entry);
                }
            });
    }
}

pub fn distances(query: &EncodedBits, entry: &EncodedBits) -> [u16; 31] {
    let engine = DistanceEngine::new(query);
    let mut result = [0_u16; 31];
    engine.batch_process(slice::from_mut(&mut result), slice::from_ref(entry));
    result
}

pub fn denominators(query: &Bits, entry: &Bits) -> [u16; 31] {
    let engine = MasksEngine::new(query);
    let mut result = [0_u16; 31];
    engine.batch_process(slice::from_mut(&mut result), slice::from_ref(entry));
    result
}

/// Decode a distances. Takes the minimum over the rotations
pub fn decode_distance(distances: &[u16; 31], denominators: &[u16; 31]) -> f64 {
    // TODO: Detect errors.
    // (d - n) must be an even number in range

    distances
        .iter()
        .zip(denominators.iter())
        .map(|(&n, &d)| (d.wrapping_sub(n) / 2, d))
        .map(|(n, d)| (n as f64) / (d as f64))
        .fold(f64::INFINITY, f64::min)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{encode, template::tests::test_data};
    use float_eq::assert_float_eq;
    use proptest::bits::u16;
    use rand::{thread_rng, Rng};

    #[test]
    fn test_preprocess() {
        let mut rng = thread_rng();
        for _ in 0..100 {
            let entry = rng.gen();
            let encrypted = encode(&entry);
            for (i, v) in encrypted.0.iter().enumerate() {
                match *v {
                    u16::MAX => assert!(entry.mask[i] && entry.pattern[i]),
                    0 => assert!(!entry.mask[i]),
                    1 => assert!(entry.mask[i] && !entry.pattern[i]),
                    _ => panic!(),
                }
            }
        }
    }

    #[test]
    fn test_dotproduct() {
        let mut rng = thread_rng();
        for _ in 0..100 {
            let a = rng.gen();
            let b = rng.gen();
            let pre_a = encode(&a);
            let pre_b = encode(&b);

            let mut equal = 0;
            let mut uneq = 0;
            let mut denominator = 0;
            for i in 0..BITS {
                if a.mask[i] && b.mask[i] {
                    denominator += 1;
                    if a.pattern[i] == b.pattern[i] {
                        equal += 1;
                    } else {
                        uneq += 1;
                    }
                }
            }

            let sum = (pre_a * &pre_b).sum() as i16;
            assert_eq!(equal - uneq, sum);
            assert_eq!(equal + uneq, denominator);
            assert_eq!((denominator - sum) % 2, 0);
            assert_eq!(uneq, (denominator - sum) / 2);
        }
    }

    #[test]
    fn test_encrypted_distances() {
        let (data, dist) = test_data();
        for d in dist {
            let expected = d.distance;
            let query = &data[d.left];
            let entry = &data[d.right];

            let encrypted = encode(&entry);
            for (i, v) in encrypted.0.iter().enumerate() {
                match *v {
                    u16::MAX => assert!(entry.mask[i] && entry.pattern[i]),
                    0 => assert!(!entry.mask[i]),
                    1 => assert!(entry.mask[i] && !entry.pattern[i]),
                    _ => panic!(),
                }
            }

            // Encode entry
            let preprocessed = encode(&query);
            let distances = distances(&preprocessed, &encrypted);
            let denominators = denominators(&query.mask, &entry.mask);

            // Measure encoded distance
            let actual = decode_distance(&distances, &denominators);

            assert_float_eq!(actual, expected, ulps <= 1);
        }
    }
}

#[cfg(feature = "bench")]
pub mod benches {
    use super::*;
    use criterion::Criterion;

    pub fn group(c: &mut Criterion) {
        arch::benches::group(c);
    }
}
