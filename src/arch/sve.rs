#![cfg(target_feature = "sve")]
#![allow(unused)]
use crate::{bits::LIMBS, BITS};
use std::arch::asm;

pub fn dot_bool(a: &[u64; LIMBS], b: &[u64; LIMBS]) -> u16 {
    a.iter()
        .zip(b.iter())
        .map(|(&a, &b)| (a & b).count_ones() as u16)
        .fold(0_u16, u16::wrapping_add)
}

pub fn width() -> usize {
    let width: usize;
    unsafe {
        asm!(
            "
            mov {width}, #0
            inch    {width}
            ",
            width = out(reg) width,
        );
    }
    width
}

pub fn dot_u16(a: &[u16; BITS], b: &[u16; BITS]) -> u16 {
    debug_assert_eq!(BITS % width(), 0); // No partial loads.

    let result: u64;
    unsafe {
        asm!(
            "
            mov     {c}, #0          // Loop counter
            mov     {e}, #12800      // Loop length
            mov     {i}, #0          // Loop step
            inch    {i}
            inch    {i}
            inch    {i}
            inch    {i}

            dup     z8.h, #0
            dup     z9.h, #0
            dup     z10.h, #0
            dup     z11.h, #0
            ptrue   p0.h

        0:
            ld4h    {{ z0.h, z1.h, z2.h, z3.h }}, p0/z, [{a}, {c}, lsl #1]
            ld4h    {{ z4.h, z5.h, z6.h, z7.h }}, p0/z, [{b}, {c}, lsl #1]
            mla     z8.h, p0/m, z0.h, z4.h
            mla     z9.h, p0/m, z1.h, z5.h
            mla     z10.h, p0/m, z2.h, z6.h
            mla     z11.h, p0/m, z3.h, z7.h

            add     {c}, {c}, {i}
            cmp     {c}, {e}
            b.ne    0b

            // Add across vector
            add     z8.h, z8.h, z9.h
            add     z10.h, z10.h, z11.h
            add     z8.h, z8.h, z10.h
            uaddv   {r:d}, p0, z8.h

            ",
            a = in(reg) a,
            b = in(reg) b,
            r = out(vreg) result,
            c = out(reg) _, // Counter
            i = out(reg) _, // Increment
            e = out(reg) _, // Length
            // TODO: Clobbers
        );
    }
    result as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    #[test]
    fn test_dot_u16() {
        let mut a_vals = [0u16; BITS];
        let mut b_vals = [0u16; BITS];
        let mut rng = rand::thread_rng();
        for val in a_vals.iter_mut() {
            *val = rng.gen();
        }
        for val in b_vals.iter_mut() {
            *val = rng.gen();
        }

        let result = dot_u16(&a_vals, &b_vals);

        let expected = a_vals
            .iter()
            .zip(b_vals.iter())
            .map(|(&a, &b)| a as u64 * b as u64)
            .sum::<u64>() as u16;

        assert_eq!(
            result, expected,
            "dot_u16 did not produce the expected result"
        );
    }
}

#[cfg(feature = "bench")]
pub mod benches {
    use super::{
        super::benches::{bench_dot_bool, bench_dot_u16},
        *,
    };
    use criterion::Criterion;

    pub fn group(criterion: &mut Criterion) {
        bench_dot_bool(criterion, "sve/dot_bool", dot_bool);
        bench_dot_u16(criterion, "sve/dot_u16", dot_u16);
    }
}
