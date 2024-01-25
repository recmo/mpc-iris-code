use crate::EncodedBits;

pub fn distances<'a>(
    query: &'a EncodedBits,
    db: &'a [EncodedBits],
) -> impl Iterator<Item = [u16; 31]> + 'a {
    // Prepare 31 rotations of query
    let rotations: Box<[EncodedBits]> = (-15..=15).map(|r| query.rotated(r)).collect();

    // Iterate over database entries
    db.iter().map(move |entry| {
        let mut result = [0_u16; 31];

        // Compute dot product
        for (d, rotation) in result.iter_mut().zip(rotations.iter()) {
            *d = rotation.dot(&entry);
        }

        result
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
    }
}
