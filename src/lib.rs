mod bits;
mod secret_bits;
mod template;

pub use crate::{bits::Bits, secret_bits::SecretBits, template::Template};

pub const COLS: usize = 200;
pub const ROWS: usize = 4 * 16;
pub const BITS: usize = ROWS * COLS;

/// Generate a [`SecretBits`] such that values are $\{-1,0,1\}$, representing
/// unset, masked and set.
pub fn preprocess(template: &Template) -> SecretBits {
    // Make sure masked-out pattern bits are zero;
    let pattern = &template.pattern & &template.mask;

    // Convert to u16s
    let pattern = SecretBits::from(&pattern);
    let mask = SecretBits::from(&template.mask);

    // Preprocessed is (mask - 2 * pattern)
    mask - &pattern - &pattern
}

pub fn distances(preprocessed: &SecretBits, pattern: &SecretBits) -> [u16; 31] {
    let mut result = [0_u16; 31];
    for (d, r) in result.iter_mut().zip(-15..=15) {
        *d = (preprocessed.rotated(r) * pattern).sum();
    }
    result
}

/// Compute the 31 rotated mask popcounts.
pub fn denominators(a: &Bits, b: &Bits) -> [u16; 31] {
    let mut result = [0_u16; 31];
    for (d, r) in result.iter_mut().zip(-15..=15) {
        *d = (a.rotated(r) & b).count_ones();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{denominators, preprocess, template::tests::test_data};
    use float_eq::assert_float_eq;
    use proptest::bits::u16;
    use rand::{thread_rng, Rng};

    #[test]
    fn test_preprocess() {
        let mut rng = thread_rng();
        for _ in 0..100 {
            let entry = rng.gen();
            let encrypted = preprocess(&entry);
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
            let pre_a = preprocess(&a);
            let pre_b = preprocess(&b);

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

            let encrypted = preprocess(&entry);
            for (i, v) in encrypted.0.iter().enumerate() {
                match *v {
                    u16::MAX => assert!(entry.mask[i] && entry.pattern[i]),
                    0 => assert!(!entry.mask[i]),
                    1 => assert!(entry.mask[i] && !entry.pattern[i]),
                    _ => panic!(),
                }
            }

            // Encode entry
            let preprocessed = preprocess(&query);
            let distances = distances(&preprocessed, &encrypted);
            let denominators = denominators(&query.mask, &entry.mask);

            // Measure encoded distance
            let actual = distances
                .iter()
                .zip(denominators.iter())
                .map(|(&n, &d)| (d.wrapping_sub(n) / 2, d))
                .map(|(n, d)| (n as f64) / (d as f64))
                .fold(f64::INFINITY, f64::min);

            assert_float_eq!(actual, expected, ulps <= 1);
        }
    }
}

#[cfg(feature = "bench")]
pub mod benches {
    use super::*;
    use core::hint::black_box;
    use criterion::Criterion;

    pub fn group(c: &mut Criterion) {
        let mut rng = thread_rng();

        // Generate 31 query templates (rotations)
        let queries: Box<[Template]> = (0..31).map(|_| rng.gen()).collect();

        // Generate 1000 reference templates (database)
        let db: Box<[SecretTemplate]> = (0..1000).map(|_| rng.gen()).collect();
    }
}
