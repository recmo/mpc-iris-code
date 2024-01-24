pub use crate::bits::Bits;
use bytemuck::{Pod, Zeroable};
use itertools::izip;
use rand::{
    distributions::{Distribution, Standard},
    Rng,
};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[repr(C)]
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Debug,
    Default,
    Serialize,
    Deserialize,
    Pod,
    Zeroable,
)]
pub struct Template {
    pub pattern: Bits,
    pub mask:    Bits,
}

impl Template {
    pub fn rotate(&mut self, amount: i32) {
        self.mask.rotate(amount);
        self.pattern.rotate(amount);
    }

    pub fn rotated(&self, amount: i32) -> Self {
        let mut copy = *self;
        copy.rotate(amount);
        copy
    }

    pub fn distance(&self, other: &Self) -> f64 {
        (-15..=15)
            .map(|r| self.rotated(r).fraction_hamming(other))
            .fold(f64::INFINITY, |a, b| a.min(b))
    }

    pub fn fraction_hamming(&self, other: &Self) -> f64 {
        let mut num = 0;
        let mut den = 0;
        for (ap, am, bp, bm) in izip!(
            self.pattern.0.iter(),
            self.mask.0.iter(),
            other.pattern.0.iter(),
            other.mask.0.iter()
        ) {
            let m = am & bm;
            let p = (ap ^ bp) & m;
            num += p.count_ones();
            den += m.count_ones();
        }
        (num as f64) / (den as f64)
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

#[cfg(test)]
mod tests {
    use super::*;
    use float_eq::assert_float_eq;
    use std::fs::File;

    #[test]
    #[ignore] // Requires test data
    fn test_distance_ref() {
        #[derive(Deserialize)]
        struct Distance {
            left:     usize,
            right:    usize,
            distance: f64,
        };

        // Read templates
        let file = File::open("data/templates.json").unwrap();
        let data: Vec<Template> = serde_json::from_reader(file).unwrap();
        assert_eq!(data.len(), 2092);

        // Read distances
        let file = File::open("data/distances.json").unwrap();
        let distances: Vec<Distance> = serde_json::from_reader(file).unwrap();
        assert_eq!(distances.len(), 1000);

        // Check distances to within 10 ulp
        for d in distances {
            let expected = d.distance;
            let actual = data[d.left].distance(&data[d.right]);
            assert_float_eq!(actual, expected, ulps <= 1);
        }
    }
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
