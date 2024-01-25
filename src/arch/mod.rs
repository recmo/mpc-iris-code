mod generic; // Optimized generic implementation
mod neon; // Optimized aarch64 NEON implementation
mod reference; // Simple generic implementations

pub use generic::distances;

#[cfg(feature = "bench")]
pub mod benches {
    use super::*;
    use criterion::Criterion;

    pub fn group(c: &mut Criterion) {
        reference::benches::group(c);

        generic::benches::group(c);

        #[cfg(target_feature = "neon")]
        neon::benches::group(c);
    }
}
