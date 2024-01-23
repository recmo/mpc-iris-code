use crate::{SecretTemplate, Template};
use core::arch::aarch64::{
    uint16x8_t, uint16x8x4_t, vaddq_u16, vaddvq_u16, vandq_u16, vbicq_u16, vdupq_n_u16,
    vextq_u16,  vld1q_u16, vld1q_u16_x4, vsubq_u16, vtstq_u16,
};
use std::ops::AddAssign;

/// A SIMD block is a mask of 8 elements and 4x8 elements of pattern.
/// A full template has exactly 400 such blocks.
/// If it is cleartext then elements are either 0x0000 or 0xffff.
/// If it is ciphertext the pattern elements are u16s.
#[derive(Clone, Copy, Debug)]
struct Block(uint16x8_t, uint16x8x4_t);

/// Compact cleartext storage of a block
/// It is 8 bits of mask followed by 4x8 bits of pattern.
#[derive(Clone, Copy, Debug, Default)]
struct ClearBlock([u8; 5]);

/// Compact cleartext storage of a template
/// The first four blocks are repeated at the end to facilitate wraparound
/// windows
#[derive(Clone, Copy, Debug)]
pub struct ClearTemplate([ClearBlock; 404]);

/// Storage of a ciphertext block.
/// Contains 8 u16s of mask and 4x8 u16s of ciphertext pattern
/// The mask is cleartext as 0x0000 or 0xffff
#[derive(Clone, Copy, Debug)]
struct CipherBlock([u16; 40]);

/// Storage of a ciphertext template
#[derive(Clone, Copy, Debug)]
pub struct CipherTemplate([CipherBlock; 400]);

/// A fractional hamming distance with as numerator, denoninator
/// The numerator is in ciphertext.
#[derive(Clone, Copy, Debug, Default)]
pub struct Distance(u16, u16);

impl From<ClearBlock> for Block {
    fn from(value: ClearBlock) -> Self {
        unsafe {
            const BIT_POS: [u16; 8] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80];
            let bit_pos = vld1q_u16(BIT_POS.as_ptr());
            Self(
                vtstq_u16(bit_pos, vdupq_n_u16(value.0[0] as u16)),
                uint16x8x4_t(
                    vtstq_u16(bit_pos, vdupq_n_u16(value.0[1] as u16)),
                    vtstq_u16(bit_pos, vdupq_n_u16(value.0[2] as u16)),
                    vtstq_u16(bit_pos, vdupq_n_u16(value.0[3] as u16)),
                    vtstq_u16(bit_pos, vdupq_n_u16(value.0[4] as u16)),
                ),
            )
        }
    }
}

impl From<&CipherBlock> for Block {
    fn from(value: &CipherBlock) -> Self {
        unsafe {
            Self(
                vld1q_u16(value.0.as_ptr()),
                vld1q_u16_x4(value.0[8..].as_ptr()),
            )
        }
    }
}

impl From<&Template> for ClearTemplate {
    fn from(value: &Template) -> Self {
        let mut result = [ClearBlock::default(); 404];
        for i in 0..400 {
            result[i] = ClearBlock([
                value.mask.0[i / 8].to_le_bytes()[i % 8],
                value.pattern.0[i / 8].to_le_bytes()[i % 8],
                value.pattern.0[50 + (i / 8)].to_le_bytes()[i % 8],
                value.pattern.0[100 + (i / 8)].to_le_bytes()[i % 8],
                value.pattern.0[150 + (i / 8)].to_le_bytes()[i % 8],
            ])
        }

        // First four blocks are repeated at the end.
        result.copy_within(0..4, 400);
        Self(result)
    }
}

impl From<&SecretTemplate> for CipherTemplate {
    fn from(value: &SecretTemplate) -> Self {
        let mut result = [CipherBlock([0; 40]); 400];
        for i in 0..400 {
            let o = i * 8;
            // result[i].0[0..8].copy_from_slice(src); // TODO
            result[i].0[8..16].copy_from_slice(&value.pattern.0[o..o + 8]);
            result[i].0[16..24].copy_from_slice(&value.pattern.0[o + 3200..o + 3208]);
            result[i].0[24..32].copy_from_slice(&value.pattern.0[o + 6400..o + 6408]);
            result[i].0[32..40].copy_from_slice(&value.pattern.0[o + 9600..o + 9608]);
        }
        Self(result)
    }
}

impl AddAssign for Distance {
    fn add_assign(&mut self, rhs: Self) {
        self.0 = self.0.wrapping_add(rhs.0);
        self.1 = self.1.wrapping_add(rhs.1);
    }
}

/// Measure fractional hamming distance between a cleartext and a ciphertext
/// block.
fn block_distance(a: Block, b: Block) -> Distance {
    unsafe {
        let mask = vandq_u16(a.0, b.0);
        let denonominator = vaddvq_u16(mask).wrapping_neg(); // Using the fact that mask is 0x0000 or 0xffff.
        let sum = vaddvq_u16(vandq_u16(
            mask,
            vsubq_u16(
                vaddq_u16(
                    vaddq_u16(vbicq_u16(b.1 .0, a.1 .0), vbicq_u16(b.1 .1, a.1 .1)),
                    vaddq_u16(vbicq_u16(b.1 .2, a.1 .2), vbicq_u16(b.1 .3, a.1 .3)),
                ),
                vaddq_u16(
                    vaddq_u16(vandq_u16(b.1 .0, a.1 .0), vandq_u16(b.1 .1, a.1 .1)),
                    vaddq_u16(vandq_u16(b.1 .2, a.1 .2), vandq_u16(b.1 .3, a.1 .3)),
                ),
            ),
        ));
        Distance(sum, denonominator)
    }
}

