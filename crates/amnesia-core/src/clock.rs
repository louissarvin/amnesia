use rand::rand_core::Rng;

use crate::rng::{uniform_u64, unit_f64};

/// Exponential gap in seconds for a Poisson clock with `rate_per_h` events per hour.
pub fn exp_gap_s<R: Rng + ?Sized>(rng: &mut R, rate_per_h: u32) -> u64 {
    let u = 1.0 - unit_f64(rng); // (0, 1]
    let mean_s = 3600.0 / f64::from(rate_per_h.max(1));
    (-u.ln() * mean_s).round() as u64
}

/// A time drawn uniformly in `[lo, hi]` (Unix seconds). Collapses to `lo` if the range is empty.
pub fn slot<R: Rng + ?Sized>(rng: &mut R, lo: u64, hi: u64) -> u64 {
    if hi <= lo {
        lo
    } else {
        uniform_u64(rng, lo, hi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::test_rng::SplitMix;

    #[test]
    fn exp_mean_close_to_rate() {
        let mut r = SplitMix(9);
        let n = 20_000;
        let sum: u64 = (0..n).map(|_| exp_gap_s(&mut r, 12)).sum();
        let mean = sum as f64 / n as f64;
        assert!((mean - 300.0).abs() < 15.0, "mean {mean}");
    }

    #[test]
    fn slot_bounds() {
        let mut r = SplitMix(4);
        for _ in 0..1000 {
            let s = slot(&mut r, 100, 200);
            assert!((100..=200).contains(&s));
        }
        assert_eq!(slot(&mut r, 50, 10), 50);
    }
}
