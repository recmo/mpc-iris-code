use crate::{SecretTemplate, Template};
use core::arch::aarch64::{
    uint16x8_t, uint16x8x4_t, vaddq_u16, vaddvq_u16, vandq_u16, vbicq_u16, vdupq_n_u16,
    vextq_u16,  vld1q_u16, vld1q_u16_x4, vsubq_u16, vtstq_u16,
};
use std::{ops::AddAssign, arch::aarch64::{vmla_u16, vmulq_u16, vmlaq_u16}, hint::black_box};

/// A SIMD block is 4x8 elements of pattern, premultiplied by mask.
/// A full template has exactly 400 such blocks.
/// If it is cleartext then elements are either 0x0000, 0x0001 or 0xffff.
/// If it is ciphertext the pattern elements are u16s.
#[derive(Clone, Copy, Debug)]
struct Block(uint16x8x4_t);

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
/// Contains 4x8 u16s of ciphertext pattern, premultiplied by mask
#[derive(Clone, Copy, Debug)]
struct CipherBlock([u16; 8 * 4]);

/// Storage of a ciphertext template
#[derive(Clone, Copy, Debug)]
pub struct CipherTemplate([CipherBlock; 400]);

/// A fractional hamming distance numerator ciphertext
#[derive(Clone, Copy, Debug, Default)]
pub struct Distance(u16);

impl From<ClearBlock> for Block {
    fn from(value: ClearBlock) -> Self {
        unsafe {
            const BIT_POS: [u16; 8] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80];
            let bit_pos = vld1q_u16(BIT_POS.as_ptr());
            let mask = vtstq_u16(bit_pos, vdupq_n_u16(value.0[0] as u16));
            Self(uint16x8x4_t(
                vtstq_u16(bit_pos, vdupq_n_u16(value.0[1] as u16)),
                vtstq_u16(bit_pos, vdupq_n_u16(value.0[2] as u16)),
                vtstq_u16(bit_pos, vdupq_n_u16(value.0[3] as u16)),
                vtstq_u16(bit_pos, vdupq_n_u16(value.0[4] as u16)),
            ))
        }
    }
}

impl From<&CipherBlock> for Block {
    fn from(value: &CipherBlock) -> Self {
        unsafe {
            Self(vld1q_u16_x4(value.0.as_ptr()))
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
        let mut result = [CipherBlock([0; 32]); 400];
        for i in 0..400 {
            let o = i * 8;
            result[i].0[0..8].copy_from_slice(&value.pattern.0[o..o + 8]);
            result[i].0[8..16].copy_from_slice(&value.pattern.0[o + 3200..o + 3208]);
            result[i].0[16..24].copy_from_slice(&value.pattern.0[o + 6400..o + 6408]);
            result[i].0[24..32].copy_from_slice(&value.pattern.0[o + 9600..o + 9608]);
        }
        Self(result)
    }
}

impl AddAssign for Distance {
    fn add_assign(&mut self, rhs: Self) {
        black_box(rhs);
        // self.0 = self.0.wrapping_add(rhs.0);
    }
}

/// Measure fractional hamming distance between a cleartext and a ciphertext
/// block.
fn block_distance(a: Block, b: Block) -> Distance {
    unsafe {
        let sum = vmulq_u16(a.0.0, b.0.0);
        let sum = vmlaq_u16(a.0.1, b.0.1, sum);
        let sum = vmlaq_u16(a.0.2, b.0.2, sum);
        let sum = vmlaq_u16(a.0.3, b.0.3, sum);
        let sum = vaddvq_u16(sum);
        Distance(sum)
    }
}

/// Computes rotation between two blocks
fn block_rotate<const N: i32>(a: Block, b: Block) -> Block {
    unsafe {
        Block(uint16x8x4_t(
            vextq_u16::<N>(a.0.0, b.0.0),
            vextq_u16::<N>(a.0.1, b.0.1),
            vextq_u16::<N>(a.0.2, b.0.2),
            vextq_u16::<N>(a.0.3, b.0.3),
        ))
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
    dist[0] += block_distance(block_rotate::<1>(a, b), t);
    dist[0] += block_distance(block_rotate::<2>(a, b), t);
    dist[0] += block_distance(block_rotate::<3>(a, b), t);
    dist[0] += block_distance(block_rotate::<4>(a, b), t);
    dist[0] += block_distance(block_rotate::<5>(a, b), t);
    dist[0] += block_distance(block_rotate::<6>(a, b), t);
    dist[0] += block_distance(block_rotate::<7>(a, b), t);

    let a = b;
    dist[0] += block_distance(a, t);
    let b = clear[2].into();
    dist[0] += block_distance(block_rotate::<1>(a, b), t);
    dist[0] += block_distance(block_rotate::<2>(a, b), t);
    dist[0] += block_distance(block_rotate::<3>(a, b), t);
    dist[0] += block_distance(block_rotate::<4>(a, b), t);
    dist[0] += block_distance(block_rotate::<5>(a, b), t);
    dist[0] += block_distance(block_rotate::<6>(a, b), t);
    dist[0] += block_distance(block_rotate::<7>(a, b), t);

    let a = b;
    dist[0] += block_distance(a, t);
    let b = clear[3].into();
    dist[0] += block_distance(block_rotate::<1>(a, b), t);
    dist[0] += block_distance(block_rotate::<2>(a, b), t);
    dist[0] += block_distance(block_rotate::<3>(a, b), t);
    dist[0] += block_distance(block_rotate::<4>(a, b), t);
    dist[0] += block_distance(block_rotate::<5>(a, b), t);
    dist[0] += block_distance(block_rotate::<6>(a, b), t);
    dist[0] += block_distance(block_rotate::<7>(a, b), t);

    let a = b;
    dist[0] += block_distance(a, t);
    let b = clear[4].into();
    dist[0] += block_distance(block_rotate::<1>(a, b), t);
    dist[0] += block_distance(block_rotate::<2>(a, b), t);
    dist[0] += block_distance(block_rotate::<3>(a, b), t);
    dist[0] += block_distance(block_rotate::<4>(a, b), t);
    dist[0] += block_distance(block_rotate::<5>(a, b), t);
    dist[0] += block_distance(block_rotate::<6>(a, b), t);
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
        let queries: Box<[Template]> = (0..1).map(|_| rng.gen()).collect();

        // Generate 1000 reference templates (database)
        let db: Box<[SecretTemplate]> = (0..1000).map(|_| rng.gen()).collect();

        // Preprocess
        let queries: Box<[ClearTemplate]> = queries.iter().map(|t| t.into()).collect();
        let db: Box<[CipherTemplate]> = db.iter().map(|t| t.into()).collect();

        c.bench_function("Bench distance4 (1x31x1000)", |b| {
            b.iter(|| {
                for reference in &*db {
                    let _ = black_box(distances(&queries, &reference));
                }
            })
        });
    }
}
