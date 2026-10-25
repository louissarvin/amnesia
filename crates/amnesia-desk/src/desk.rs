//! The desk loop: open epochs, scan, forward at slots, merge/emit/carry on a Poisson clock,
//! bury, shred. Payments never trigger anything directly.

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use amnesia_core::{
    FEE_2_ACTIONS as FEE, buckets, clock,
    params::Params,
    plan::{self, Op},
    rng::uniform_u64,
};
use anyhow::{Result, anyhow};
use rand::{rand_core::UnwrapErr, rngs::SysRng};
use zcash_protocol::{TxId, consensus::BlockHeight};

use crate::{
    chain::{self, Chain, Client},
    epoch::{Epoch, InFlight, NoteRef, PendingEmit, Phase, note_ref, txid_from_hex},
    ledger::{self, Ledger, ShredLine},
    log,
    wallet::{self, Key, Note},
};

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub struct Desk {
    pub p: Params,
    pub epochs_dir: PathBuf,
    pub ledger: Ledger,
    pub chain: Chain,
    treasury: orchard::Address,
    pub epochs: Vec<Epoch>,
    next_tick_at: u64,
    pub tip: Option<BlockHeight>,
    pub synced: bool,
    pub swept_since_boot: u64,
    outbox: Vec<Outgoing>,
    pub profile: String,
}

/// A built transaction waiting to be broadcast by the caller, outside the desk lock: a Tor
/// broadcast can take minutes and must not block donor intake.
pub struct Outgoing {
    epoch_id: String,
    pub txid: TxId,
    pub raw: Vec<u8>,
}

/// What an op spends and where its outputs go, before it is built.
struct Planned<'a> {
    spends: Vec<(&'a Key, &'a Note)>,
    outputs: Vec<(orchard::Address, u64)>,
    emitted: u64,
}

impl Desk {
    pub fn new(p: Params, profile: &str, epochs_dir: PathBuf, ledger: Ledger, chain: Chain, treasury_ua: &str) -> Result<Self> {
        let treasury = wallet::orchard_receiver(treasury_ua)?;
        fs::create_dir_all(&epochs_dir)?;
        let mut epochs = vec![];
        for entry in fs::read_dir(&epochs_dir)? {
            let dir = entry?.path();
            if dir.is_dir() {
                match Epoch::load(&dir) {
                    Ok(e) => epochs.push(e),
                    Err(_) => log::fatal("epoch_load"),
                }
            }
        }
        epochs.sort_by_key(|e| e.st.opened_at);
        let live: Vec<String> = epochs.iter().map(|e| e.st.id.clone()).collect();
        for _ in 0..ledger.record_lost(&live, ledger::hour(now()), profile)? {
            log::event("epoch_lost", &[]);
        }
        let next_tick_at = now() + clock::exp_gap_s(&mut UnwrapErr(SysRng), p.merge_rate_per_h);
        Ok(Self {
            p,
            epochs_dir,
            ledger,
            chain,
            treasury,
            epochs,
            next_tick_at,
            tip: None,
            synced: false,
            swept_since_boot: 0,
            outbox: vec![],
            profile: profile.to_string(),
        })
    }

    /// The epoch currently issuing addresses.
    pub fn open_epoch_mut(&mut self, t: u64) -> Option<&mut Epoch> {
        let p = self.p.clone();
        self.epochs
            .iter_mut()
            .find(|e| e.st.phase == Phase::Open && e.st.opened_at <= t && t < e.ends_at(&p))
    }

    /// Epoch dirs to scan before the next tick. The caller scans them without the desk lock,
    /// on separate SQLite connections: intake never touches the wallet DBs, so donors do not
    /// wait on the network.
    pub fn epoch_dirs(&self) -> Vec<PathBuf> {
        self.epochs.iter().map(|e| e.dir.clone()).collect()
    }

    /// Local work after `sync_epochs` reached `tip`. Only epoch creation touches the network.
    pub async fn tick(&mut self, client: &mut Client, tip: BlockHeight) -> Result<()> {
        let t = now();
        self.tip = Some(tip);
        self.synced = true;
        self.ensure_epochs(client, t, tip).await?;
        for e in &mut self.epochs {
            settle_inflight(e, tip)?;
        }
        self.settle_emits(tip)?;
        // Stages are independent: a failed build must not hold back phase changes or shredding.
        let phases = self.advance_phases(t);
        let forwards = self.run_forwards(t);
        let mut op = Ok(());
        if t >= self.next_tick_at {
            self.next_tick_at = t + clock::exp_gap_s(&mut UnwrapErr(SysRng), self.p.merge_rate_per_h);
            op = self.run_clock_op();
        }
        let shred = self.shred_buried(tip);
        phases.and(forwards).and(op).and(shred)
    }

