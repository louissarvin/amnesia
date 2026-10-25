//! One epoch unit: wallet DB + keyring + schedule in one directory, destroyed as a whole.

use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use amnesia_core::{clock, params::Params, FEE_2_ACTIONS as FEE};
use anyhow::{Context, Result};
use rand::{
    rand_core::{Rng as _, UnwrapErr},
    rngs::SysRng,
};
use serde::{Deserialize, Serialize};
use zcash_client_backend::data_api::AccountBirthday;
use zcash_protocol::{consensus::BlockHeight, TxId};

use crate::wallet::{self, Db, Key};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Open,
    Closed,
    Final,
    SweepWait,
    Sweeping,
    Burying,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Issue {
    /// Index into the keyring (1..=cap).
    pub idx: u32,
    /// Forward slot drawn at issuance, independent of any payment (ZIP 315).
    pub slot: u64,
    pub slot_done: bool,
    /// Final forced forward after the deadline.
    pub final_slot: u64,
}

/// Schedule and progress. Lives in the epoch directory and dies with it.
#[derive(Serialize, Deserialize)]
pub struct State {
    pub id: String,
    pub opened_at: u64,
    pub phase: Phase,
    pub next_issue: u32,
    pub issues: Vec<Issue>,
    pub sweep_at: Option<u64>,
    pub pending_denoms: Option<Vec<u64>>,
    pub last_tx_height: Option<u32>,
    /// Treasury emits broadcast but not yet mined. Persisted so a restart cannot emit a
    /// denomination twice; the denom leaves `pending_denoms` only once its tx is mined.
    #[serde(default)]
    pub pending_emits: Vec<PendingEmit>,
    /// Mirror of `Epoch::inflight`, so a restart neither reuses notes spent by a tx still in
    /// the mempool nor shreds before that tx is buried.
    #[serde(default)]
    pub inflight: Vec<SavedInFlight>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SavedInFlight {
    pub txid: String,
    pub expiry: u32,
    /// Spent notes as (receiving txid hex, output index): note row ids are not stable to store.
    pub notes: Vec<(String, u16)>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PendingEmit {
    /// Raw txid bytes, hex.
    pub txid: String,
    pub expiry: u32,
    pub denom: u64,
}

pub struct InFlight {
    pub txid: TxId,
    pub expiry: BlockHeight,
    pub notes: Vec<NoteRef>,
}

/// A note by the tx that created it and its output index.
pub type NoteRef = (TxId, u16);

pub fn note_ref(n: &wallet::Note) -> NoteRef {
    (*n.txid(), n.output_index())
}

fn txid_hex(t: &TxId) -> String {
    let b: &[u8; 32] = t.as_ref();
    hex::encode(b)
}

pub fn txid_from_hex(s: &str) -> Result<TxId> {
    let mut b = [0u8; 32];
    hex::decode_to_slice(s, &mut b)?;
    Ok(TxId::from_bytes(b))
}

pub struct Epoch {
    pub dir: PathBuf,
    pub db: Db,
    pub keys: Vec<Key>,
    pub st: State,
    pub inflight: Vec<InFlight>,
}

impl Epoch {
    pub fn ends_at(&self, p: &Params) -> u64 {
        self.st.opened_at + p.epoch_s
    }

    pub fn deadline(&self, p: &Params) -> u64 {
        self.st.opened_at + p.epoch_s + p.grace_s
    }

    pub fn pool(&self) -> &Key {
        &self.keys[0]
    }

    pub fn create(
        root: &Path,
        opened_at: u64,
        birthday: &AccountBirthday,
        p: &Params,
    ) -> Result<Self> {
        let mut id = [0u8; 16];
        UnwrapErr(SysRng).fill_bytes(&mut id);
        let id = hex::encode(id);
        let dir = root.join(&id);
        fs::create_dir(&dir)?;
        set_private(&dir)?;
        let (db, keys) = wallet::create(&dir, birthday, p.cap + 1)?;
        let st = State {
            id,
            opened_at,
            phase: Phase::Open,
            next_issue: 1,
            issues: vec![],
            sweep_at: None,
            pending_denoms: None,
            last_tx_height: None,
            pending_emits: vec![],
            inflight: vec![],
        };
        let e = Self {
            dir,
            db,
            keys,
            st,
            inflight: vec![],
        };
        e.save()?;
        Ok(e)
    }

    pub fn load(dir: &Path) -> Result<Self> {
        let st: State =
            serde_json::from_slice(&fs::read(dir.join("state.json"))?).context("epoch state")?;
        let inflight = st
            .inflight
            .iter()
            .map(|f| {
                Ok(InFlight {
                    txid: txid_from_hex(&f.txid)?,
                    expiry: BlockHeight::from_u32(f.expiry),
                    notes: f.notes.iter().map(|(t, i)| Ok((txid_from_hex(t)?, *i))).collect::<Result<_>>()?,
                })
            })
            .collect::<Result<_>>()?;
        Ok(Self {
            dir: dir.to_path_buf(),
            db: wallet::open_db(dir)?,
            keys: wallet::load_keyring(dir)?,
            st,
            inflight,
        })
    }

    pub fn save(&self) -> Result<()> {
        let tmp = self.dir.join("state.json.tmp");
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&serde_json::to_vec(&self.st)?)?;
        f.sync_all()?;
        fs::rename(tmp, self.dir.join("state.json"))?;
        Ok(())
    }

    /// Hands out the next pre-generated address. Slots are drawn now, before any payment.
    pub fn issue(&mut self, now: u64, p: &Params) -> Result<Option<(String, u64)>> {
        if self.st.phase != Phase::Open || now >= self.ends_at(p) || self.st.next_issue > p.cap {
            return Ok(None);
        }
        let idx = self.st.next_issue;
        let d = self.deadline(p);
        let rng = &mut UnwrapErr(SysRng);
        let lead = (p.grace_s / 4).min(30 * 60);
        let issue = Issue {
            idx,
            slot: clock::slot(rng, now + lead, d),
            slot_done: false,
            final_slot: clock::slot(rng, d, d + lead),
        };
        self.st.next_issue += 1;
        self.st.issues.push(issue);
        self.save()?;
        Ok(Some((self.keys[idx as usize].address.clone(), d)))
    }

    /// A tx of this epoch may still land: no new sweep op and no shred until it settles.
    pub fn busy(&self) -> bool {
        !self.inflight.is_empty() || !self.st.pending_emits.is_empty()
    }

    /// Spendable notes of `account` that no in-flight tx of this epoch spends.
    pub fn free_notes(&self, account: zcash_client_sqlite::AccountUuid) -> Result<Vec<wallet::Note>> {
        let busy: Vec<NoteRef> = self.inflight.iter().flat_map(|f| f.notes.iter().copied()).collect();
        Ok(wallet::spendable_notes(&self.db, account, &[])?
            .into_iter()
            .filter(|n| !busy.contains(&note_ref(n)))
            .collect())
    }

    /// Spendable notes of the pool, values above one fee only.
    pub fn pool_notes(&self) -> Result<Vec<wallet::Note>> {
        Ok(self
            .free_notes(self.pool().uuid)?
            .into_iter()
            .filter(|n| n.note().value().inner() > FEE)
            .collect())
    }

    pub fn save_inflight(&mut self) -> Result<()> {
        self.st.inflight = self
            .inflight
            .iter()
            .map(|f| SavedInFlight {
                txid: txid_hex(&f.txid),
                expiry: u32::from(f.expiry),
                notes: f.notes.iter().map(|(t, i)| (txid_hex(t), *i)).collect(),
            })
            .collect();
        self.save()
    }

    /// Overwrite every file with random bytes, fsync, unlink, remove the directory. The caller
    /// drops `self` right after, which zeroizes the seeds.
    pub fn shred(self) -> Result<()> {
        let Epoch {
            dir,
            db,
            keys,
            st,
            inflight,
        } = self;
        drop(db);
        drop(inflight);
        drop(st);
        overwrite_tree(&dir)?;
        fs::remove_dir_all(&dir)?;
        drop(keys);
        Ok(())
    }
}

fn overwrite_tree(dir: &Path) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let meta = fs::symlink_metadata(&path)?;
        if meta.is_dir() {
            overwrite_tree(&path)?;
        } else if meta.is_file() {
            let len = meta.len() as usize;
            let mut buf = vec![0u8; len.min(1 << 20)];
            let mut f = fs::OpenOptions::new().write(true).open(&path)?;
            let mut left = len;
            while left > 0 {
                let n = left.min(buf.len());
                UnwrapErr(SysRng).fill_bytes(&mut buf[..n]);
                f.write_all(&buf[..n])?;
                left -= n;
            }
            f.sync_all()?;
            drop(f);
            fs::remove_file(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn set_private(dir: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    Ok(())
}
