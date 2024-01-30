mod generic; // Generic implementation
mod sve; // SVE2

pub use generic::{dot_bool, dot_u16};

#[cfg(feature = "bench")]
pub mod benches {
    use super::*;
    use crate::{bits::LIMBS, Bits, EncodedBits, BITS};
    use criterion::{BenchmarkId, Criterion, Throughput};
    use rand::{thread_rng, Rng};
    use std::hint::black_box;

    pub fn group(c: &mut Criterion) {
        generic::benches::group(c);
    }

    pub fn bench_dot_bool(
        criterion: &mut Criterion,
        name: &'static str,
        f: impl Fn(&[u64; LIMBS], &[u64; LIMBS]) -> u16,
    ) {
        let mut rng = thread_rng();
        let mut group = criterion.benchmark_group(name);
        for (a, b) in [(1, 1), (1, 1000), (31, 1000), (1, 100_000)] {
            group.throughput(Throughput::Elements(a * b));
            group.sample_size(if a * b > 10_000 { 10 } else { 100 });
            let a_vals = (0..a).map(|_| rng.gen::<Bits>().0).collect::<Box<[_]>>();
            let b_vals = (0..b).map(|_| rng.gen::<Bits>().0).collect::<Box<[_]>>();
            group.bench_function(BenchmarkId::from_parameter(a * b), |bencher| {
                bencher.iter(|| {
                    for b in black_box(&b_vals).iter() {
                        for a in black_box(&a_vals).iter() {
                            black_box(f(a, b));
                        }
                    }
                });
            });
        }
    }

    pub fn bench_dot_u16(
        criterion: &mut Criterion,
        name: &'static str,
        f: impl Fn(&[u16; BITS], &[u16; BITS]) -> u16,
    ) {
        let mut rng = thread_rng();
        let mut group = criterion.benchmark_group(name);
        for (a, b) in [(1, 1), (1, 1000), (31, 1000), (1, 100_000)] {
            group.throughput(Throughput::Elements(a * b));
            group.sample_size(if a * b > 10_000 { 10 } else { 100 });
            let a_vals = (0..a)
                .map(|_| rng.gen::<EncodedBits>().0)
                .collect::<Box<[_]>>();
            let b_vals = (0..b)
                .map(|_| rng.gen::<EncodedBits>().0)
                .collect::<Box<[_]>>();
            group.bench_function(BenchmarkId::from_parameter(a * b), |bencher| {
                bencher.iter(|| {
                    for b in black_box(&b_vals).iter() {
                        for a in black_box(&a_vals).iter() {
                            black_box(f(a, b));
                        }
                    }
                });
            });
        }
    }
}
