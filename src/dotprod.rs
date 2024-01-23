const N: usize = 16 * 200 * 2 * 2;

pub fn dot_ref(a: &[u16; N], b: &[u16; N]) -> u16 {
    let mut sum = 0_u16;
    for (a, b) in a.iter().zip(b.iter()) {
        sum = sum.wrapping_add(a.wrapping_mul(*b));
    }
    sum
}

#[cfg(feature = "bench")]
pub mod benches {
    use super::*;
    use core::hint::black_box;
    use criterion::Criterion;
    use rand::{thread_rng, Rng};
    use std::{mem::size_of, array};

    pub fn group(c: &mut Criterion) {
        let mut rng = thread_rng();

        // Generate 31 query template
        let queries: Box<[[u16;N]]> = (0..31).map(|_| array::from_fn(|_| rng.gen())).collect();

        // Generate 1000 reference templates (database)
        let db: Box<[[u16;N]]> = (0..1000).map(|_| array::from_fn(|_| rng.gen())).collect();

        c.bench_function("Bench dotref (1x31x1000)", |b| {
            b.iter(|| {
                for reference in &*db {
                    for query in &*queries {
                        black_box(dot_ref(query, reference));
                    }
                }
            })
        });
    }
}
