use crate::{Bits, BITS, COLS};
use bytemuck::{cast_slice_mut, Pod, Zeroable};
use rand::{
    distributions::{Distribution, Standard},
    thread_rng, Rng,
};
use std::{
    array,
    iter::{self, Sum},
    ops::{self, MulAssign},
};

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct EncodedBits(pub [u16; BITS]);

unsafe impl Zeroable for EncodedBits {}

unsafe impl Pod for EncodedBits {}

impl EncodedBits {
    /// Generate secret shares from this bitvector.
    pub fn share(&self, n: usize) -> Box<[EncodedBits]> {
        assert!(n > 0);

        // Create `n - 1` random shares.
        let mut rng = thread_rng();
        let mut result: Box<[EncodedBits]> = iter::repeat_with(|| rng.gen::<EncodedBits>())
            .take(n - 1)
            .chain(iter::once(EncodedBits([0_u16; BITS])))
            .collect();
        let (last, rest) = result.split_last_mut().unwrap();

        // Initialize last to sum of self
        *last = self - rest.iter().sum::<EncodedBits>();

        result
    }

    pub fn rotate(&mut self, amount: i32) {
        if amount < 0 {
            let amount = amount.abs() as usize;
            for row in self.0.chunks_exact_mut(COLS) {
                row.rotate_left(amount);
            }
        } else if amount > 0 {
            let amount = amount as usize;
            for row in self.0.chunks_exact_mut(COLS) {
                row.rotate_right(amount);
            }
        }
    }

    pub fn rotated(&self, amount: i32) -> Self {
        let mut copy = *self;
        copy.rotate(amount);
        copy
    }

    pub fn sum(&self) -> u16 {
        self.0.iter().copied().fold(0_u16, u16::wrapping_add)
    }

    pub fn dot(&self, other: &Self) -> u16 {
        (self * other).sum()
    }
}

impl Default for EncodedBits {
    fn default() -> Self {
        Self([0; BITS])
    }
}

impl From<&Bits> for EncodedBits {
    fn from(value: &Bits) -> Self {
        EncodedBits(array::from_fn(|i| if value[i] { 1 } else { 0 }))
    }
}

impl Distribution<EncodedBits> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> EncodedBits {
        let mut values = [0_u16; BITS];
        rng.fill_bytes(cast_slice_mut(values.as_mut_slice()));
        EncodedBits(values)
    }
}

impl ops::Neg for EncodedBits {
    type Output = EncodedBits;

    fn neg(mut self) -> Self::Output {
        for r in self.0.iter_mut() {
            *r = 0_u16.wrapping_sub(*r);
        }
        self
    }
}

impl ops::Neg for &EncodedBits {
    type Output = EncodedBits;

    fn neg(self) -> Self::Output {
        let mut result = *self;
        for r in result.0.iter_mut() {
            *r = 0_u16.wrapping_sub(*r);
        }
        result
    }
}

impl<'a> Sum<&'a EncodedBits> for EncodedBits {
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        let mut result = Self::default();
        for i in iter {
            result += i;
        }
        result
    }
}

impl ops::Sub<EncodedBits> for &EncodedBits {
    type Output = EncodedBits;

    fn sub(self, mut rhs: EncodedBits) -> Self::Output {
        for (a, &b) in rhs.0.iter_mut().zip(self.0.iter()) {
            *a = b.wrapping_sub(*a);
        }
        rhs
    }
}

impl ops::Sub<&EncodedBits> for EncodedBits {
    type Output = EncodedBits;

    fn sub(mut self, rhs: &EncodedBits) -> Self::Output {
        self -= rhs;
        self
    }
}

impl ops::Mul for &EncodedBits {
    type Output = EncodedBits;

    fn mul(self, rhs: Self) -> Self::Output {
        let mut copy = *self;
        copy.mul_assign(rhs);
        copy
    }
}

impl ops::Mul<&EncodedBits> for EncodedBits {
    type Output = EncodedBits;

    fn mul(mut self, rhs: &EncodedBits) -> Self::Output {
        self.mul_assign(rhs);
        self
    }
}

impl ops::AddAssign<&EncodedBits> for EncodedBits {
    fn add_assign(&mut self, rhs: &EncodedBits) {
        for (s, &r) in self.0.iter_mut().zip(rhs.0.iter()) {
            *s = s.wrapping_add(r);
        }
    }
}

impl ops::SubAssign<&EncodedBits> for EncodedBits {
    fn sub_assign(&mut self, rhs: &EncodedBits) {
        for (s, &r) in self.0.iter_mut().zip(rhs.0.iter()) {
            *s = s.wrapping_sub(r);
        }
    }
}

impl ops::MulAssign<&EncodedBits> for EncodedBits {
    fn mul_assign(&mut self, rhs: &EncodedBits) {
        for (s, &r) in self.0.iter_mut().zip(rhs.0.iter()) {
            *s = s.wrapping_mul(r);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotated_inverse() {
        let mut rng = thread_rng();

        for _ in 0..100 {
            let secret: EncodedBits = rng.gen();
            for amount in -15..=15 {
                assert_eq!(
                    secret.rotated(amount).rotated(-amount),
                    secret,
                    "Rotation failed for {amount}"
                )
            }
        }
    }

    #[test]
    fn test_rotated_number() {
        let secret = EncodedBits(array::from_fn(|i| {
            let (row, col) = (i / COLS, i % COLS);
            (row << 8 | col) as u16
        }));
        for amount in -15..=15 {
            let rotated = secret.rotated(amount);
            for (i, &v) in rotated.0.iter().enumerate() {
                let (row, col) = (i / COLS, i % COLS);
                let col = (((COLS + col) as i32 - amount) % (COLS as i32)) as usize;
                assert_eq!(v, (row << 8 | col) as u16);
            }
        }
    }

    #[test]
    fn test_rotated_bits() {
        let mut rng = thread_rng();

        for _ in 0..100 {
            let bits: Bits = rng.gen();
            let secret = EncodedBits::from(&bits);
            for amount in -15..=15 {
                assert_eq!(
                    EncodedBits::from(&bits.rotated(amount)),
                    secret.rotated(amount),
                    "Rotation equivalence failed for {amount}"
                )
            }
        }
    }
}
