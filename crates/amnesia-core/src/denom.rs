/// Canonical values 1, 2, 5 x 10^k zatoshis, descending.
fn canonical_desc() -> impl Iterator<Item = u64> {
    (0..=10u32)
        .rev()
        .flat_map(|k| [5u64, 2, 1].map(|m| m * 10u64.pow(k)))
}

/// Greedy decomposition of `total` into at most `max_notes` canonical values, each at least
/// `d_min`. Returns the notes (descending) and the uncovered remainder.
pub fn decompose(total: u64, d_min: u64, max_notes: usize) -> (Vec<u64>, u64) {
    let mut left = total;
    let mut notes = Vec::new();
    for d in canonical_desc().filter(|&d| d >= d_min) {
        while notes.len() < max_notes && left >= d {
            notes.push(d);
            left -= d;
        }
    }
    (notes, left)
}

pub fn is_canonical(v: u64) -> bool {
    canonical_desc().any(|d| d == v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_and_remainder() {
        assert_eq!(
            decompose(8_000_000, 100_000, 4),
            (vec![5_000_000, 2_000_000, 1_000_000], 0)
        );
        let (n, r) = decompose(9_999_999, 100_000, 4);
        assert_eq!(n, vec![5_000_000, 2_000_000, 2_000_000, 500_000]);
        assert_eq!(r, 499_999);
    }

    #[test]
    fn respects_min_and_cap() {
        assert_eq!(decompose(99_999, 100_000, 4), (vec![], 99_999));
        let (n, r) = decompose(1_700_000_000, 100_000, 2);
        assert_eq!(n, vec![1_000_000_000, 500_000_000]);
        assert_eq!(r, 200_000_000);
    }

    #[test]
    fn conservation_and_shape() {
        for t in (0..50_000_000u64).step_by(77_777) {
            let (n, r) = decompose(t, 100_000, 4);
            assert_eq!(n.iter().sum::<u64>() + r, t);
            assert!(n.len() <= 4);
            assert!(n.iter().all(|&d| is_canonical(d) && d >= 100_000));
            assert!(n.windows(2).all(|w| w[0] >= w[1]));
        }
    }
}
