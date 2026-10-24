use rand::rand_core::Rng;

/// Uniform integer in `[lo, hi]` without modulo bias.
pub fn uniform_u64<R: Rng + ?Sized>(rng: &mut R, lo: u64, hi: u64) -> u64 {
    assert!(lo <= hi, "empty range");
    let span = hi - lo;
    if span == u64::MAX {
        return rng.next_u64();
    }
    let n = span + 1;
    let zone = u64::MAX - (u64::MAX % n) - 1;
    loop {
        let x = rng.next_u64();
        if x <= zone {
            return lo + x % n;
        }
    }
}

/// Uniform float in `[0, 1)` with 53 bits of precision.
pub fn unit_f64<R: Rng + ?Sized>(rng: &mut R) -> f64 {
    (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
pub(crate) mod test_rng {
    use core::convert::Infallible;
    use rand::rand_core::TryRng;

    /// Deterministic SplitMix64, tests only.
    pub struct SplitMix(pub u64);

    impl TryRng for SplitMix {
        type Error = Infallible;
        fn try_next_u32(&mut self) -> Result<u32, Infallible> {
            Ok((self.try_next_u64()? >> 32) as u32)
        }
        fn try_next_u64(&mut self) -> Result<u64, Infallible> {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            Ok(z ^ (z >> 31))
        }
        fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Infallible> {
            for chunk in dst.chunks_mut(8) {
                let v = self.try_next_u64()?.to_le_bytes();
                chunk.copy_from_slice(&v[..chunk.len()]);
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{test_rng::SplitMix, *};

    #[test]
    fn uniform_stays_in_range() {
        let mut r = SplitMix(1);
        for _ in 0..10_000 {
            let v = uniform_u64(&mut r, 5, 9);
            assert!((5..=9).contains(&v));
        }
        assert_eq!(uniform_u64(&mut r, 7, 7), 7);
    }

    #[test]
    fn unit_is_half_open() {
        let mut r = SplitMix(2);
        for _ in 0..10_000 {
            let v = unit_f64(&mut r);
            assert!((0.0..1.0).contains(&v));
        }
    }
}
