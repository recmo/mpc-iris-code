#![cfg(target_feature = "sve2")]
#![allow(unused)]
use crate::{bits::LIMBS, BITS};

pub fn dot_bool(a: &[u64; LIMBS], b: &[u64; LIMBS]) -> u16 {
    a.iter()
        .zip(b.iter())
        .map(|(&a, &b)| (a & b).count_ones() as u16)
        .fold(0_u16, u16::wrapping_add)
}

pub fn dot_u16(a: &[u16; BITS], b: &[u16; BITS]) -> u16 {
    a.iter()
        .zip(b.iter())
        .map(|(&a, &b)| u16::wrapping_mul(a, b))
        .fold(0_u16, u16::wrapping_add)
}

#[cfg(feature = "bench")]
pub mod benches {
    use super::{
        super::benches::{bench_dot_bool, bench_dot_u16},
        *,
    };
    use criterion::Criterion;

    pub fn group(criterion: &mut Criterion) {
        bench_dot_bool(criterion, "sve2/dot_bool", dot_bool);
        bench_dot_u16(criterion, "sve2/dot_u16", dot_u16);
    }
}
