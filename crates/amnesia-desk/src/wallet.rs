//! Epoch wallet: one `zcash_client_sqlite` DB holding cap + 1 accounts, each from its own
//! OS-random seed. Index 0 is the pool `P_e`; 1..=cap are receive keys.

use std::{fs, io::Write as _, num::NonZeroU32, path::Path};

use anyhow::{anyhow, bail, ensure, Result};
use rand::{
    rand_core::{Rng as _, UnwrapErr},
    rngs::SysRng,
};
use transparent::builder::TransparentSigningSet;
use zcash_address::ZcashAddress;
use zcash_client_backend::{
    data_api::{
        wallet::{input_selection::LockFilter, ConfirmationsPolicy},
        Account as _, AccountBirthday, AccountPurpose, InputSource, MaxSpendMode, TargetValue,
        WalletCommitmentTrees, WalletRead, WalletWrite,
    },
    wallet::ReceivedNote,
};
use zcash_client_sqlite::{
    util::SystemClock, wallet::init::init_wallet_db, AccountUuid, ReceivedNoteId, WalletDb,
};
use zcash_keys::{
    address::Address,
    keys::{UnifiedAddressRequest, UnifiedFullViewingKey, UnifiedSpendingKey},
};
use zcash_primitives::transaction::{
    builder::{BuildConfig, Builder, BundlePadding},
    fees::zip317,
};
use zcash_proofs::prover::LocalTxProver;
use zcash_protocol::{
    consensus::{BlockHeight, Network},
    memo::MemoBytes,
    value::Zatoshis,
    ShieldedPool, TxId,
};
use zeroize::Zeroizing;

use crate::PARAMS;

pub type Db = WalletDb<rusqlite::Connection, Network, SystemClock, UnwrapErr<SysRng>>;
pub type Note = ReceivedNote<ReceivedNoteId, orchard::note::Note>;

/// One account's secret seed plus its public handles. No `Debug`: it must never be printed.
pub struct Key {
    pub uuid: AccountUuid,
    seed: Zeroizing<[u8; 32]>,
    pub address: String,
}

impl Drop for Key {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.address.zeroize();
    }
}

impl Key {
    fn usk(&self) -> Result<UnifiedSpendingKey> {
        Ok(UnifiedSpendingKey::from_seed(
            &PARAMS,
            &self.seed[..],
            zip32::AccountId::ZERO,
        )?)
    }
}

pub fn open_db(dir: &Path) -> Result<Db> {
    Ok(WalletDb::for_path(
        dir.join("data.sqlite"),
        PARAMS,
        SystemClock,
        UnwrapErr(SysRng),
    )?)
}

/// Creates the wallet and every account up front: adding an account to a synced DB rewinds the
/// whole DB to that account's birthday (`13b` §1).
pub fn create(dir: &Path, birthday: &AccountBirthday, accounts: u32) -> Result<(Db, Vec<Key>)> {
    let mut db = open_db(dir)?;
    init_wallet_db(&mut db, None).map_err(|e| anyhow!("init wallet db: {e:?}"))?;
    let mut keys = Vec::with_capacity(accounts as usize);
    for i in 0..accounts {
        let mut seed = Zeroizing::new([0u8; 32]);
        UnwrapErr(SysRng).fill_bytes(&mut seed[..]);
        keys.push(import(&mut db, birthday, &format!("a{i}"), seed)?);
    }
    write_keyring(dir, &keys)?;
    Ok((db, keys))
}

fn import(
    db: &mut Db,
    birthday: &AccountBirthday,
    label: &str,
    seed: Zeroizing<[u8; 32]>,
) -> Result<Key> {
    let usk = UnifiedSpendingKey::from_seed(&PARAMS, &seed[..], zip32::AccountId::ZERO)?;
    let ofvk = usk
        .to_unified_full_viewing_key()
        .orchard()
        .cloned()
        .ok_or(anyhow!("no orchard fvk"))?;
    drop(usk);
    let ufvk = UnifiedFullViewingKey::from_orchard_fvk(ofvk)?;
    let acct = db.import_account_ufvk(
        label,
        &ufvk,
        birthday,
        AccountPurpose::Spending { derivation: None },
        None,
    )?;
    let (ua, _) = db
        .get_next_available_address(acct.id(), UnifiedAddressRequest::ORCHARD)?
        .ok_or(anyhow!("no address"))?;
    Ok(Key {
        uuid: acct.id(),
        seed,
        address: ua.encode(&PARAMS),
    })
}

