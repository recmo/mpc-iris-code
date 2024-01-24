#![feature(array_chunks)]

mod bits;
mod secret_bits;
mod template;

pub use crate::{bits::Bits, secret_bits::SecretBits, template::Template};

pub const BITS: usize = 4 * 16 * 200;

pub fn preprocess(template: &Template) -> () {
    todo!()
}

pub fn distances(preprocessed: &(), pattern: &SecretBits) -> [u16; 31] {
    todo!()
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