    /// Keeps one epoch open and the next one created and synced 10 minutes before it opens.
    async fn ensure_epochs(&mut self, client: &mut Client, t: u64, tip: BlockHeight) -> Result<()> {
        let latest_open = self.epochs.iter().filter(|e| e.st.phase == Phase::Open).map(|e| e.st.opened_at).max();
        let opened_at = match latest_open {
            None => t,
            Some(o) if o + self.p.epoch_s <= t + 600 => (o + self.p.epoch_s).max(t.saturating_sub(60)),
            Some(_) => return Ok(()),
        };
        // Birthday rounded down to a 1,000-block boundary, so the request does not pin our start.
        let bday_h = BlockHeight::from_u32(u32::from(tip) / 1000 * 1000);
        let ts = chain::treestate(client, bday_h - 1).await?;
        let birthday = wallet::birthday_from(ts)?;
        let mut e = Epoch::create(&self.epochs_dir, opened_at, &birthday, &self.p)?;
        chain::put_roots(client, &mut e.db).await?;
        self.ledger.append_open(&e.st.id, ledger::hour(e.st.opened_at))?;
        log::event("epoch_open", &[("epoch_id", &e.st.id)]);
        self.epochs.push(e);
        Ok(())
    }

    fn advance_phases(&mut self, t: u64) -> Result<()> {
        let p = self.p.clone();
        for e in &mut self.epochs {
            let before = e.st.phase;
            match e.st.phase {
                Phase::Open if t >= e.ends_at(&p) => {
                    e.st.phase = Phase::Closed;
                    log::event("epoch_close", &[("epoch_id", &e.st.id)]);
                }
                Phase::Closed if t >= e.deadline(&p) => e.st.phase = Phase::Final,
                Phase::Final => {
                    let last_final = e.st.issues.iter().map(|i| i.final_slot).max().unwrap_or(0);
                    let give_up = t >= e.deadline(&p) + p.grace_s;
                    if (t >= last_final && receive_drained(e)? && e.inflight.is_empty()) || give_up {
                        let w = uniform_u64(&mut UnwrapErr(SysRng), p.grace_s / 4, p.grace_s);
                        e.st.sweep_at = Some(t + w);
                        e.st.phase = Phase::SweepWait;
                    }
                }
                Phase::SweepWait if e.st.sweep_at.is_some_and(|s| t >= s) && e.inflight.is_empty() => {
                    let total = wallet::total_value(&e.db, e.pool().uuid)?;
                    let notes = e.pool_notes()?;
                    let spendable: u64 = notes.iter().map(|n| n.note().value().inner()).sum();
                    // Wait until every forward and merge into the pool has confirmed.
                    if total == spendable {
                        let values: Vec<u64> = notes.iter().map(|n| n.note().value().inner()).collect();
                        let plan = plan::plan_sweep(&values, &p, &mut UnwrapErr(SysRng));
                        e.st.pending_denoms = Some(plan.denoms);
                        e.st.phase = Phase::Sweeping;
                    }
                }
                Phase::Sweeping if !e.busy() => {
                    let total = wallet::total_value(&e.db, e.pool().uuid)?;
                    if total <= FEE {
                        e.st.phase = Phase::Burying;
                    }
                }
                _ => {}
            }
            if e.st.phase != before {
                e.save()?;
            }
        }
        Ok(())
    }

    fn run_forwards(&mut self, t: u64) -> Result<()> {
        let p = self.p.clone();
        for ei in 0..self.epochs.len() {
            let phase = self.epochs[ei].st.phase;
            if !matches!(phase, Phase::Open | Phase::Closed | Phase::Final) {
                continue;
            }
            for ii in 0..self.epochs[ei].st.issues.len() {
                let e = &mut self.epochs[ei];
                let is = e.st.issues[ii].clone();
                let due_slot = !is.slot_done && t >= is.slot;
                let due_final = phase == Phase::Final && t >= is.final_slot;
                if !due_slot && !due_final {
                    continue;
                }
                let key = &e.keys[is.idx as usize];
                let notes = e.free_notes(key.uuid)?;
                if due_slot {
                    e.st.issues[ii].slot_done = true;
                    e.save()?;
                }
                let mut notes: Vec<Note> = notes.into_iter().filter(|n| n.note().value().inner() > FEE).collect();
                log::dev(|| format!("forward check epoch={} idx={} notes={}", &e.st.id[..8], is.idx, notes.len()));
                if notes.is_empty() {
                    continue;
                }
                notes.sort_by_key(|n| std::cmp::Reverse(n.note().value().inner()));
                notes.truncate(2);
                let value: u64 = notes.iter().map(|n| n.note().value().inner()).sum::<u64>() - FEE;
                let pool = wallet::orchard_receiver(&e.pool().address)?;
                let key = &e.keys[is.idx as usize];
                let planned = Planned {
                    spends: notes.iter().map(|n| (key, n)).collect(),
                    outputs: vec![(pool, value)],
                    emitted: 0,
                };
                let ids: Vec<_> = notes.iter().map(note_ref).collect();
                let tx = build_planned(&mut e.db, planned, p.tx_expiry_delta)?;
                self.send(ei, tx, ids, 0)?;
            }
        }
        Ok(())
    }

