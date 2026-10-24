//! Sweep planning and op selection. Every op the desk builds spends at most 2 notes and
//! creates at most 2 outputs, so its Orchard-family bundle is exactly 2 actions.

use rand::rand_core::Rng;

use crate::{
    denom::decompose,
    params::Params,
    rng::{uniform_u64, unit_f64},
    FEE_2_ACTIONS as FEE,
};

/// Fixed at SWEEPING entry: canonical notes owed to the treasury, largest first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SweepPlan {
    pub denoms: Vec<u64>,
    /// Lower bound of what the carry will move into the open pool (fee slack adds to it).
    pub carry_floor: u64,
}

/// Notes below one fee are never spent: they would cost more than they move.
pub fn spendable(notes: &[u64]) -> Vec<u64> {
    notes.iter().copied().filter(|&v| v > FEE).collect()
}

pub fn plan_sweep<R: Rng + ?Sized>(notes: &[u64], p: &Params, rng: &mut R) -> SweepPlan {
    let live = spendable(notes);
    let n = live.len() as u64;
    let total: u64 = live.iter().sum();
    if n == 0 || total < p.carry_min + 2 * FEE {
        return SweepPlan {
            denoms: vec![],
            carry_floor: total.saturating_sub(FEE),
        };
    }
    let carry = uniform_u64(
        rng,
        p.carry_min,
        p.carry_max.min(total - FEE).max(p.carry_min),
    );
    // Budget fees for the worst case: every merge, every emit as its own tx, one carry.
    let ops = (n - 1) + p.max_canonical as u64 + 1;
    let target = total.saturating_sub(carry + ops * FEE);
    let (denoms, _) = decompose(target, p.d_min, p.max_canonical);
    let carry_floor = total - denoms.iter().sum::<u64>() - ops * FEE;
    SweepPlan {
        denoms,
        carry_floor,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    /// Two pool notes into one pool note.
    Merge {
        inputs: Vec<usize>,
        output: u64,
    },
    /// One canonical note to the treasury, remainder (if any) back to the pool.
    Emit {
        inputs: Vec<usize>,
        denom: u64,
        change: u64,
    },
    /// The last pool value into the currently open pool, as 1 or 2 notes.
    Carry {
        inputs: Vec<usize>,
        outputs: Vec<u64>,
    },
    /// The pending denomination cannot be paid from what is left; fold it into the carry.
    DropDenom,
    Done,
}

/// Indices of `notes`, largest value first.
fn by_value_desc(notes: &[u64]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..notes.len()).filter(|&i| notes[i] > FEE).collect();
    idx.sort_by(|&a, &b| notes[b].cmp(&notes[a]).then(a.cmp(&b)));
    idx
}

/// Merge used while an epoch is OPEN or CLOSED: the two smallest spendable notes.
pub fn merge_op(notes: &[u64]) -> Option<Op> {
    let idx = by_value_desc(notes);
    if idx.len() < 2 {
        return None;
    }
    let a = idx[idx.len() - 1];
    let b = idx[idx.len() - 2];
    Some(Op::Merge {
        inputs: vec![b, a],
        output: notes[a] + notes[b] - FEE,
    })
}

