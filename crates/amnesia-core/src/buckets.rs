//! Coarse public values: the only numbers the desk ever publishes.

/// Rounds to 2 significant figures, half up.
pub fn round_2sf(v: u64) -> u64 {
    if v < 100 {
        return v;
    }
    let mut scale = 1u64;
    while v / scale >= 100 {
        scale *= 10;
    }
    (v + scale / 2) / scale * scale
}

pub fn issued_bucket(n: u32) -> &'static str {
    match n {
        0 => "0",
        1..=9 => "1-9",
        10..=49 => "10-49",
        _ => "50+",
    }
}

/// Swept TAZ since boot, bucketed.
pub fn swept_bucket(zats: u64) -> &'static str {
    const TAZ: u64 = 100_000_000;
    match zats {
        0 => "0",
        z if z < TAZ / 10 => "lt_0.1",
        z if z < TAZ => "0.1-1",
        z if z < 10 * TAZ => "1-10",
        _ => "10+",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_sig_figs() {
        assert_eq!(round_2sf(0), 0);
        assert_eq!(round_2sf(99), 99);
        assert_eq!(round_2sf(1_234_567), 1_200_000);
        assert_eq!(round_2sf(1_250_000), 1_300_000);
        assert_eq!(round_2sf(995), 1_000);
    }

    #[test]
    fn buckets() {
        assert_eq!(issued_bucket(0), "0");
        assert_eq!(issued_bucket(9), "1-9");
        assert_eq!(issued_bucket(50), "50+");
        assert_eq!(swept_bucket(5_000_000), "lt_0.1");
    }
}
