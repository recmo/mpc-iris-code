#![feature(array_chunks)]

pub mod preprocess;

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

const BITS: usize = 16 * 200 * 4;
const LIMBS: usize = BITS / 64;

const MASK_BITS: [u16; 8] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80];

// OPT: If the mask is the same 64x200 pattern repeated there may be
// optimizations.

#[repr(transparent)]
pub struct Bits([u64; LIMBS]);

#[repr(transparent)]
pub struct SecretBits([u16; BITS]);

pub struct Template {
    pattern: Bits,
    mask:    Bits,
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

    #[test]
    fn limbs_exact() {
        assert_eq!(LIMBS * 64, BITS);
    }

    #[test]
    fn distance2_ref() {}
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
    }
}
