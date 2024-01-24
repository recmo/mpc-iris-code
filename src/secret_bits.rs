use crate::{Bits, BITS};
use bytemuck::{cast_slice_mut, Pod, Zeroable};
use rand::{
    distributions::{Distribution, Standard},
    thread_rng, Rng,
};
use std::iter::{self};

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SecretBits(pub [u16; BITS]);

unsafe impl Zeroable for SecretBits {}

unsafe impl Pod for SecretBits {}

impl SecretBits {
    pub fn from_bits(bits: &Bits, count: usize) -> Box<[SecretBits]> {
        assert!(count > 0);
        let mut rng = thread_rng();
        let mut result: Box<[SecretBits]> = iter::repeat_with(|| rng.gen::<SecretBits>())
            .take(count - 1)
            .chain(iter::once(SecretBits([0_u16; BITS])))
            .collect();

        // Make shares sum to zero
        let (last, rest) = result.split_last_mut().unwrap();
        for share in rest {
            for (last, share) in last.0.iter_mut().zip(share.0.iter()) {
                *last = last.wrapping_sub(*share);
            }
        }

        // Add secret
        for (i, share) in last.0.iter_mut().enumerate() {
            let value = if bits[i] { 0 } else { 1 };
            *share = share.wrapping_add(value);
        }

        result
    }
}

impl Distribution<SecretBits> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> SecretBits {
        let mut values = [0_u16; BITS];
        rng.fill_bytes(cast_slice_mut(values.as_mut_slice()));
        SecretBits(values)
    }
}