/// Keyring lives inside the epoch directory (tmpfs in production) and dies with it.
fn write_keyring(dir: &Path, keys: &[Key]) -> Result<()> {
    let mut buf = Zeroizing::new(Vec::new());
    for k in keys {
        buf.extend_from_slice(k.uuid.expose_uuid().as_bytes());
        buf.extend_from_slice(&k.seed[..]);
        buf.extend_from_slice(&(k.address.len() as u16).to_be_bytes());
        buf.extend_from_slice(k.address.as_bytes());
    }
    let mut f = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(dir.join("keyring.bin"))?;
    f.write_all(&buf)?;
    f.sync_all()?;
    Ok(())
}

pub fn load_keyring(dir: &Path) -> Result<Vec<Key>> {
    let buf = Zeroizing::new(fs::read(dir.join("keyring.bin"))?);
    let mut keys = vec![];
    let mut i = 0;
    while i < buf.len() {
        ensure!(buf.len() >= i + 50, "truncated keyring");
        let uuid =
            AccountUuid::from_uuid(uuid_from_slice(&buf[i..i + 16]).ok_or(anyhow!("bad uuid"))?);
        let mut seed = Zeroizing::new([0u8; 32]);
        seed.copy_from_slice(&buf[i + 16..i + 48]);
        let n = u16::from_be_bytes([buf[i + 48], buf[i + 49]]) as usize;
        ensure!(buf.len() >= i + 50 + n, "truncated keyring");
        let address = String::from_utf8(buf[i + 50..i + 50 + n].to_vec())?;
        keys.push(Key {
            uuid,
            seed,
            address,
        });
        i += 50 + n;
    }
    Ok(keys)
}

fn uuid_from_slice(b: &[u8]) -> Option<uuid::Uuid> {
    uuid::Uuid::from_slice(b).ok()
}

/// Spendable Ironwood notes of one account at the default ZIP 315 policy, minus `exclude`.
pub fn spendable_notes(
    db: &Db,
    account: AccountUuid,
    exclude: &[ReceivedNoteId],
) -> Result<Vec<Note>> {
    let Some((target, _)) = db.get_target_and_anchor_heights(NonZeroU32::MIN)? else {
        return Ok(vec![]);
    };
    Ok(db
        .select_spendable_notes(
            account,
            TargetValue::AllFunds(MaxSpendMode::MaxSpendable),
            &[ShieldedPool::Ironwood],
            target,
            ConfirmationsPolicy::default(),
            exclude,
            LockFilter::Unfiltered,
        )?
        .take_ironwood())
}

/// Total Ironwood value an account holds, spendable or not (pending detection).
pub fn total_value(db: &Db, account: AccountUuid) -> Result<u64> {
    let Some(s) = db.get_wallet_summary(ConfirmationsPolicy::default())? else {
        return Ok(0);
    };
    Ok(s.account_balances()
        .get(&account)
        .map(|b| u64::from(b.ironwood_balance().total()))
        .unwrap_or(0))
}

pub fn orchard_receiver(ua: &str) -> Result<orchard::Address> {
    match Address::decode(&PARAMS, ua).ok_or(anyhow!("bad address"))? {
        Address::Unified(ua) => Ok(*ua.orchard().ok_or(anyhow!("no orchard receiver"))?),
        _ => bail!("need a unified address"),
    }
}

pub fn is_valid_shielded(ua: &str) -> bool {
    ZcashAddress::try_from_encoded(ua).is_ok() && orchard_receiver(ua).is_ok()
}

pub struct Built {
    pub txid: TxId,
    pub raw: Vec<u8>,
    pub expiry: BlockHeight,
}

