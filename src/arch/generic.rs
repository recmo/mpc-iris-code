#![allow(unused)]
use crate::{bits::LIMBS, BITS};
use ndarray::Array;
use ndarray_rand::{rand_distr::Uniform, RandomExt};

const PRIME: u16 = 0xffef_u16;

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

type Matrix = ndarray::Array2<u16>;
type MatrixRef<'a> = ndarray::ArrayView2<'a, u16>;
type MatrixMut<'a> = ndarray::ArrayViewMut2<'a, u16>;

pub fn dot_ring(c: &mut [u16], q: &[u16], d: &[u16]) {
    // Fetch and check dimensions.
    let n_code = 12_800; // Code size
    debug_assert_eq!(q.len() % n_code, 0);
    debug_assert_eq!(d.len() % n_code, 0);
    let n_batch = q.len() / n_code;
    let n_db = d.len() / n_code;
    debug_assert_eq!(c.len(), n_batch * n_db);

    for i in 0..n_db {
        for j in 0..n_batch {
            let mut accumulator = 0_u16;
            for k in 0..n_code {
                let qjk = q[j * n_code + k];
                let dik = d[i * n_code + k];
                accumulator = accumulator.wrapping_add(qjk.wrapping_mul(dik));
            }
            c[i * n_batch + j] = accumulator;
        }
    }
}

pub fn dot_field(c: &mut [u16], q: &[u16], d: &[u16]) {
    // Fetch and check dimensions.
    let n_code = 12_800; // Code size
    debug_assert_eq!(q.len() % n_code, 0);
    debug_assert_eq!(d.len() % n_code, 0);
    let n_batch = q.len() / n_code;
    let n_db = d.len() / n_code;
    debug_assert_eq!(c.len(), n_batch * n_db);

    for i in 0..n_db {
        for j in 0..n_batch {
            let mut accumulator = 0_u64;
            for k in 0..n_code {
                let qjk = q[j * n_code + k];
                let dik = d[i * n_code + k];
                accumulator =
                    accumulator.wrapping_add((qjk as u32).wrapping_mul(dik as u32) as u64);
            }
            c[i * n_batch + j] = (accumulator % (PRIME as u64)) as u16;
        }
    }
}

#[cfg(feature = "bench")]
pub mod benches {
    use super::{
        super::benches::{bench_dot_bool, bench_dot_u16},
        *,
    };
    use bytemuck::cast_slice_mut;
    use criterion::{black_box, Criterion};
    use rand::{distributions::Standard, Rng, RngCore};
    use std::time::Instant;

    pub fn group(criterion: &mut Criterion) {
        // bench_dot_bool(criterion, "generic/dot_bool", dot_bool);
        // bench_dot_u16(criterion, "generic/dot_u16", dot_u16);

        bench_mul_ring(criterion);
        bench_mul_field(criterion);
    }

    pub fn bench_mul_ring(criterion: &mut Criterion) {
        let n_code = 12_800; // Code size
        let n_db = 100_000; // DB Size
        let n_batch = 20 * 31; // Batch size

        eprintln!("DB size:     {0} GB", (2 * n_db * n_code) as f64 / 1.0e9);
        eprintln!(
            "Batch size:  {0} MB (result {1} kB)",
            (2 * n_batch * n_code) as f64 / 1.0e6,
            (2 * n_batch) as f64 / 1.0e3
        );
        eprintln!("Result size: {0} MB", (2 * n_batch * n_db) as f64 / 1.0e6);

        eprintln!("Generating data ({n_batch}×{n_code}) and ({n_code}×{n_db})");
        let mut rng = rand::thread_rng();
        let mut q: Vec<u16> = vec![0_u16; n_batch * n_code];
        let mut d: Vec<u16> = vec![0_u16; n_code * n_db];
        let mut c: Vec<u16> = vec![0_u16; n_batch * n_db];
        rng.fill_bytes(cast_slice_mut(q.as_mut_slice()));
        rng.fill_bytes(cast_slice_mut(d.as_mut_slice()));

        eprintln!("Benchmarking ({n_batch}×{n_code})⋅({n_code}×{n_db}) multiplication in ℤ_2^16.");
        let start = Instant::now();
        {
            let (q, d) = black_box((&q, &d));
            dot_ring(c.as_mut_slice(), q, d);
        };
        let duration = Instant::now().duration_since(start).as_secs_f64();
        eprintln!("Duration:    {0} s", duration);
        eprintln!(
            "Throughput:  {0} k/s",
            (n_batch as f64) * (n_db as f64) / (1.0e3 * 31.0 * duration)
        );
        eprintln!(
            "FLOPS:       {0} GOp/s",
            2.0 * (n_batch as f64) * (n_db as f64) * (n_code as f64) / (1.0e9 * duration)
        );
        eprintln!(
            "Bandwidth:   {0} MB/s",
            2.0 * (n_batch as f64 + n_db as f64) * (n_code as f64) / (1.0e6 * duration)
        );
    }

    pub fn bench_mul_field(criterion: &mut Criterion) {
        let n_code = 12_800; // Code size
        let n_db = 100_000; // DB Size
        let n_batch = 20 * 31; // Batch size

        eprintln!("DB size:     {0} GB", (2 * n_db * n_code) as f64 / 1.0e9);
        eprintln!(
            "Batch size:  {0} MB (result {1} kB)",
            (2 * n_batch * n_code) as f64 / 1.0e6,
            (2 * n_batch) as f64 / 1.0e3
        );
        eprintln!("Result size: {0} MB", (2 * n_batch * n_db) as f64 / 1.0e6);

        eprintln!("Generating data ({n_batch}×{n_code}) and ({n_code}×{n_db})");
        let mut rng = rand::thread_rng();
        let mut q: Vec<u16> = vec![0_u16; n_batch * n_code];
        let mut d: Vec<u16> = vec![0_u16; n_code * n_db];
        let mut c: Vec<u16> = vec![0_u16; n_batch * n_db];
        rng.fill_bytes(cast_slice_mut(q.as_mut_slice()));
        rng.fill_bytes(cast_slice_mut(d.as_mut_slice()));

        eprintln!("Benchmarking ({n_batch}×{n_code})⋅({n_code}×{n_db}) multiplication in ℤ_2^16.");
        let start = Instant::now();
        {
            let (q, d) = black_box((&q, &d));
            dot_field(c.as_mut_slice(), q, d);
        };
        let duration = Instant::now().duration_since(start).as_secs_f64();
        eprintln!("Duration:    {0} s", duration);
        eprintln!(
            "Throughput:  {0} k/s",
            (n_batch as f64) * (n_db as f64) / (1.0e3 * 31.0 * duration)
        );
        eprintln!(
            "FLOPS:       {0} GOp/s",
            2.0 * (n_batch as f64) * (n_db as f64) * (n_code as f64) / (1.0e9 * duration)
        );
        eprintln!(
            "Bandwidth:   {0} MB/s",
            2.0 * (n_batch as f64 + n_db as f64) * (n_code as f64) / (1.0e6 * duration)
        );
    }
}
