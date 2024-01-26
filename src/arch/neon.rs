#![cfg(target_feature = "neon")]
#![allow(unused)]

// Rust + LLVM already generates good NEON code for the generic implementation.

#[cfg(feature = "bench")]
pub mod benches {
    use super::*;
    use core::hint::black_box;
    use criterion::Criterion;
    use rand::{thread_rng, Rng};

    pub fn group(c: &mut Criterion) {
        let mut g = c.benchmark_group("neon");
        let mut rng = thread_rng();
    }
}
