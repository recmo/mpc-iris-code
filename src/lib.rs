#![feature(array_chunks)]

pub mod preprocess;
pub mod dotprod;

use core::arch::aarch64::{
    uint16x8_t, vaddq_u16, vaddvq_u16, vandq_u16, vceqq_u16, vdupq_n_u16, vld1q_u16, vld1q_u16_x4,
};
use itertools::izip;
use rand::{
    distributions::{Distribution, Standard},
    thread_rng, Rng,
};
use std::{
    arch::aarch64::{vst1q_u16},
    array,
    mem::size_of,
};
use serde::{Serialize, de::Error, Deserialize};
use bytemuck::{Pod, Zeroable, try_cast_slice, try_cast_slice_mut};

const BITS: usize = 4 * 16 * 200 ;
const LIMBS: usize = BITS / 64;

const MASK_BITS: [u16; 8] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80];

// OPT: If the mask is the same 64x200 pattern repeated there may be
// optimizations.

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Bits([u64; LIMBS]);

#[repr(transparent)]
pub struct SecretBits([u16; BITS]);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Deserialize)]
pub struct Template {
    pattern: Bits,
    mask:    Bits,
}

impl Default for Bits {
    fn default() -> Self {
        Self([0; LIMBS])
    }
}

unsafe impl Zeroable for Bits { }

unsafe impl Pod for Bits { }

impl<'de> Deserialize<'de> for Bits {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes: Vec<u8> = hex::deserialize(deserializer)?;
        let limbs = try_cast_slice(bytes.as_slice()).unwrap();
        let limbs = limbs.try_into().unwrap();
        Ok(Bits(limbs))
    }
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

