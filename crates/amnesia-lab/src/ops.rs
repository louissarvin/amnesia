//! Exchange-side actions shared by the CLI and the daemon.

use std::path::Path;

use amnesia_desk::chain::Chain;
use anyhow::{Context, Result, anyhow, bail};
use rand::{
    rand_core::{Rng as _, UnwrapErr},
    rngs::SysRng,
};
use zcash_client_backend::data_api::WalletRead;
use zcash_keys::keys::{UnifiedAddressRequest, UnifiedSpendingKey};
use zcash_protocol::TxId;

use crate::{intake_client, records, seedwallet::SeedWallet};

/// A shielded address under a throwaway random key: an unrelated recipient.
pub fn random_address() -> Result<String> {
    let mut seed = [0u8; 32];
    UnwrapErr(SysRng).fill_bytes(&mut seed);
    let usk = UnifiedSpendingKey::from_seed(&amnesia_desk::PARAMS, &seed, zip32::AccountId::ZERO)?;
    let (ua, _) = usk.to_unified_full_viewing_key().default_address(UnifiedAddressRequest::ORCHARD)?;
    Ok(ua.encode(&amnesia_desk::PARAMS))
}

/// TxIds display byte-reversed, like Bitcoin.
pub fn parse_txid(s: &str) -> Result<TxId> {
    let mut b: [u8; 32] = hex::decode(s)?.try_into().map_err(|_| anyhow!("bad txid"))?;
    b.reverse();
    Ok(TxId::from_bytes(b))
}

fn record(dir: &Path, client: &str, to: &str, zats: u64, txid: TxId, kind: &str) -> Result<()> {
    records::append(
        dir,
        &records::Withdrawal {
            client_id: client.into(),
            recipient_address: to.into(),
            value_zats: zats,
            txid: txid.to_string(),
            kind: kind.into(),
            height: None,
        },
    )
}

/// The intake answers 503 while the desk lock is held by a slow tick: transient, so retry for ~3 minutes.
async fn fetch_address_retrying(intake: &str) -> Result<(String, u64, Option<String>)> {
    let mut tries = 0;
    loop {
        match intake_client::fetch_address(intake).await {
            Err(e) if tries < 9 && e.to_string().contains("returned 503") => {
                tries += 1;
                tokio::time::sleep(std::time::Duration::from_secs(20)).await;
            }
            r => return r,
        }
    }
}

pub struct Donation {
    pub amnesia_address: String,
    /// The intake's payment deadline, floored to the hour as the desk reports it.
    pub valid_until: u64,
    /// The desk's shred receipt for this address's epoch.
    pub receipt: Option<String>,
    pub amnesia_txid: TxId,
    pub baseline_txid: TxId,
}

/// The client withdraws `zats` to a fresh Amnesia address (fetched through the desk's own
/// intake, like any donor) and the same amount to a fresh baseline address.
pub async fn donate(
    chain: &Chain,
    exch_dir: &Path,
    exch: &mut SeedWallet,
    baseline: &mut SeedWallet,
    intake: &str,
    client: &str,
    zats: u64,
) -> Result<Donation> {
    let (amnesia_address, valid_until, receipt) = fetch_address_retrying(intake).await.context("amnesia intake")?;
    let base_addr = baseline.next_address()?;
    let amnesia_txid = exch.pay(chain, &amnesia_address, zats).await?;
    record(exch_dir, client, &amnesia_address, zats, amnesia_txid, "amnesia")?;
    let baseline_txid = exch.pay(chain, &base_addr, zats).await?;
    record(exch_dir, client, &base_addr, zats, baseline_txid, "baseline")?;
    Ok(Donation { amnesia_address, valid_until, receipt, amnesia_txid, baseline_txid })
}

/// A withdrawal to an unrelated recipient. With `sink`, the recipient is a fresh address of a
/// lab wallet that the attacker never sees, so the float is recycled instead of burned.
pub async fn decoy(
    chain: &Chain,
    exch_dir: &Path,
    exch: &mut SeedWallet,
    sink: Option<&mut SeedWallet>,
    client: &str,
    zats: u64,
) -> Result<TxId> {
    let to = match sink {
        Some(w) => w.next_address()?,
        None => random_address()?,
    };
    let t = exch.pay(chain, &to, zats).await?;
    record(exch_dir, client, &to, zats, t, "decoy")?;
    Ok(t)
}

/// Fans the exchange float out into `n` notes so several withdrawals can go out per block.
pub async fn split(chain: &Chain, exch: &mut SeedWallet, n: usize, each: u64) -> Result<TxId> {
    let outs = (0..n).map(|_| Ok((exch.next_address()?, each))).collect::<Result<Vec<_>>>()?;
    exch.pay_many(chain, &outs).await
}

/// The withdrawal log an attacker of `target` receives: every exchange withdrawal except those
/// to the other demo desk (which would not exist in the real world).
pub fn records_csv(exch_dir: &Path, exch: &SeedWallet, target: &str) -> Result<String> {
    let skip = match target {
        "amnesia" => "baseline",
        "baseline" => "amnesia",
        _ => bail!("target is amnesia or baseline"),
    };
    let mut rows: Vec<_> = records::load(exch_dir)?.into_iter().filter(|r| r.kind != skip).collect();
    for r in &mut rows {
        r.height = exch.db.get_tx_height(parse_txid(&r.txid)?)?.map(u32::from);
    }
    Ok(records::to_csv(&rows))
}