    /// One op per clock tick, offered to epochs oldest first.
    fn run_clock_op(&mut self) -> Result<()> {
        let carry_to = self
            .epochs
            .iter()
            .filter(|e| e.st.phase == Phase::Open)
            .max_by_key(|e| e.st.opened_at)
            .map(|e| e.pool().address.clone());
        for ei in 0..self.epochs.len() {
            let e = &mut self.epochs[ei];
            if e.busy() && e.st.phase == Phase::Sweeping {
                continue;
            }
            let notes = e.pool_notes()?;
            let values: Vec<u64> = notes.iter().map(|n| n.note().value().inner()).collect();
            let pool = wallet::orchard_receiver(&e.pool().address)?;
            let op = match e.st.phase {
                Phase::Open | Phase::Closed | Phase::Final | Phase::SweepWait => match plan::merge_op(&values) {
                    Some(op) => op,
                    None => continue,
                },
                Phase::Sweeping => {
                    let pending = e.st.pending_denoms.clone().unwrap_or_default();
                    let op = plan::next_sweep_op(&values, &pending, &mut UnwrapErr(SysRng));
                    if op == Op::DropDenom {
                        if let Some(d) = e.st.pending_denoms.as_mut() {
                            d.remove(0);
                        }
                        e.save()?;
                        continue;
                    }
                    op
                }
                Phase::Burying => continue,
            };
            let pool_key = &e.keys[0];
            let pick = |idx: &[usize]| idx.iter().map(|&i| (pool_key, &notes[i])).collect::<Vec<_>>();
            let planned = match &op {
                Op::Merge { inputs, output } => Planned {
                    spends: pick(inputs),
                    outputs: vec![(pool, *output)],
                    emitted: 0,
                },
                Op::Emit { inputs, denom, change } => {
                    let mut outputs = vec![(self.treasury, *denom)];
                    if *change > 0 {
                        outputs.push((pool, *change));
                    }
                    Planned { spends: pick(inputs), outputs, emitted: *denom }
                }
                Op::Carry { inputs, outputs } => {
                    let Some(to) = carry_to.as_deref().filter(|a| *a != pool_key.address) else {
                        continue;
                    };
                    let to = wallet::orchard_receiver(to)?;
                    Planned {
                        spends: pick(inputs),
                        outputs: outputs.iter().map(|v| (to, *v)).collect(),
                        emitted: 0,
                    }
                }
                Op::Done | Op::DropDenom => continue,
            };
            log::dev(|| format!("clock op epoch={} {:?}", &e.st.id[..8], op));
            let emitted = planned.emitted;
            let ids: Vec<_> = planned.spends.iter().map(|(_, n)| note_ref(n)).collect();
            let tx = build_planned(&mut e.db, planned, self.p.tx_expiry_delta)?;
            self.send(ei, tx, ids, emitted)?;
            return Ok(());
        }
        Ok(())
    }

    /// Queues `tx` for broadcast. Its notes count as in flight right away so no later build in
    /// this tick reuses them; they stay in flight until the tx is mined or expires, even if
    /// every broadcast attempt fails, since a failed send may still have reached the mempool.
    fn send(&mut self, ei: usize, tx: wallet::Built, notes: Vec<NoteRef>, emitted: u64) -> Result<()> {
        let e = &mut self.epochs[ei];
        if emitted > 0 {
            let txid: &[u8; 32] = tx.txid.as_ref();
            e.st.pending_emits.push(PendingEmit { txid: hex::encode(txid), expiry: u32::from(tx.expiry), denom: emitted });
            e.save()?;
        }
        e.inflight.push(InFlight { txid: tx.txid, expiry: tx.expiry, notes });
        e.save_inflight()?;
        self.outbox.push(Outgoing { epoch_id: e.st.id.clone(), txid: tx.txid, raw: tx.raw });
        Ok(())
    }

