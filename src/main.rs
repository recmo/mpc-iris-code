use std::time::{Duration, Instant};
use mpc_iris_code::{Template, SecretTemplate, preprocess::{ClearTemplate, CipherTemplate, distances}};
use rand::{thread_rng, Rng};
use rayon::{slice::ParallelSlice, iter::{IntoParallelRefIterator, ParallelIterator}};

fn main() {
    let mut rng = thread_rng();

    let n: usize = 1;
    let m: usize = 1_000_000;

    // Generate query templates
    let queries: Box<[Template]> = (0..n).map(|_| rng.gen()).collect();

    // Generate reference templates (database)
    eprintln!("Generating {m} reference templates.");
    let db: Box<[SecretTemplate]> = (0..m).map(|_| rng.gen()).collect();

    // Preprocess
    eprintln!("Preprocessing.");
    let start_time = Instant::now();
    let queries: Box<[ClearTemplate]> = queries.iter().map(|t| t.into()).collect();
    let db: Box<[CipherTemplate]> = db.iter().map(|t| t.into()).collect();
    eprintln!("Time taken preparing size {m} database: {:?}", start_time.elapsed());

    eprintln!("Comparing");
    let start_time = Instant::now();
    let _distances = db.par_iter().map(|reference| {
        distances(&queries, &reference)
    }).collect::<Box<[_]>>();

    let elapsed_time = start_time.elapsed();
    eprintln!("Time taken for {n} x 31 queries into size {m} database calculations: {elapsed_time:?}",);
}
