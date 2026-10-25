//! Ordinary ZIP 32 seed wallets: the baseline desk (today's best practice) and the exchange.
//! The seed sits on disk in `seed.hex` on purpose: that is what a seizure finds.

use std::{fs, path::Path};

use amnesia_desk::{
    PARAMS,
    chain::{self, Chain, Client},
    wallet::{self, Db},
};
use anyhow::{Result, anyhow, bail};
use rand::{
    rand_core::{Rng as _, UnwrapErr},
    rngs::SysRng,
};
use secrecy::{ExposeSecret, SecretVec};
use zcash_address::ZcashAddress;
use zcash_client_backend::{
    data_api::{
        WalletRead, WalletWrite,
        wallet::{
            ConfirmationsPolicy, SpendingKeys, create_proposed_transactions,
            input_selection::{GreedyInputSelector, SpendPolicy},
            propose_transfer,
        },
    },
    fees::{DustOutputPolicy, SplitPolicy, StandardFeeRule, standard::MultiOutputChangeStrategy},
    wallet::OvkPolicy,
};
use zcash_client_sqlite::{AccountUuid, wallet::init::init_wallet_db};
use zcash_keys::keys::{UnifiedAddressRequest, UnifiedSpendingKey};
use zcash_proofs::prover::LocalTxProver;
use zcash_protocol::{ShieldedPool, TxId, consensus::BlockHeight, value::Zatoshis};

type CtErr = zcash_client_sqlite::wallet::commitment_tree::Error;

pub struct SeedWallet {
    pub db: Db,
    seed: SecretVec<u8>,
    pub account: AccountUuid,
}

impl SeedWallet {
    pub async fn init(dir: &Path, client: &mut Client) -> Result<()> {
        if dir.join("seed.hex").exists() {
            bail!("wallet exists");
        }
        fs::create_dir_all(dir)?;
        // seed.hex appears only once the wallet is complete, so a failed init is retried from
        // scratch instead of leaving a seed with no account.
        let _ = fs::remove_file(dir.join("data.sqlite"));
        let tip = chain::tip(client).await?;
        let bday_h = tip - 2;
        let birthday = wallet::birthday_from(chain::treestate(client, bday_h - 1).await?)?;
        let mut seed = vec![0u8; 32];
        UnwrapErr(SysRng).fill_bytes(&mut seed);
        let seed = SecretVec::new(seed);
        let mut db = wallet::open_db(dir)?;
        init_wallet_db(&mut db, Some(SecretVec::new(seed.expose_secret().clone()))).map_err(|e| anyhow!("init: {e:?}"))?;
        db.create_account("main", &seed, &birthday, None)?;
        chain::put_roots(client, &mut db).await?;
        let tmp = dir.join("seed.hex.tmp");
        fs::write(&tmp, hex::encode(seed.expose_secret()))?;
        fs::rename(tmp, dir.join("seed.hex"))?;
        Ok(())
    }

    pub fn open(dir: &Path) -> Result<Self> {
        let seed = SecretVec::new(hex::decode(fs::read_to_string(dir.join("seed.hex"))?.trim())?);
        let db = wallet::open_db(dir)?;
        let account = *db.get_account_ids()?.first().ok_or(anyhow!("no account"))?;
        Ok(Self { db, seed, account })
    }

    fn usk(&self) -> Result<UnifiedSpendingKey> {
        Ok(UnifiedSpendingKey::from_seed(&PARAMS, self.seed.expose_secret(), zip32::AccountId::ZERO)?)
    }

    /// A fresh diversified Orchard-only address under the same seed: one per donor.
    pub fn next_address(&mut self) -> Result<String> {
        let (ua, _) = self
            .db
            .get_next_available_address(self.account, UnifiedAddressRequest::ORCHARD)?
            .ok_or(anyhow!("no address"))?;
        Ok(ua.encode(&PARAMS))
    }