    pub fn take_outbox(&mut self) -> Vec<Outgoing> {
        std::mem::take(&mut self.outbox)
    }

    /// Applies a broadcast result. A failed broadcast may still have reached the mempool, so its
    /// notes stay in flight until expiry instead of being reused at once.
    pub fn record(&mut self, out: Outgoing, ok: bool) {
        let id = self.epochs.iter().find(|e| e.st.id == out.epoch_id).map(|e| e.st.id[..8].to_string());
        log::dev(|| format!("broadcast epoch={} ok={ok} txid={}", id.unwrap_or_default(), out.txid));
        if !ok {
            log::event("broadcast_fail", &[]);
        }
    }

    /// Counts an emit only once its tx is mined; an expired one leaves its denom pending.
    fn settle_emits(&mut self, tip: BlockHeight) -> Result<()> {
        for e in &mut self.epochs {
            if e.st.pending_emits.is_empty() {
                continue;
            }
            let mut keep = vec![];
            for pe in std::mem::take(&mut e.st.pending_emits) {
                match wallet::tx_height(&e.db, txid_from_hex(&pe.txid)?)? {
                    Some(_) => {
                        if let Some(d) = e.st.pending_denoms.as_mut()
                            && let Some(i) = d.iter().position(|&x| x == pe.denom)
                        {
                            d.remove(i);
                        }
                        self.ledger.add_emitted(now(), pe.denom)?;
                        self.swept_since_boot += pe.denom;
                    }
                    None if u32::from(tip) > pe.expiry => {}
                    None => keep.push(pe),
                }
            }
            e.st.pending_emits = keep;
            e.save()?;
        }
        Ok(())
    }

    fn shred_buried(&mut self, tip: BlockHeight) -> Result<()> {
        let burial = self.p.burial_blocks;
        let mut i = 0;
        while i < self.epochs.len() {
            let e = &self.epochs[i];
            let deep = match e.st.last_tx_height {
                Some(h) => u32::from(tip).saturating_sub(h) >= burial,
                None => true,
            };
            if e.st.phase == Phase::Burying && !e.busy() && deep {
                let e = self.epochs.remove(i);
                let line = ShredLine {
                    epoch_id: e.st.id.clone(),
                    opened_hour: ledger::hour(e.st.opened_at),
                    closed_kind: "shredded".into(),
                    closed_hour: ledger::hour(now()),
                    issued_bucket: buckets::issued_bucket(e.st.issues.len() as u32).into(),
                    profile: Some(self.profile.clone()),
                };
                e.shred()?;
                self.ledger.append_shred(&line)?;
                log::event("epoch_shred", &[("epoch_id", &line.epoch_id)]);
            } else {
                i += 1;
            }
        }
        Ok(())
    }
}

fn build_planned(db: &mut wallet::Db, planned: Planned<'_>, delta: u32) -> Result<wallet::Built> {
    tokio::task::block_in_place(|| wallet::build(db, &planned.spends, &planned.outputs, delta))
}

/// Drops mined or expired in-flight txs and records the latest mined height for burial.
fn settle_inflight(e: &mut Epoch, tip: BlockHeight) -> Result<()> {
    let mut keep = vec![];
    let before = e.inflight.len();
    for f in std::mem::take(&mut e.inflight) {
        match wallet::tx_height(&e.db, f.txid)? {
            Some(h) => {
                let h = u32::from(h);
                e.st.last_tx_height = Some(e.st.last_tx_height.map_or(h, |x| x.max(h)));
                e.save()?;
            }
            None if tip > f.expiry => {}
            None => keep.push(f),
        }
    }
    let changed = keep.len() != before;
    e.inflight = keep;
    if changed {
        e.save_inflight()?;
    }
    Ok(())
}

/// Every issued receive key holds no more than dust (forwards confirmed or nothing paid).
fn receive_drained(e: &Epoch) -> Result<bool> {
    for is in &e.st.issues {
        if wallet::total_value(&e.db, e.keys[is.idx as usize].uuid)? > FEE {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn status_line(d: &Desk) -> Result<String> {
    let phases: Vec<String> = d.epochs.iter().map(|e| format!("{:?}", e.st.phase)).collect();
    Ok(format!(
        "tip={} epochs=[{}] shredded={}",
        d.tip.map(u32::from).unwrap_or(0),
        phases.join(","),
        d.ledger.shred_lines().map_err(|e| anyhow!("{e}"))?.len()
    ))
}
