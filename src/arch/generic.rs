#![allow(unused)]
use crate::{Bits, EncodedBits};
use rayon::prelude::*;
use std::{cmp::min, mem::swap, thread::JoinHandle};

pub fn distances<'a>(
    query: &'a EncodedBits,
    db: &'a [EncodedBits],
) -> impl Iterator<Item = [u16; 31]> + 'a {
    const BATCH: usize = 10_000;

    // Prepare 31 rotations of query in advance
    let rotations: Box<[_]> = (-15..=15).map(|r| query.rotated(r)).collect();

    // Iterate over a batch of database entries
    db.chunks(BATCH).flat_map(move |chunk| {
        let mut results = [[0_u16; 31]; BATCH];

        // Parallel computation over batch
        results
            .par_iter_mut()
            .zip(chunk.par_iter())
            .for_each(|(result, entry)| {
                // Compute dot product for each rotation
                for (d, rotation) in result.iter_mut().zip(rotations.iter()) {
                    *d = rotation.dot(entry);
                }
            });

        // Sequentially output results
        results.into_iter().take(chunk.len())
    })
}

pub fn denominators<'a>(query: &'a Bits, db: &'a [Bits]) -> impl Iterator<Item = [u16; 31]> + 'a {
    const BATCH: usize = 10_000;

    // Prepare 31 rotations of query in advance
    let rotations: Box<[_]> = (-15..=15).map(|r| query.rotated(r)).collect();

    // Iterate over a batch of database entries
    db.chunks(BATCH).flat_map(move |chunk| {
        // Parallel computation over batch
        let results = chunk
            .par_iter()
            .map(|(entry)| {
                let mut result = [0_u16; 31];
                // Compute dot product for each rotation
                for (d, rotation) in result.iter_mut().zip(rotations.iter()) {
                    *d = rotation.dot(entry);
                }
                result
            })
            .collect::<Vec<_>>();

        // Sequentially output results
        results.into_iter().take(chunk.len())
    })
}

#[cfg(feature = "bench")]
pub mod benches {
    use super::*;
    use core::hint::black_box;
    use criterion::Criterion;
    use rand::{thread_rng, Rng};

    pub fn group(c: &mut Criterion) {
        let mut rng = thread_rng();
        let mut g = c.benchmark_group("generic");

        g.bench_function("distances 31x1000", |bench| {
            let a: EncodedBits = rng.gen();
            let b: Box<[EncodedBits]> = (0..1000).map(|_| rng.gen()).collect();
            bench.iter(|| black_box(distances(black_box(&a), black_box(&b))).for_each(|_| {}))
        });

        g.bench_function("denominators 31x1000", |bench| {
            let a: Bits = rng.gen();
            let b: Box<[Bits]> = (0..1000).map(|_| rng.gen()).collect();
            bench.iter(|| black_box(denominators(black_box(&a), black_box(&b))).for_each(|_| {}))
        });
    }
}
