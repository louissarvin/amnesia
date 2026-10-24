//! Pure Amnesia desk logic: no IO, no chain, RNG injected. Unit-tested without a network.

pub mod buckets;
pub mod clock;
pub mod denom;
pub mod params;
pub mod plan;
pub mod pow;
pub mod rng;

/// ZIP 317 fee for a transaction with 2 logical actions: 5,000 x max(2, 2).
pub const FEE_2_ACTIONS: u64 = 10_000;
