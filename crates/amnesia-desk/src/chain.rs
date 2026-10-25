//! Chain reads (compact blocks only, never `GetTransaction`) and broadcast.

use std::{fs, path::Path};

use anyhow::{Context, Result, anyhow, bail};
use futures_util::TryStreamExt;
use orchard::tree::MerkleHashOrchard;
use prost::Message;
use tonic::transport::{Channel, ClientTlsConfig};
use zcash_client_backend::{
    data_api::{
        WalletCommitmentTrees, WalletRead, WalletWrite,
        chain::{ChainState, CommitmentTreeRoot, error::Error as ChainError, scan_cached_blocks},
        scanning::ScanPriority,
    },
    proto::service::{self, BlockId, compact_tx_streamer_client::CompactTxStreamerClient},
};
use zcash_client_sqlite::{
    FsBlockDb,
    chain::{BlockMeta, init::init_blockmeta_db},
};
use zcash_primitives::merkle_tree::HashSer;
use zcash_protocol::consensus::BlockHeight;

use crate::{PARAMS, wallet::Db};

pub type Client = CompactTxStreamerClient<Channel>;

#[derive(Clone)]
pub struct Chain {
    host: String,
    tor: Option<zcash_client_backend::tor::Client>,
}

impl Chain {
    /// `tor_dir` set: reads and every broadcast go over Tor, each on its own isolated circuit,
    /// so lightwalletd never sees the host IP and cannot tie reads to broadcasts (ZIP 318).
    pub async fn new(host: &str, tor_dir: Option<&Path>) -> Result<Self> {
        let tor = match tor_dir {
            Some(dir) => {
                fs::create_dir_all(dir)?;
                // A stalled bootstrap must fail loudly, never fall back to clearnet broadcast.
                let c = tokio::time::timeout(
                    std::time::Duration::from_secs(180),
                    zcash_client_backend::tor::Client::create(dir, |_| {}),
                )
                .await
                .map_err(|_| anyhow!("tor bootstrap timed out"))??;
                crate::log::event("tor_ready", &[]);
                Some(c)
            }
            None => None,
        };
        Ok(Self { host: host.to_string(), tor })
    }

    pub fn uses_tor(&self) -> bool {
        self.tor.is_some()
    }

    pub async fn connect(&self) -> Result<Client> {
        if let Some(tor) = &self.tor {
            let uri = format!("https://{}:443", self.host).parse()?;
            return tokio::time::timeout(
                std::time::Duration::from_secs(60),
                tor.isolated_client().connect_to_lightwalletd(uri, false),
            )
            .await
            .map_err(|_| anyhow!("tor read connect timed out"))?
            .map_err(|e| anyhow!("tor read connect: {e:?}"));
        }
        let tls = ClientTlsConfig::new()
            .domain_name(self.host.clone())
            .assume_http2(true)
            .with_webpki_roots();
        // Bounded so a dead link (sleep, Wi-Fi drop) fails the tick instead of hanging it.
        let ch = Channel::from_shared(format!("https://{}:443", self.host))?
            .tls_config(tls)?
            .connect_timeout(std::time::Duration::from_secs(20))
            .timeout(std::time::Duration::from_secs(120))
            .connect()
            .await?;
        Ok(CompactTxStreamerClient::new(ch))
    }

    pub async fn broadcast(&self, raw: Vec<u8>) -> Result<()> {
        tokio::time::timeout(std::time::Duration::from_secs(90), self.broadcast_inner(raw))
            .await
            .map_err(|_| anyhow!("broadcast timed out"))?
    }

    async fn broadcast_inner(&self, raw: Vec<u8>) -> Result<()> {
        let mut client = match &self.tor {
            Some(tor) => {
                tor.isolated_client()
                    .connect_to_lightwalletd(format!("https://{}:443", self.host).parse()?, false)
                    .await?
            }
            None => self.connect().await?,
        };
        let resp = client
            .send_transaction(service::RawTransaction { data: raw, height: 0 })
            .await?
            .into_inner();
        if resp.error_code != 0 {
            bail!("broadcast rejected ({})", resp.error_code);
        }
        Ok(())
    }
}

pub async fn tip(client: &mut Client) -> Result<BlockHeight> {
    let h = client.get_latest_block(service::ChainSpec::default()).await?.into_inner().height;
    Ok(BlockHeight::from_u32(u32::try_from(h)?))
}

pub async fn treestate(client: &mut Client, height: BlockHeight) -> Result<service::TreeState> {
    Ok(client
        .get_tree_state(BlockId { height: u32::from(height).into(), hash: vec![] })
        .await?
        .into_inner())
}

