use crate::BITS;
use bytemuck::{bytes_of, cast_slice_mut, try_cast_slice, try_cast_slice_mut, Pod, Zeroable};
use rand::{
    distributions::{Distribution, Standard},
    Rng,
};
use serde::{de::Error as _, Deserialize, Serialize};
use std::{fmt::Debug, ops, ops::Index};

const LIMBS: usize = BITS / 64;

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Bits(pub [u64; LIMBS]);

unsafe impl Zeroable for Bits {}

unsafe impl Pod for Bits {}

impl Index<usize> for Bits {
    type Output = bool;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < BITS);
        let b = self.0[index / 64] & (1 << index % 63) != 0;
        if b {
            &true
        } else {
            &false
        }
    }
}

impl Default for Bits {
    fn default() -> Self {
        Self([0; LIMBS])
    }
}

impl Debug for Bits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for limb in self.0 {
            write!(f, "{limb:016x}")?;
        }
        Ok(())
    }
}

impl Serialize for Bits {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        hex::serialize(bytes_of(self), serializer)
    }
}

impl<'de> Deserialize<'de> for Bits {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes: Vec<u8> = hex::deserialize(deserializer)?;
        let limbs = try_cast_slice(bytes.as_slice()).map_err(D::Error::custom)?;
        let limbs = limbs.try_into().map_err(D::Error::custom)?;
        Ok(Bits(limbs))
    }
}

impl Distribution<Bits> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Bits {
        let mut values = [0_u64; LIMBS];
        rng.fill_bytes(cast_slice_mut(values.as_mut_slice()));
        Bits(values)
    }
}

impl Bits {
    pub fn rotate(&mut self, amount: i32) {
        let bytes: &mut [u8] = try_cast_slice_mut(self.0.as_mut_slice()).unwrap();
        for chunk in bytes.chunks_exact_mut(25) {
            rotate_row(chunk.try_into().unwrap(), amount)
        }
    }
}

impl ops::Not for &Bits {
    type Output = Bits;

    fn not(self) -> Self::Output {
        let mut result = Bits::default();
        for (r, s) in result.0.iter_mut().zip(self.0.iter()) {
            *r = !s;
        }
        result
    }
}

impl ops::BitAnd for &Bits {
    type Output = Bits;

    fn bitand(self, rhs: Self) -> Self::Output {
        let mut result = *self;
        result &= rhs;
        result
    }
}

impl ops::BitOr for &Bits {
    type Output = Bits;

    fn bitor(self, rhs: Self) -> Self::Output {
        let mut result = *self;
        result |= rhs;
        result
    }
}

impl ops::BitXor for &Bits {
    type Output = Bits;

    fn bitxor(self, rhs: Self) -> Self::Output {
        let mut result = *self;
        result ^= rhs;
        result
    }
}

impl ops::BitAndAssign<&Bits> for Bits {
    fn bitand_assign(&mut self, rhs: &Self) {
        for (s, r) in self.0.iter_mut().zip(rhs.0.iter()) {
            s.bitand_assign(r);
        }
    }
}

impl ops::BitOrAssign<&Bits> for Bits {
    fn bitor_assign(&mut self, rhs: &Self) {
        for (s, r) in self.0.iter_mut().zip(rhs.0.iter()) {
            s.bitor_assign(r);
        }
    }
}

impl ops::BitXorAssign<&Bits> for Bits {
    fn bitxor_assign(&mut self, rhs: &Self) {
        for (s, r) in self.0.iter_mut().zip(rhs.0.iter()) {
            s.bitxor_assign(r);
        }
    }
}

fn rotate_row(a: &mut [u8; 25], amount: i32) {
    assert!(amount >= -15);
    assert!(amount <= 15);
    if amount < -8 {
        rotate_row(a, -8);
        rotate_row(a, amount + 8);
    } else if amount == -8 {
        let first = a[0];
        a.copy_within(1.., 0);
        a[24] = first;
    } else if amount < 0 {
        let l = -amount;
        let r = 8 - l;
        let mut carry = a[0] >> r;
        for b in a.iter_mut().rev() {
            let old = *b;
            *b = (old << l) | carry;
            carry = old >> r;
        }
    } else if amount == 0 {
        return;
    } else if amount < 8 {
        let r = amount;
        let l = 8 - r;
        let mut carry = a[24] << l;
        for b in a.iter_mut() {
            let old = *b;
            *b = (old >> r) | carry;
            carry = old << l;
        }
    } else if amount == 8 {
        let last = a[24];
        a.copy_within(0..24, 1);
        a[0] = last;
    } else {
        rotate_row(a, 8);
        rotate_row(a, amount - 8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limbs_exact() {
        assert_eq!(LIMBS * 64, BITS);
    }
}