/// Computes rotation between two blocks
fn block_rotate<const N: i32>(a: Block, b: Block) -> Block {
    unsafe {
        Block(
            vextq_u16::<N>(a.0, b.0),
            uint16x8x4_t(
                vextq_u16::<N>(a.1 .0, b.1 .0),
                vextq_u16::<N>(a.1 .1, b.1 .1),
                vextq_u16::<N>(a.1 .2, b.1 .2),
                vextq_u16::<N>(a.1 .3, b.1 .3),
            ),
        )
    }
}

/// Computes 31 rotated distances between a window of 5 cleartext blocks and one
/// ciphertext block. OPT: If `from_clear` is a heavy we can cache the decoding
/// between ciphertext blocks.      as only one new block gets added to the
/// sliding window.
fn rotations_31(dist: &mut [Distance; 31], clear: &[ClearBlock; 5], t: &CipherBlock) {
    let t = t.into();
    let a = clear[0].into();

    dist[0] += block_distance(a, t);
    let b = clear[1].into();
    dist[1] += block_distance(block_rotate::<1>(a, b), t);
    dist[2] += block_distance(block_rotate::<2>(a, b), t);
    dist[3] += block_distance(block_rotate::<3>(a, b), t);
    dist[4] += block_distance(block_rotate::<4>(a, b), t);
    dist[5] += block_distance(block_rotate::<5>(a, b), t);
    dist[6] += block_distance(block_rotate::<6>(a, b), t);
    dist[7] += block_distance(block_rotate::<7>(a, b), t);

    let a = b;
    dist[8] += block_distance(a, t);
    let b = clear[2].into();
    dist[9] += block_distance(block_rotate::<1>(a, b), t);
    dist[10] += block_distance(block_rotate::<2>(a, b), t);
    dist[11] += block_distance(block_rotate::<3>(a, b), t);
    dist[12] += block_distance(block_rotate::<4>(a, b), t);
    dist[13] += block_distance(block_rotate::<5>(a, b), t);
    dist[14] += block_distance(block_rotate::<6>(a, b), t);
    dist[15] += block_distance(block_rotate::<7>(a, b), t);

    let a = b;
    dist[16] += block_distance(a, t);
    let b = clear[3].into();
    dist[17] += block_distance(block_rotate::<1>(a, b), t);
    dist[18] += block_distance(block_rotate::<2>(a, b), t);
    dist[19] += block_distance(block_rotate::<3>(a, b), t);
    dist[20] += block_distance(block_rotate::<4>(a, b), t);
    dist[21] += block_distance(block_rotate::<5>(a, b), t);
    dist[22] += block_distance(block_rotate::<6>(a, b), t);
    dist[23] += block_distance(block_rotate::<7>(a, b), t);

    let a = b;
    dist[24] += block_distance(a, t);
    let b = clear[4].into();
    dist[25] += block_distance(block_rotate::<1>(a, b), t);
    dist[26] += block_distance(block_rotate::<2>(a, b), t);
    dist[27] += block_distance(block_rotate::<3>(a, b), t);
    dist[28] += block_distance(block_rotate::<4>(a, b), t);
    dist[29] += block_distance(block_rotate::<5>(a, b), t);
    dist[30] += block_distance(block_rotate::<6>(a, b), t);
}

/// Compute distances including rotations.
pub fn distances(clear: &[ClearTemplate], cipher: &CipherTemplate) -> Box<[[Distance; 31]]> {
    let mut result: Box<[[Distance; 31]]> =
        clear.iter().map(|_| [Distance::default(); 31]).collect();
    for i in 0..400 {
        for (j, clear) in clear.iter().enumerate() {
            rotations_31(
                &mut result[j],
                clear.0[i..i + 5].try_into().unwrap(),
                &cipher.0[i],
            );
        }
    }
    result
}

#[cfg(feature = "bench")]
pub mod benches {
    use super::*;
    use core::hint::black_box;
    use criterion::Criterion;
    use rand::{thread_rng, Rng};
    use std::mem::size_of;

    pub fn group(c: &mut Criterion) {
        eprintln!("Sizeof ClearTemplate: {}", size_of::<ClearTemplate>());
        eprintln!("Sizeof CipherTemplate: {}", size_of::<CipherTemplate>());

        let mut rng = thread_rng();

        // Generate 100 query template
        let queries: Box<[Template]> = (0..10).map(|_| rng.gen()).collect();

        // Generate 1000 reference templates (database)
        let db: Box<[SecretTemplate]> = (0..100_000).map(|_| rng.gen()).collect();

        // Preprocess
        let queries: Box<[ClearTemplate]> = queries.iter().map(|t| t.into()).collect();
        let db: Box<[CipherTemplate]> = db.iter().map(|t| t.into()).collect();

        c.bench_function("Bench distance4 (100x31x1000)", |b| {
            b.iter(|| {
                for reference in &*db {
                    let _ = black_box(distances(&queries, &reference));
                }
            })
        });
    }
}