    /// Sapling + Orchard receivers: only for the Testnet faucet, which cannot pay Orchard-only
    /// addresses. Lab funding only; the desk never issues one.
    pub fn funding_address(&mut self) -> Result<String> {
        let (ua, _) = self
            .db
            .get_next_available_address(self.account, UnifiedAddressRequest::SHIELDED)?
            .ok_or(anyhow!("no address"))?;
        Ok(ua.encode(&PARAMS))
    }

    /// A transparent (P2PKH) receiver of this wallet: the Valar faucet pays transparent
    /// addresses but answers "the wallet cannot pay this address" for Ironwood ones.
    pub fn transparent_address(&mut self) -> Result<String> {
        use zcash_keys::encoding::AddressCodec as _;
        if let Some(a) = self.db.get_transparent_receivers(self.account, false, true)?.into_keys().next() {
            return Ok(a.encode(&PARAMS));
        }
        let (ua, _) = self
            .db
            .get_next_available_address(self.account, UnifiedAddressRequest::ALLOW_ALL)?
            .ok_or(anyhow!("no address"))?;
        let t = ua.transparent().ok_or(anyhow!("no transparent receiver"))?;
        Ok(t.encode(&PARAMS))
    }

    /// Records this wallet's unspent transparent outputs from lightwalletd: compact blocks
    /// carry no transparent data (same approach as zcash-devtool `wallet sync`).
    pub async fn refresh_utxos(&mut self, client: &mut Client) -> Result<usize> {
        use futures_util::TryStreamExt as _;
        use zcash_keys::encoding::AddressCodec as _;
        let addresses: Vec<String> = self
            .db
            .get_transparent_receivers(self.account, true, true)?
            .into_keys()
            .map(|a| a.encode(&PARAMS))
            .collect();
        if addresses.is_empty() {
            return Ok(0);
        }
        let req = zcash_client_backend::proto::service::GetAddressUtxosArg { addresses, start_height: 0, max_entries: 0 };
        let replies: Vec<_> = client.get_address_utxos_stream(req).await?.into_inner().try_collect().await?;
        let n = replies.len();
        for r in replies {
            let out = zcash_client_backend::wallet::WalletTransparentOutput::from_parts(
                transparent::bundle::OutPoint::new(r.txid[..].try_into()?, r.index.try_into()?),
                transparent::bundle::TxOut::new(
                    Zatoshis::from_nonnegative_i64(r.value_zat)?,
                    transparent::address::Script(zcash_script::script::Code(r.script)),
                ),
                Some(BlockHeight::from(u32::try_from(r.height)?)),
                None,
                None,
                None,
            )
            .ok_or(anyhow!("utxo is not P2PKH or P2SH"))?;
            self.db.put_received_transparent_utxo(&out)?;
        }
        Ok(n)
    }

    /// Shields every confirmed transparent output into Ironwood (NU6.3+ forbids new value
    /// entering Orchard), split into several notes so the float can serve parallel payments.
    pub async fn shield(&mut self, chain: &Chain) -> Result<Option<TxId>> {
        let Some(tip) = self.db.chain_height()? else { return Ok(None) };
        let target = (tip + 1).into();
        let policy = ConfirmationsPolicy::MIN;
        let from: Vec<_> = self.db.get_transparent_balances(self.account, target, policy)?.into_keys().collect();
        if from.is_empty() {
            return Ok(None);
        }
        let change = MultiOutputChangeStrategy::new(
            StandardFeeRule::Zip317,
            None,
            ShieldedPool::Ironwood,
            DustOutputPolicy::default(),
            SplitPolicy::with_min_output_value(
                std::num::NonZeroUsize::new(8).ok_or(anyhow!("note count"))?,
                Zatoshis::from_u64(400_000)?,
            ),
        );
        let proposal = zcash_client_backend::data_api::wallet::propose_shielding::<_, _, _, _, CtErr>(
            &mut self.db,
            &PARAMS,
            &GreedyInputSelector::new(),
            &change,
            Zatoshis::ZERO,
            &from,
            self.account,
            policy,
            zcash_client_backend::data_api::CoinbaseFilter::AllTransparentOutputs,
            None,
        )
        .map_err(|e| anyhow!("propose shielding: {e}"))?;
        let usk = self.usk()?;
        let prover = LocalTxProver::bundled();
        let txids = create_proposed_transactions::<_, _, std::convert::Infallible, _, std::convert::Infallible, _>(
            &mut self.db,
            &PARAMS,
            &zcash_client_sqlite::util::SystemClock,
            &mut UnwrapErr(SysRng),
            &prover,
            &prover,
            &SpendingKeys::from_unified_spending_key(usk),
            OvkPolicy::Sender,
            &proposal,
            None,
        )
        .map_err(|e| anyhow!("create shielding: {e}"))?;
        let txid = *txids.first();
        let tx = self.db.get_transaction(txid)?.ok_or(anyhow!("tx not stored"))?;
        let mut raw = vec![];
        tx.write(&mut raw)?;
        chain.broadcast(raw).await?;
        Ok(Some(txid))
    }

