//! Amount attack on shredded epochs. The attacker holds the treasury seed, so it sees every
//! canonical note the desk swept in, with heights. It groups them into sweeps and asks, for
//! each sweep: which subsets of the exchange's unlinked withdrawals could sum to it?
//!
//! Per sweep k with total S_k: donations X minus desk fees equal S_k plus an unknown carry out
//! minus an unknown carry in, both inside the public carry band. A candidate is pinned if it
//! sits in every consistent subset, excluded if in none, ambiguous otherwise.

use serde::Serialize;

use crate::records::Withdrawal;

#[derive(Clone, Copy)]
pub struct Band {
    pub carry_min: u64,
    pub carry_max: u64,
    pub fee: u64,
    /// Treasury notes further apart than this many blocks belong to different sweeps.
    pub sweep_gap_blocks: u32,
}

impl Default for Band {
    /// Public parameters of the demo desk (spec 18 §5.3).
    fn default() -> Self {
        Self {
            carry_min: 500_000,
            carry_max: 3_500_000,
            fee: 10_000,
            sweep_gap_blocks: 40,
        }
    }
}

#[derive(Serialize, Default)]
pub struct SweepWindow {
    pub sweep_height: u32,
    pub sweep_total_zats: u64,
    pub candidates: usize,
    pub consistent_subsets: u64,
    pub pinned: Vec<String>,
    pub excluded: usize,
    pub ambiguous: usize,
    pub enumerated: bool,
}

#[derive(Serialize, Default)]
pub struct AmountReport {
    pub sweeps: usize,
    pub candidates: usize,
    pub pinned: usize,
    pub excluded: usize,
    pub ambiguous: usize,
    pub windows: Vec<SweepWindow>,
    pub model: String,
}

/// Largest candidate set enumerated exactly (2^22 subsets); bigger sets report ambiguous.
const MAX_ENUM: usize = 22;

pub fn attack(treasury: &[(u64, u32)], unlinked: &[&Withdrawal], band: Band) -> AmountReport {
    let mut notes: Vec<(u64, u32)> = treasury.to_vec();
    notes.sort_by_key(|n| n.1);
    let mut sweeps: Vec<(u32, u64)> = vec![];
    for (v, h) in notes {
        match sweeps.last_mut() {
            Some((last_h, total)) if h.saturating_sub(*last_h) <= band.sweep_gap_blocks => {
                *last_h = h;
                *total += v;
            }
            _ => sweeps.push((h, v)),
        }
    }
    let mut rep = AmountReport {
        model: format!(
            "carry band [{}, {}] zats, fee {} per 2-action tx, sweeps split at gaps over {} blocks; \
             candidates for a sweep are unlinked withdrawals mined after the previous sweep; \
             assumes every donation came through this exchange (worst case for the desk)",
            band.carry_min, band.carry_max, band.fee, band.sweep_gap_blocks
        ),
        sweeps: sweeps.len(),
        ..Default::default()
    };
    let mut prev = 0u32;
    for (h, total) in sweeps {
        let cands: Vec<&&Withdrawal> = unlinked
            .iter()
            .filter(|w| w.height.is_some_and(|wh| wh > prev && wh < h))
            .collect();
        prev = h;
        rep.windows.push(classify(h, total, &cands, band));
    }
    for w in &rep.windows {
        rep.candidates += w.candidates;
        rep.pinned += w.pinned.len();
        rep.excluded += w.excluded;
        rep.ambiguous += w.ambiguous;
    }
    rep
}

fn classify(h: u32, total: u64, cands: &[&&Withdrawal], band: Band) -> SweepWindow {
    let n = cands.len();
    let mut win = SweepWindow {
        sweep_height: h,
        sweep_total_zats: total,
        candidates: n,
        ..Default::default()
    };
    if n > MAX_ENUM {
        win.ambiguous = n;
        return win;
    }
    win.enumerated = true;
    // Net of fees, donations equal the sweep plus carry_out minus carry_in. Each donation costs
    // at most a forward and a merge; emits and the carry add up to 5 more txs.
    let slack = band.carry_max - band.carry_min;
    let lo = total.saturating_sub(slack) as i128;
    let hi = (total + slack) as i128;
    let mut in_all = u32::MAX;
    let mut in_any = 0u32;
    let mut consistent = 0u64;
    // The empty set counts: a sweep inside the carry band can be pure carry from earlier epochs.
    for mask in 0u32..(1u32 << n) {
        let (mut sum, k) = (0i128, mask.count_ones() as i128);
        for (i, w) in cands.iter().enumerate() {
            if mask & (1 << i) != 0 {
                sum += w.value_zats as i128;
            }
        }
        let fee = band.fee as i128;
        let (net_hi, net_lo) = (sum - k * fee, sum - (2 * k + 5) * fee);
        if net_hi >= lo && net_lo <= hi {
            consistent += 1;
            in_all &= mask;
            in_any |= mask;
        }
    }
    win.consistent_subsets = consistent;
    for (i, w) in cands.iter().enumerate() {
        let bit = 1u32 << i;
        if consistent > 0 && in_all & bit != 0 {
            win.pinned.push(w.client_id.clone());
        } else if in_any & bit == 0 {
            win.excluded += 1;
        } else {
            win.ambiguous += 1;
        }
    }
    win
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(id: &str, v: u64, h: u32) -> Withdrawal {
        Withdrawal {
            client_id: id.into(),
            recipient_address: String::new(),
            value_zats: v,
            txid: String::new(),
            kind: String::new(),
            height: Some(h),
        }
    }

    #[test]
    fn lone_candidate_matching_sweep_is_pinned() {
        let a = w("a", 20_000_000, 10);
        let r = attack(&[(20_000_000, 100)], &[&a], Band::default());
        assert_eq!(r.pinned, 1);
    }

    #[test]
    fn many_small_candidates_are_ambiguous() {
        let ws: Vec<Withdrawal> = (0..10)
            .map(|i| w(&format!("c{i}"), 300_000 + i * 50_000, 10))
            .collect();
        let refs: Vec<&Withdrawal> = ws.iter().collect();
        let r = attack(&[(1_000_000, 100), (200_000, 101)], &refs, Band::default());
        assert_eq!(r.pinned, 0);
        assert_eq!(r.ambiguous, 10);
    }

    #[test]
    fn sweep_inside_carry_band_pins_nobody() {
        // A 300k sweep is explainable by carry alone, so a lone decoy must not be accused.
        let decoy = w("decoy", 304_000, 10);
        let r = attack(&[(300_000, 100)], &[&decoy], Band::default());
        assert_eq!(r.pinned, 0);
        assert_eq!(r.ambiguous, 1);
    }

    #[test]
    fn candidate_too_large_for_sweep_is_excluded() {
        let big = w("big", 50_000_000, 10);
        let small = w("s", 500_000, 11);
        let r = attack(&[(1_000_000, 100)], &[&big, &small], Band::default());
        assert_eq!(r.excluded, 1);
    }
}