fn rotate_row(a: &mut[u8; 25], amount: i32) {
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

fn rotate_bits(a: &mut Bits, amount: i32) {
    let bytes : &mut [u8] = try_cast_slice_mut(a.0.as_mut_slice()).unwrap();
    for chunk in bytes.chunks_exact_mut(25) {
        rotate_row(chunk.try_into().unwrap(), amount)
    }
}

fn rotate_template(a: &mut Template, amount: i32) {
    rotate_bits(&mut a.mask, amount);
    rotate_bits(&mut a.pattern, amount);
}

pub fn distance_ref(a: &Template, b: &Template) -> f64 {
    let a_orig = a;
    let mut min_d = f64::INFINITY;
    for r in -15..=15 {
        let mut a = a_orig.clone();
        rotate_template(&mut a, r);
        let mut num = 0;
        let mut den = 0;
        for (ap, am, bp, bm) in izip!(a.pattern.0.iter(), a.mask.0.iter(), b.pattern.0.iter(), b.mask.0.iter()) {
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

pub fn inner1(mask: u64, a: u64, b: &[u16; 64]) -> u16 {
    let mut sum: u16 = 0;
    let mut bit = 1;
    for &b in b.iter() {
        if mask & bit != 0 {
            if a & bit != 0 {
                sum = sum.wrapping_sub(b);
            } else {
                sum = sum.wrapping_add(b);
            }
        }
        bit <<= 1;
    }
    sum
}

pub fn inner2(mask: u64, a: u64, shares: &[u16; 64]) -> u16 {
    let neg_bits = mask & a;
    let pos_bits = mask & !a;
    unsafe {
        let mut neg: uint16x8_t = vdupq_n_u16(0);
        let mut pos: uint16x8_t = vdupq_n_u16(0);

        // Load bit selectors
        let bit_pos = vld1q_u16(MASK_BITS.as_ptr());

        // Load first 32 u16s in batches of 4 x 8 u16.
        let b = vld1q_u16_x4(shares.as_ptr());

        let neg_mask = vceqq_u16(vandq_u16(vdupq_n_u16(neg_bits as u16), bit_pos), bit_pos);
        let pos_mask = vceqq_u16(vandq_u16(vdupq_n_u16(pos_bits as u16), bit_pos), bit_pos);
        neg = vaddq_u16(neg, vandq_u16(b.0, neg_mask));
        pos = vaddq_u16(pos, vandq_u16(b.0, pos_mask));

        let neg_mask = vceqq_u16(
            vandq_u16(vdupq_n_u16((neg_bits >> 8) as u16), bit_pos),
            bit_pos,
        );
        let pos_mask = vceqq_u16(
            vandq_u16(vdupq_n_u16((pos_bits >> 8) as u16), bit_pos),
            bit_pos,
        );
        neg = vaddq_u16(neg, vandq_u16(b.1, neg_mask));
        pos = vaddq_u16(pos, vandq_u16(b.1, pos_mask));

        let neg_mask = vceqq_u16(
            vandq_u16(vdupq_n_u16((neg_bits >> 16) as u16), bit_pos),
            bit_pos,
        );
        let pos_mask = vceqq_u16(
            vandq_u16(vdupq_n_u16((pos_bits >> 16) as u16), bit_pos),
            bit_pos,
        );
        neg = vaddq_u16(neg, vandq_u16(b.2, neg_mask));
        pos = vaddq_u16(pos, vandq_u16(b.2, pos_mask));

        let neg_mask = vceqq_u16(
            vandq_u16(vdupq_n_u16((neg_bits >> 24) as u16), bit_pos),
            bit_pos,
        );
        let pos_mask = vceqq_u16(
            vandq_u16(vdupq_n_u16((pos_bits >> 24) as u16), bit_pos),
            bit_pos,
        );
        neg = vaddq_u16(neg, vandq_u16(b.3, neg_mask));
        pos = vaddq_u16(pos, vandq_u16(b.3, pos_mask));

        let b = vld1q_u16_x4(shares[32..].as_ptr());

        let neg_mask = vceqq_u16(
            vandq_u16(vdupq_n_u16((neg_bits >> 32) as u16), bit_pos),
            bit_pos,
        );
        let pos_mask = vceqq_u16(
            vandq_u16(vdupq_n_u16((pos_bits >> 32) as u16), bit_pos),
            bit_pos,
        );
        neg = vaddq_u16(neg, vandq_u16(b.0, neg_mask));
        pos = vaddq_u16(pos, vandq_u16(b.0, pos_mask));

        let neg_mask = vceqq_u16(
            vandq_u16(vdupq_n_u16((neg_bits >> 40) as u16), bit_pos),
            bit_pos,
        );
        let pos_mask = vceqq_u16(
            vandq_u16(vdupq_n_u16((pos_bits >> 40) as u16), bit_pos),
            bit_pos,
        );
        neg = vaddq_u16(neg, vandq_u16(b.1, neg_mask));
        pos = vaddq_u16(pos, vandq_u16(b.1, pos_mask));

        let neg_mask = vceqq_u16(
            vandq_u16(vdupq_n_u16((neg_bits >> 48) as u16), bit_pos),
            bit_pos,
        );
        let pos_mask = vceqq_u16(
            vandq_u16(vdupq_n_u16((pos_bits >> 48) as u16), bit_pos),
            bit_pos,
        );
        neg = vaddq_u16(neg, vandq_u16(b.2, neg_mask));
        pos = vaddq_u16(pos, vandq_u16(b.2, pos_mask));

        let neg_mask = vceqq_u16(
            vandq_u16(vdupq_n_u16((neg_bits >> 56) as u16), bit_pos),
            bit_pos,
        );
        let pos_mask = vceqq_u16(
            vandq_u16(vdupq_n_u16((pos_bits >> 56) as u16), bit_pos),
            bit_pos,
        );
        neg = vaddq_u16(neg, vandq_u16(b.3, neg_mask));
        pos = vaddq_u16(pos, vandq_u16(b.3, pos_mask));

        let neg = vaddvq_u16(neg);
        let pos = vaddvq_u16(pos);
        pos.wrapping_sub(neg)
    }
}

pub fn create_lookup() -> [[u16; 8]; 256] {
    const MASK_BITS: [u16; 8] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80];
    unsafe {
        let bit_pos = vld1q_u16(MASK_BITS.as_ptr());
        array::from_fn(|i| {
            let mut result = [0_u16; 8];
            let mask = vceqq_u16(vandq_u16(vdupq_n_u16(i as u16), bit_pos), bit_pos);
            vst1q_u16(result.as_mut_ptr(), mask);
            result
        })
    }
}

pub fn inner3(lookup: &[[u16; 8]; 256], mask: u64, a: u64, shares: &[u16; 64]) -> u16 {
    let neg_bits = (mask & a).to_be_bytes();
    let pos_bits = (mask & !a).to_be_bytes();
    unsafe {
        let lookup = lookup.as_ptr();

        let mut neg: uint16x8_t = vdupq_n_u16(0);
        let mut pos: uint16x8_t = vdupq_n_u16(0);

        // Load first 32 u16s in batches of 4 x 8 u16.
        let b = vld1q_u16_x4(shares.as_ptr());

        let neg_mask = vld1q_u16(lookup.add(neg_bits[0] as usize) as *const u16);
        let pos_mask = vld1q_u16(lookup.add(pos_bits[0] as usize) as *const u16);
        neg = vaddq_u16(neg, vandq_u16(b.0, neg_mask));
        pos = vaddq_u16(pos, vandq_u16(b.0, pos_mask));

        let neg_mask = vld1q_u16(lookup.add(neg_bits[1] as usize) as *const u16);
        let pos_mask = vld1q_u16(lookup.add(pos_bits[1] as usize) as *const u16);
        neg = vaddq_u16(neg, vandq_u16(b.1, neg_mask));
        pos = vaddq_u16(pos, vandq_u16(b.1, pos_mask));

        let neg_mask = vld1q_u16(lookup.add(neg_bits[2] as usize) as *const u16);
        let pos_mask = vld1q_u16(lookup.add(pos_bits[2] as usize) as *const u16);
        neg = vaddq_u16(neg, vandq_u16(b.2, neg_mask));
        pos = vaddq_u16(pos, vandq_u16(b.2, pos_mask));

        let neg_mask = vld1q_u16(lookup.add(neg_bits[3] as usize) as *const u16);
        let pos_mask = vld1q_u16(lookup.add(pos_bits[3] as usize) as *const u16);
        neg = vaddq_u16(neg, vandq_u16(b.3, neg_mask));
        pos = vaddq_u16(pos, vandq_u16(b.3, pos_mask));

        let b = vld1q_u16_x4(shares[32..].as_ptr());

        let neg_mask = vld1q_u16(lookup.add(neg_bits[4] as usize) as *const u16);
        let pos_mask = vld1q_u16(lookup.add(pos_bits[4] as usize) as *const u16);
        neg = vaddq_u16(neg, vandq_u16(b.0, neg_mask));
        pos = vaddq_u16(pos, vandq_u16(b.0, pos_mask));

        let neg_mask = vld1q_u16(lookup.add(neg_bits[5] as usize) as *const u16);
        let pos_mask = vld1q_u16(lookup.add(pos_bits[5] as usize) as *const u16);
        neg = vaddq_u16(neg, vandq_u16(b.1, neg_mask));
        pos = vaddq_u16(pos, vandq_u16(b.1, pos_mask));

        let neg_mask = vld1q_u16(lookup.add(neg_bits[6] as usize) as *const u16);
        let pos_mask = vld1q_u16(lookup.add(pos_bits[6] as usize) as *const u16);
        neg = vaddq_u16(neg, vandq_u16(b.2, neg_mask));
        pos = vaddq_u16(pos, vandq_u16(b.2, pos_mask));

        let neg_mask = vld1q_u16(lookup.add(neg_bits[7] as usize) as *const u16);
        let pos_mask = vld1q_u16(lookup.add(pos_bits[7] as usize) as *const u16);
        neg = vaddq_u16(neg, vandq_u16(b.3, neg_mask));
        pos = vaddq_u16(pos, vandq_u16(b.3, pos_mask));

        let neg = vaddvq_u16(neg);
        let pos = vaddvq_u16(pos);
        pos.wrapping_sub(neg)
    }
}

pub fn distance1(query: &Template, reference: &SecretTemplate) -> (u16, u16) {
    let mut sum: u16 = 0;
    let mut denominator: u16 = 0;
    for (a, a_mask, b, b_mask) in izip!(
        query.pattern.0.iter(),
        query.mask.0.iter(),
        reference.pattern.0.array_chunks(),
        reference.mask.0.iter(),
    ) {
        let mask = a_mask & b_mask;
        let d = a & mask;
        sum = sum.wrapping_add(d.count_ones() as u16);
        denominator = denominator.wrapping_add(mask.count_ones() as u16);
        sum = sum.wrapping_add(inner1(mask, *a, b));
    }
    (sum, denominator)
}

pub fn distance2(query: &Template, reference: &SecretTemplate) -> (u16, u16) {
    let mut sum: u16 = 0;
    let mut denominator: u16 = 0;
    for (a, a_mask, b, b_mask) in izip!(
        query.pattern.0.iter(),
        query.mask.0.iter(),
        reference.pattern.0.array_chunks(),
        reference.mask.0.iter(),
    ) {
        let mask = a_mask & b_mask;
        let d = a & mask;
        sum = sum.wrapping_add(d.count_ones() as u16);
        denominator = denominator.wrapping_add(mask.count_ones() as u16);
        sum = sum.wrapping_add(inner2(mask, *a, b));
    }
    (sum, denominator)
}

pub fn distance3(
    lookup: &[[u16; 8]; 256],
    query: &Template,
    reference: &SecretTemplate,
) -> (u16, u16) {
    // TODO: Preprocess query to optimize for inner loop.

    let mut sum: u16 = 0;
    let mut denominator: u16 = 0;
    for (a, a_mask, b, b_mask) in izip!(
        query.pattern.0.iter(),
        query.mask.0.iter(),
        reference.pattern.0.array_chunks(),
        reference.mask.0.iter(),
    ) {
        let mask = a_mask & b_mask;
        let d = a & mask;
        sum = sum.wrapping_add(d.count_ones() as u16);
        denominator = denominator.wrapping_add(mask.count_ones() as u16);
        let inner = inner3(lookup, mask, *a, b);
        sum = sum.wrapping_add(inner);
    }
    (sum, denominator)
}

pub fn main() {
    eprintln!("Size of Template: {}", size_of::<Template>());
    eprintln!("Size of SecretTemplate: {}", size_of::<SecretTemplate>());

    let mut rng = thread_rng();
    let query: Template = rng.gen();
    let reference: SecretTemplate = rng.gen();
    let (s, d) = distance1(&query, &reference);
    eprintln!("Distance {s} / {d} = {}", (s as f64) / (d as f64));

    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use float_eq::assert_float_eq;

    #[test]
    fn limbs_exact() {
        assert_eq!(LIMBS * 64, BITS);
    }

    #[test]
    #[ignore] // Requires test data
    fn test_distance_ref() {
        #[derive(Deserialize)]
        struct Distance {
            left: usize,
            right: usize,
            distance: f64
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

        c.bench_function("Bench distance1 (31x1000)", |b| {
            b.iter(|| {
                for reference in &*db {
                    for query in &*queries {
                        let (s, d) = black_box(distance1(query, reference));
                    }
                }
            })
        });
        c.bench_function("Bench distance2 (31x1000)", |b| {
            b.iter(|| {
                for reference in &*db {
                    for query in &*queries {
                        let (s, d) = black_box(distance2(query, reference));
                    }
                }
            })
        });
        let lookup = create_lookup();
        c.bench_function("Bench distance3 (31x1000)", |b| {
            b.iter(|| {
                for reference in &*db {
                    for query in &*queries {
                        let (s, d) = black_box(distance3(&lookup, query, reference));
                    }
                }
            })
        });

        preprocess::benches::group(c);
        dotprod::benches::group(c);
    }
}
