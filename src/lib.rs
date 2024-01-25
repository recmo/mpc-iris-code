mod arch;
mod bits;
mod encoded_bits;
mod template;

pub use crate::{bits::Bits, encoded_bits::EncodedBits, template::Template};

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

/// Compute the 31 rotated mask popcounts.
pub fn denominators(a: &Bits, b: &Bits) -> [u16; 31] {
    let mut result = [0_u16; 31];
    for (d, r) in result.iter_mut().zip(-15..=15) {
        *d = (a.rotated(r) & b).count_ones();
    }
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

/// Compute encoded distances for each rotation, iterating over a database
pub fn distances<'a>(
    query: &'a EncodedBits,
    db: &'a [EncodedBits],
) -> impl Iterator<Item = [u16; 31]> + 'a {
    arch::distances(query, db)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{denominators, encode, template::tests::test_data};
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
            let distances = distances(&preprocessed, &[encrypted]).next().unwrap();
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