/// Next SWEEPING op given the pool's spendable notes and the denominations still owed.
pub fn next_sweep_op<R: Rng + ?Sized>(notes: &[u64], pending: &[u64], rng: &mut R) -> Op {
    let idx = by_value_desc(notes);
    if idx.is_empty() {
        return Op::Done;
    }
    if let Some(&d) = pending.first() {
        let take: Vec<usize> = idx.iter().take(2).copied().collect();
        let sum: u64 = take.iter().map(|&i| notes[i]).sum();
        if sum >= d + FEE {
            let change = sum - d - FEE;
            // Change between 0 and one fee would be stranded dust; merge first instead.
            if change == 0 || change > FEE || idx.len() < 3 {
                return Op::Emit {
                    inputs: take,
                    denom: d,
                    change,
                };
            }
        }
        if idx.len() >= 3 {
            let inputs = vec![idx[0], idx[1]];
            return Op::Merge {
                output: notes[idx[0]] + notes[idx[1]] - FEE,
                inputs,
            };
        }
        return Op::DropDenom;
    }
    if idx.len() >= 3 {
        return merge_op(notes).expect("3 or more notes");
    }
    let inputs: Vec<usize> = idx.clone();
    let value = inputs.iter().map(|&i| notes[i]).sum::<u64>() - FEE;
    let outputs = if value >= 4 * FEE {
        let share = 0.3 + 0.4 * unit_f64(rng);
        let a = ((value as f64) * share) as u64;
        vec![a, value - a]
    } else {
        vec![value]
    };
    Op::Carry { inputs, outputs }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{denom::is_canonical, rng::test_rng::SplitMix};

    /// Runs a plan to completion and returns (treasury notes, carried value, tx count).
    fn simulate(mut notes: Vec<u64>, seed: u64) -> (Vec<u64>, u64, usize) {
        let p = Params::demo();
        let mut rng = SplitMix(seed);
        let plan = plan_sweep(&notes, &p, &mut rng);
        let mut pending = plan.denoms.clone();
        let (mut treasury, mut carried, mut txs) = (vec![], 0u64, 0usize);
        loop {
            let op = next_sweep_op(&notes, &pending, &mut rng);
            let take = |notes: &mut Vec<u64>, inputs: &[usize]| {
                let mut sorted = inputs.to_vec();
                sorted.sort_unstable_by(|a, b| b.cmp(a));
                sorted.into_iter().map(|i| notes.remove(i)).sum::<u64>()
            };
            match op {
                Op::Merge { inputs, output } => {
                    assert_eq!(inputs.len(), 2);
                    assert_eq!(take(&mut notes, &inputs), output + FEE);
                    notes.push(output);
                    txs += 1;
                }
                Op::Emit {
                    inputs,
                    denom,
                    change,
                } => {
                    assert!(inputs.len() <= 2);
                    assert_eq!(take(&mut notes, &inputs), denom + change + FEE);
                    treasury.push(denom);
                    pending.remove(0);
                    if change > 0 {
                        notes.push(change);
                    }
                    txs += 1;
                }
                Op::Carry { inputs, outputs } => {
                    assert!(inputs.len() <= 2 && outputs.len() <= 2);
                    assert_eq!(take(&mut notes, &inputs), outputs.iter().sum::<u64>() + FEE);
                    carried += outputs.iter().sum::<u64>();
                    txs += 1;
                }
                Op::DropDenom => {
                    pending.remove(0);
                }
                Op::Done => break,
            }
            assert!(txs < 100, "plan did not terminate");
        }
        (treasury, carried, txs)
    }

    #[test]
    fn single_large_note() {
        let (t, c, _) = simulate(vec![20_000_000], 7);
        assert!(!t.is_empty());
        assert!(t.iter().all(|&d| is_canonical(d)));
        assert!(c >= Params::demo().carry_min - 6 * FEE);
    }

    #[test]
    fn small_pool_is_all_carry() {
        let (t, c, txs) = simulate(vec![300_000, 200_000], 3);
        assert!(t.is_empty());
        assert_eq!(c, 500_000 - FEE);
        assert_eq!(txs, 1);
    }

    /// Pools of dust and near-boundary values, where an unchecked subtraction would underflow.
    /// Release tests run with overflow checks on, so any wrap panics here.
    #[test]
    fn edge_pools_never_underflow_and_strand_at_most_dust() {
        let p = Params::demo();
        let edges = [FEE + 1, 2 * FEE, 3 * FEE + 1, p.carry_min, p.carry_min + 2 * FEE, p.carry_max, p.d_min + FEE];
        for seed in 0..600u64 {
            let mut rng = SplitMix(seed * 17 + 5);
            let k = 1 + (seed % 9) as usize;
            let notes: Vec<u64> = (0..k)
                .map(|_| {
                    let e = edges[uniform_u64(&mut rng, 0, edges.len() as u64 - 1) as usize];
                    e + uniform_u64(&mut rng, 0, 2)
                })
                .collect();
            let total: u64 = notes.iter().sum();
            let (t, c, txs) = simulate(notes, seed);
            let moved = t.iter().sum::<u64>() + c + txs as u64 * FEE;
            assert!(moved <= total, "seed {seed}: created value");
            assert!(total - moved <= k as u64 * FEE, "seed {seed}: stranded more than dust");
            assert!(t.iter().all(|&d| is_canonical(d) && d >= p.d_min), "seed {seed}");
        }
    }

    #[test]
    fn conservation_and_bounds_over_many_pools() {
        let p = Params::demo();
        for seed in 0..400u64 {
            let mut rng = SplitMix(seed * 31 + 1);
            let k = 1 + (seed % 12) as usize;
            let notes: Vec<u64> = (0..k)
                .map(|_| uniform_u64(&mut rng, 100_000, 1_000_000) - FEE)
                .collect();
            let total: u64 = notes.iter().sum();
            let (t, c, txs) = simulate(notes, seed);
            let fees = txs as u64 * FEE;
            assert_eq!(t.iter().sum::<u64>() + c + fees, total, "seed {seed}");
            assert!(t.len() <= p.max_canonical);
            assert!(t.iter().all(|&d| is_canonical(d) && d >= p.d_min));
        }
    }

    #[test]
    fn merge_takes_two_smallest() {
        assert_eq!(
            merge_op(&[500_000, 100_000, 300_000]),
            Some(Op::Merge {
                inputs: vec![2, 1],
                output: 390_000
            })
        );
        assert_eq!(merge_op(&[500_000, 5_000]), None);
    }
}
