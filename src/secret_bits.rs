use crate::{Bits, BITS};
use bytemuck::{cast_slice_mut, Pod, Zeroable};
use rand::{
    distributions::{Distribution, Standard},
    thread_rng, Rng,
};
use std::{
    array,
    iter::{self, Sum},
    ops,
};

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct SecretBits(pub [u16; BITS]);

unsafe impl Zeroable for SecretBits {}

unsafe impl Pod for SecretBits {}

impl SecretBits {
    /// Generate secret shares from this bitvector.
    pub fn share(&self, n: usize) -> Box<[SecretBits]> {
        assert!(n > 0);

        // Create `n - 1` random shares.
        let mut rng = thread_rng();
        let mut result: Box<[SecretBits]> = iter::repeat_with(|| rng.gen::<SecretBits>())
            .take(n - 1)
            .chain(iter::once(SecretBits([0_u16; BITS])))
            .collect();
        let (last, rest) = result.split_last_mut().unwrap();

        // Initialize last to sum of self
        *last = self - rest.iter().sum::<SecretBits>();

        result
    }
}

impl Default for SecretBits {
    fn default() -> Self {
        Self([0; BITS])
    }
}

impl From<&Bits> for SecretBits {
    fn from(value: &Bits) -> Self {
        SecretBits(array::from_fn(|i| if value[i] { 1 } else { 0 }))
    }
}

impl Distribution<SecretBits> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> SecretBits {
        let mut values = [0_u16; BITS];
        rng.fill_bytes(cast_slice_mut(values.as_mut_slice()));
        SecretBits(values)
    }
}

impl ops::Neg for &SecretBits {
    type Output = SecretBits;

    fn neg(self) -> Self::Output {
        let mut result = *self;
        for r in result.0.iter_mut() {
            *r = 0_u16.wrapping_sub(*r);
        }
        result
    }
}

impl<'a> Sum<&'a SecretBits> for SecretBits {
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        let mut result = Self::default();
        for i in iter {
            result += i;
        }
        result
    }
}

impl ops::Sub<SecretBits> for &SecretBits {
    type Output = SecretBits;

    fn sub(self, mut rhs: SecretBits) -> Self::Output {
        for (a, &b) in rhs.0.iter_mut().zip(self.0.iter()) {
            *a = b.wrapping_sub(*a);
        }
        rhs
    }
}

impl ops::AddAssign<&SecretBits> for SecretBits {
    fn add_assign(&mut self, rhs: &SecretBits) {
        for (s, &r) in self.0.iter_mut().zip(rhs.0.iter()) {
            *s = s.wrapping_add(r);
        }
    }
}

impl ops::SubAssign<&SecretBits> for SecretBits {
    fn sub_assign(&mut self, rhs: &SecretBits) {
        for (s, &r) in self.0.iter_mut().zip(rhs.0.iter()) {
            *s = s.wrapping_sub(r);
        }
    }
}

impl ops::MulAssign<&SecretBits> for SecretBits {
    fn mul_assign(&mut self, rhs: &SecretBits) {
        for (s, &r) in self.0.iter_mut().zip(rhs.0.iter()) {
            *s = s.wrapping_mul(r);
        }
    }
}