/// Builds a desk transaction with the custom builder: at most 2 spends, at most 2 outputs, no
/// OVK (`OvkPolicy::Discard`), explicit expiry. Refuses anything that is not exactly 2 actions
/// and the standard 10,000 zat fee.
pub fn build(
    db: &mut Db,
    spends: &[(&Key, &Note)],
    outputs: &[(orchard::Address, u64)],
    expiry_delta: u32,
) -> Result<Built> {
    ensure!(
        (1..=2).contains(&spends.len()) && (1..=2).contains(&outputs.len()),
        "not a 2-action op"
    );
    let (target, anchor_h) = db
        .get_target_and_anchor_heights(NonZeroU32::new(3).ok_or(anyhow!("anchor depth"))?)?
        .ok_or(anyhow!("not synced"))?;
    let target_h: BlockHeight = target.into();
    let (anchor, paths) = db
        .with_ironwood_tree_mut::<_, _, anyhow::Error>(|tree| {
            let root = tree
                .root_at_checkpoint_id(&anchor_h)?
                .ok_or(anyhow!("no anchor"))?;
            let mut paths = vec![];
            for (_, n) in spends {
                paths.push(
                    tree.witness_at_checkpoint_id_caching(
                        n.note_commitment_tree_position(),
                        &anchor_h,
                    )?
                    .ok_or(anyhow!("no witness"))?,
                );
            }
            Ok((orchard::Anchor::from(root), paths))
        })?
        .ok_or(anyhow!("no ironwood tree"))?;
    let usks: Vec<UnifiedSpendingKey> =
        spends.iter().map(|(k, _)| k.usk()).collect::<Result<_>>()?;
    let expiry = target_h + expiry_delta;
    let mut b = Builder::new(
        PARAMS,
        target_h,
        BuildConfig::Standard {
            sapling_anchor: None,
            orchard_anchor: None,
            ironwood_anchor: Some(anchor),
            orchard_padding: BundlePadding::DEFAULT,
            ironwood_padding: BundlePadding::DEFAULT,
        },
    )
    .with_expiry_height(expiry);
    for ((usk, (_, n)), path) in usks.iter().zip(spends).zip(paths) {
        let fvk: orchard::keys::FullViewingKey = usk.orchard().into();
        b.add_ironwood_spend::<zip317::FeeError>(
            fvk,
            *n.note(),
            orchard::tree::MerklePath::from(path),
        )
        .map_err(|e| anyhow!("{e}"))?;
    }
    for (to, v) in outputs {
        b.add_ironwood_output::<zip317::FeeError>(
            None,
            *to,
            Zatoshis::from_u64(*v)?,
            MemoBytes::empty(),
        )
        .map_err(|e| anyhow!("{e}"))?;
    }
    let fee_rule = zip317::FeeRule::standard();
    let fee = u64::from(b.get_fee(&fee_rule).map_err(|e| anyhow!("{e:?}"))?);
    ensure!(fee == amnesia_core::FEE_2_ACTIONS, "unexpected fee {fee}");
    let saks: Vec<orchard::keys::SpendAuthorizingKey> =
        usks.iter().map(|u| u.orchard().into()).collect();
    drop(usks);
    let prover = LocalTxProver::bundled();
    let res = b
        .build(
            &TransparentSigningSet::new(),
            &[],
            &saks,
            UnwrapErr(SysRng),
            &prover,
            &prover,
            &fee_rule,
        )
        .map_err(|e| anyhow!("build: {e}"))?;
    let tx = res.transaction();
    let actions = tx
        .ironwood_bundle()
        .map(|b: &orchard::Bundle<_, _>| b.actions().len())
        .unwrap_or(0);
    ensure!(
        actions == 2 && tx.orchard_bundle().is_none() && tx.sapling_bundle().is_none(),
        "tx shape"
    );
    ensure!(tx.transparent_bundle().is_none(), "transparent component");
    let mut raw = vec![];
    tx.write(&mut raw)?;
    Ok(Built {
        txid: tx.txid(),
        raw,
        expiry,
    })
}

pub fn tx_height(db: &Db, txid: TxId) -> Result<Option<BlockHeight>> {
    Ok(db.get_tx_height(txid)?)
}

pub fn birthday_from(
    ts: zcash_client_backend::proto::service::TreeState,
) -> Result<AccountBirthday> {
    AccountBirthday::from_treestate(ts, None).map_err(|e| anyhow!("{e:?}"))
}