pub async fn put_roots(client: &mut Client, db: &mut Db) -> Result<()> {
    let mut req = service::GetSubtreeRootsArg::default();
    req.set_shielded_protocol(service::ShieldedProtocol::Sapling);
    let s: Vec<CommitmentTreeRoot<sapling::Node>> = client
        .get_subtree_roots(req)
        .await?
        .into_inner()
        .and_then(|r| async move {
            Ok(CommitmentTreeRoot::from_parts(
                BlockHeight::from_u32(r.completing_block_height as u32),
                sapling::Node::read(&r.root_hash[..])
                    .map_err(|e| tonic::Status::internal(e.to_string()))?,
            ))
        })
        .try_collect()
        .await?;
    db.put_sapling_subtree_roots(0, &s)?;
    for (proto, is_iw) in [
        (service::ShieldedProtocol::Orchard, false),
        (service::ShieldedProtocol::Ironwood, true),
    ] {
        let mut req = service::GetSubtreeRootsArg::default();
        req.set_shielded_protocol(proto);
        let o: Vec<CommitmentTreeRoot<MerkleHashOrchard>> = client
            .get_subtree_roots(req)
            .await?
            .into_inner()
            .and_then(|r| async move {
                Ok(CommitmentTreeRoot::from_parts(
                    BlockHeight::from_u32(r.completing_block_height as u32),
                    MerkleHashOrchard::read(&r.root_hash[..])
                        .map_err(|e| tonic::Status::internal(e.to_string()))?,
                ))
            })
            .try_collect()
            .await?;
        if is_iw {
            db.put_ironwood_subtree_roots(0, &o)?;
        } else {
            db.put_orchard_subtree_roots(0, &o)?;
        }
    }
    Ok(())
}

/// Downloads `[start, end)` into the epoch's block cache.
async fn download(
    client: &mut Client,
    cache_root: &Path,
    start: BlockHeight,
    end: BlockHeight,
) -> Result<Vec<BlockMeta>> {
    let range = service::BlockRange {
        start: Some(BlockId { height: u32::from(start).into(), hash: vec![] }),
        end: Some(BlockId { height: (u32::from(end) - 1).into(), hash: vec![] }),
        pool_types: Default::default(),
    };
    let mut stream = client.get_block_range(range).await?.into_inner();
    let mut metas = vec![];
    let blocks_dir = cache_root.join("blocks");
    while let Some(block) = stream.try_next().await? {
        let (s, o) = block.vtx.iter().fold((0u32, 0u32), |a, tx| {
            (
                a.0 + tx.outputs.len() as u32,
                a.1 + tx.actions.len() as u32 + tx.ironwood_actions.len() as u32,
            )
        });
        let meta = BlockMeta {
            height: block.height(),
            block_hash: block.hash(),
            block_time: block.time,
            sapling_outputs_count: s,
            orchard_actions_count: o,
        };
        fs::write(meta.block_file_path(&blocks_dir), block.encode_to_vec())?;
        metas.push(meta);
    }
    Ok(metas)
}

/// Scans the epoch wallet to `tip`. The block cache lives inside the epoch directory, so it is
/// destroyed with it.
/// Blocks downloaded and scanned per request.
const SYNC_CHUNK_BLOCKS: u32 = 100;

pub async fn sync(client: &mut Client, db: &mut Db, dir: &Path, tip: BlockHeight) -> Result<()> {
    let cache_root = dir.join("cache");
    fs::create_dir_all(&cache_root)?;
    // The cache handle is not Sync, so it is only opened between awaits.
    let open_cache = || -> Result<FsBlockDb> {
        let mut cache = FsBlockDb::for_path(&cache_root).map_err(|e| anyhow!("{e:?}"))?;
        init_blockmeta_db(&mut cache)?;
        Ok(cache)
    };
    open_cache()?;
    db.update_chain_tip(tip)?;
    loop {
        let ranges = db.suggest_scan_ranges()?;
        let Some(r) = ranges.into_iter().find(|r| r.priority() > ScanPriority::Scanned) else {
            break;
        };
        // Bounded chunks: after an outage a range can be thousands of blocks, and one request
        // for all of it over a slow Tor circuit fails partway and restarts from scratch forever.
        // Scanning 100 at a time saves progress after every chunk.
        let start = r.block_range().start;
        let end = std::cmp::min(r.block_range().end, start + SYNC_CHUNK_BLOCKS);
        let metas = download(client, &cache_root, start, end)
            .await
            .with_context(|| format!("download {start}..{end}"))?;
        let cs: ChainState = treestate(client, start - 1)
            .await
            .with_context(|| format!("treestate {}", start - 1))?
            .to_chain_state()?;
        let reorg_at = {
            let cache = open_cache()?;
            cache.write_block_metadata(&metas).map_err(|e| anyhow!("{e:?}"))?;
            match scan_cached_blocks(&PARAMS, &cache, db, start, &cs, metas.len()) {
                Ok(_) => None,
                Err(ChainError::Scan(e)) if e.is_continuity_error() => Some(e.at_height()),
                Err(e) => bail!("scan: {e:?}"),
            }
        };
        for m in &metas {
            let _ = fs::remove_file(m.block_file_path(&cache_root.join("blocks")));
        }
        // A reorg replaced blocks we had scanned: rewind below it and rescan, as the
        // zcash_client_backend data_api::chain docs prescribe. Without this one reorg wedges the desk.
        if let Some(h) = reorg_at {
            let rewind = h.saturating_sub(10);
            let cs = treestate(client, rewind).await?.to_chain_state()?;
            db.truncate_to_chain_state(cs)?;
            open_cache()?.truncate_to_height(rewind).map_err(|e| anyhow!("{e:?}"))?;
            // Truncation drops the tip; re-announce it so the blocks above are queued again.
            db.update_chain_tip(tip)?;
            crate::log::event("read_source", &[("code", "reorg")]);
        }
    }
    let scanned = db.block_fully_scanned()?.map(|m| m.block_height());
    if scanned < Some(tip) {
        bail!("scan stopped below tip");
    }
    Ok(())
}