    pub async fn sync(&mut self, client: &mut Client, dir: &Path) -> Result<BlockHeight> {
        let tip = chain::tip(client).await?;
        chain::sync(client, &mut self.db, dir, tip).await?;
        Ok(tip)
    }

    pub fn spendable(&self) -> Result<u64> {
        let s = self.db.get_wallet_summary(ConfirmationsPolicy::default())?.ok_or(anyhow!("not synced"))?;
        Ok(s.account_balances()
            .get(&self.account)
            .map(|b| u64::from(b.ironwood_balance().spendable_value()))
            .unwrap_or(0))
    }

    /// Pays `to` like an exchange or a normal wallet does: with the default `OvkPolicy::Sender`.
    pub async fn pay(&mut self, chain: &Chain, to: &str, zats: u64) -> Result<TxId> {
        self.pay_many(chain, &[(to.to_string(), zats)]).await
    }

    /// One transaction with several outputs (the exchange uses it to split its float).
    pub async fn pay_many(&mut self, chain: &Chain, outs: &[(String, u64)]) -> Result<TxId> {
        let mut payments = vec![];
        for (to, zats) in outs {
            payments.push(zip321::Payment::without_memo(ZcashAddress::try_from_encoded(to)?, Zatoshis::from_u64(*zats)?));
        }
        let req = zip321::TransactionRequest::new(payments)?;
        let change = MultiOutputChangeStrategy::new(
            StandardFeeRule::Zip317,
            None,
            ShieldedPool::Ironwood,
            DustOutputPolicy::default(),
            SplitPolicy::single_output(),
        );
        let proposal = propose_transfer::<_, _, _, _, CtErr>(
            &mut self.db,
            &PARAMS,
            self.account,
            &GreedyInputSelector::new(),
            &change,
            req,
            ConfirmationsPolicy::default(),
            &SpendPolicy::default(),
            None,
            None,
        )
        .map_err(|e| anyhow!("propose: {e}"))?;
        let usk = self.usk()?;
        let prover = LocalTxProver::bundled();
        let txids = create_proposed_transactions::<_, _, std::convert::Infallible, _, std::convert::Infallible, _>(
            &mut self.db,
            &PARAMS,
            &zcash_client_sqlite::util::SystemClock,
            &mut UnwrapErr(SysRng),
            &prover,
            &prover,
            &SpendingKeys::from_unified_spending_key(usk),
            OvkPolicy::Sender,
            &proposal,
            None,
        )
        .map_err(|e| anyhow!("create: {e}"))?;
        let txid = *txids.first();
        let tx = self.db.get_transaction(txid)?.ok_or(anyhow!("tx not stored"))?;
        let mut raw = vec![];
        tx.write(&mut raw)?;
        chain.broadcast(raw).await?;
        Ok(txid)
    }
}
