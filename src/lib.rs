#![feature(array_chunks)]

pub mod dotprod;
pub mod preprocess;

use bytemuck::{cast_slice, try_cast_slice, try_cast_slice_mut, Pod, Zeroable};
use core::arch::aarch64::{
    uint16x8_t, vaddq_u16, vaddvq_u16, vandq_u16, vceqq_u16, vdupq_n_u16, vld1q_u16, vld1q_u16_x4,
};
use itertools::izip;
use rand::{
    distributions::{Distribution, Standard},
    thread_rng, Rng,
};
use serde::{de::Error as _, ser::Error as _, Deserialize, Serialize};
use std::{arch::aarch64::vst1q_u16, array, fmt::Debug, mem::size_of, process::Output};

pub const BITS: usize = 4 * 16 * 200;
const LIMBS: usize = BITS / 64;

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Bits(pub [u64; LIMBS]);

#[repr(transparent)]
pub struct SecretBits(pub [u16; BITS]);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize)]
pub struct Template {
    pub pattern: Bits,
    pub mask:    Bits,
}

unsafe impl Zeroable for Bits {}

unsafe impl Pod for Bits {}

impl Bits {
    pub fn as_bytes(&self) -> &[u8] {
        cast_slice(self.0.as_slice())
    }
}

impl Default for Bits {
    fn default() -> Self {
        Self([0; LIMBS])
    }
}

impl Debug for Bits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for limb in self.0 {
            write!(f, "{limb:016x}")?;
        }
        Ok(())
    }
}

impl Serialize for Bits {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let bytes: &[u8] = try_cast_slice(self.0.as_slice()).map_err(S::Error::custom)?;
        hex::serialize(&bytes, serializer)
    }
}

impl<'de> Deserialize<'de> for Bits {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes: Vec<u8> = hex::deserialize(deserializer)?;
        let limbs = try_cast_slice(bytes.as_slice()).map_err(D::Error::custom)?;
        let limbs = limbs.try_into().map_err(D::Error::custom)?;
        Ok(Bits(limbs))
    }
}

impl SecretBits {
    pub fn from_bits(bits: &Bits, count: usize) -> Box<[SecretBits]> {
        // Write SecretBit to share files
        for limb in &t.pattern.0 {
            for b in 0..64 {
                let bit = (1 << b) & limb != 0;

                // Compute shares
                let mut sum: u16 = 0;
                for output in &mut outputs[1..] {
                    let element: u16 = rng.gen();
                    sum += element;
                    output.write_all(&element.to_le_bytes())?;
                }
                sum = sum.wrapping_neg().wrapping_add(if bit { 1 } else { 0 });
                outputs[0].write_all(&sum.to_le_bytes())?;
            }
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        cast_slice(self.0.as_slice())
    }

    fn random()
}

pub struct SecretTemplate {
    pattern: SecretBits,
    mask:    Bits,
}

impl Distribution<Bits> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Bits {
        Bits(array::from_fn(|_| rng.gen()))
    }
}

impl Distribution<SecretBits> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> SecretBits {
        SecretBits(array::from_fn(|_| rng.gen()))
    }
}

impl Distribution<Template> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Template {
        Template {
            pattern: rng.gen(),
            mask:    rng.gen(),
        }
    }
}

impl Distribution<SecretTemplate> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> SecretTemplate {
        SecretTemplate {
            pattern: rng.gen(),
            mask:    rng.gen(),
        }
    }
}

fn rotate_row(a: &mut [u8; 25], amount: i32) {
    assert!(amount >= -15);
    assert!(amount <= 15);
    if amount < -8 {
        rotate_row(a, -8);
        rotate_row(a, amount + 8);
    } else if amount == -8 {
        let first = a[0];
        a.copy_within(1.., 0);
        a[24] = first;
    } else if amount < 0 {
        let l = -amount;
        let r = 8 - l;
        let mut carry = a[0] >> r;
        for b in a.iter_mut().rev() {
            let old = *b;
            *b = (old << l) | carry;
            carry = old >> r;
        }
    } else if amount == 0 {
        return;
    } else if amount < 8 {
        let r = amount;
        let l = 8 - r;
        let mut carry = a[24] << l;
        for b in a.iter_mut() {
            let old = *b;
            *b = (old >> r) | carry;
            carry = old << l;
        }
    } else if amount == 8 {
        let last = a[24];
        a.copy_within(0..24, 1);
        a[0] = last;
    } else {
        rotate_row(a, 8);
        rotate_row(a, amount - 8);
    }
}

impl Bits {
    pub fn rotate(&mut self, amount: i32) {
        let bytes: &mut [u8] = try_cast_slice_mut(self.0.as_mut_slice()).unwrap();
        for chunk in bytes.chunks_exact_mut(25) {
            rotate_row(chunk.try_into().unwrap(), amount)
        }
    }
}

impl Template {
    pub fn rotate(&mut self, amount: i32) {
        self.mask.rotate(amount);
        self.pattern.rotate(amount);
    }
}

pub fn distance_ref(a: &Template, b: &Template) -> f64 {
    let a_orig = a;
    let mut min_d = f64::INFINITY;
    for r in -15..=15 {
        let mut a = a_orig.clone();
        a.rotate(r);
        let mut num = 0;
        let mut den = 0;
        for (ap, am, bp, bm) in izip!(
            a.pattern.0.iter(),
            a.mask.0.iter(),
            b.pattern.0.iter(),
            b.mask.0.iter()
        ) {
            let m = am & bm;
            let p = (ap ^ bp) & m;
            num += p.count_ones();
            den += m.count_ones();
        }
        let d = (num as f64) / (den as f64);
        min_d = f64::min(min_d, d);
    }
    min_d
}

#[cfg(test)]
mod tests {
    use super::*;
    use float_eq::assert_float_eq;
    use std::fs::File;

    #[test]
    fn limbs_exact() {
        assert_eq!(LIMBS * 64, BITS);
    }

    #[test]
    #[ignore] // Requires test data
    fn test_distance_ref() {
        #[derive(Deserialize)]
        struct Distance {
            left:     usize,
            right:    usize,
            distance: f64,
        };

        // Read templates
        let file = File::open("data/templates.json").unwrap();
        let data: Vec<Template> = serde_json::from_reader(file).unwrap();
        assert_eq!(data.len(), 2092);

        // Read distances
        let file = File::open("data/distances.json").unwrap();
        let distances: Vec<Distance> = serde_json::from_reader(file).unwrap();
        assert_eq!(distances.len(), 1000);

        // Check distances to within 10 ulp
        for d in distances {
            let left = data[d.left];
            let right = data[d.right];
            let expected = d.distance;
            let actual = distance_ref(&left, &right);
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

        preprocess::benches::group(c);
        dotprod::benches::group(c);
    }
}
