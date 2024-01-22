use itertools::izip;
use rand::{
    distributions::{Distribution, Standard},
    thread_rng, Rng,
};
use std::{array, mem::size_of};

const BITS: usize = 16 * 200 * 4;
const LIMBS: usize = BITS / 64;

#[repr(transparent)]
struct Bits([u64; LIMBS]);

#[repr(transparent)]
struct SecretBits([u16; BITS]);

struct Template {
    pattern: Bits,
    mask:    Bits,
}

struct SecretTemplate {
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

fn distance(query: &Template, reference: &SecretTemplate) -> (u16, u16) {
    let mut sum: u16 = 0;
    let mut denominator: u16 = 0;
    for (a, a_mask, b, b_mask) in izip!(
        query.pattern.0.iter(),
        query.mask.0.iter(),
        reference.pattern.0.chunks_exact(64),
        reference.mask.0.iter(),
    ) {
        let mask = a_mask & b_mask;
        let d = a & mask;
        sum = sum.wrapping_add(d.count_ones() as u16);
        denominator = denominator.wrapping_add(mask.count_ones() as u16);
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
    }
    (sum, denominator)
}

fn main() {
    eprintln!("Size of Template: {}", size_of::<Template>());
    eprintln!("Size of SecretTemplate: {}", size_of::<SecretTemplate>());

    let mut rng = thread_rng();
    let query: Template = rng.gen();
    let reference: SecretTemplate = rng.gen();
    let (s, d) = distance(&query, &reference);
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
}

#[cfg(feature = "bench")]
pub mod benches {
    use super::*;
    use criterion::Criterion;

    pub fn group(c: &mut Criterion) {
        let mut rng = thread_rng();

        // Generate 31 query templates (rotations)
        let queries: Box<[Template]> = (0..31).map(|_| rng.gen()).collect();

        // Generate 1000 reference templates (database)
        let db: Box<[SecretTemplate]>  = (0..1000).map(|_| rng.gen()).collect();

        c.bench_function("Bench query lookup", |b| b.iter(|| {
            for reference in &*db {
                for query in &*queries {
                    let (s, d) = distance(query, reference);
                }
            }
        }));
    }
}
