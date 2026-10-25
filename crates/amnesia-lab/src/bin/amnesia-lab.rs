//! Demo driver.
//!
//!   amnesia-lab serve                            demo API + bots (env: see daemon::Config)
//!   amnesia-lab init-all                         create exchange, baseline, treasury wallets;
//!                                                print the treasury address for the desk
//!   amnesia-lab init DIR                         new seed wallet
//!   amnesia-lab sync DIR                         scan to tip, print spendable
//!   amnesia-lab addr DIR                         next diversified address
//!   amnesia-lab taddr DIR                        transparent receiver (faucet and Testnet donors)
//!   amnesia-lab donate EXCH CLIENT INTAKE BASE ZATS
//!   amnesia-lab split EXCH N ZATS                fan the float out into N notes
//!   amnesia-lab decoy EXCH CLIENT ZATS
//!   amnesia-lab records EXCH FOR                 CSV for the attack on FOR (amnesia|baseline)

use std::path::{Path, PathBuf};

use amnesia_desk::chain::Chain;
use amnesia_lab::{daemon, ops, seedwallet::SeedWallet};
use anyhow::{Result, anyhow, bail};

fn arg(a: &[String], i: usize) -> Result<&str> {
    a.get(i).map(String::as_str).ok_or(anyhow!("missing argument {i}"))
}

async fn chain() -> Result<Chain> {
    let host = std::env::var("AMNESIA_LWD_HOST").unwrap_or_else(|_| "testnet.zec.rocks".into());
    let tor = std::env::var("AMNESIA_TOR").ok().filter(|v| v == "1").map(|_| PathBuf::from("var/lab-arti"));
    Chain::new(&host, tor.as_deref()).await
}

async fn synced(dir: &Path, chain: &Chain) -> Result<SeedWallet> {
    let mut w = SeedWallet::open(dir)?;
    let mut c = chain.connect().await?;
    w.sync(&mut c, dir).await?;
    Ok(w)
}

#[tokio::main]
async fn main() -> Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match arg(&a, 0)? {
        "serve" => daemon::serve(daemon::Config::from_env()).await?,
        "init-all" => println!("{}", daemon::init_all(&daemon::Config::from_env(), &chain().await?).await?),
        "init" => {
            let chain = chain().await?;
            let mut c = chain.connect().await?;
            SeedWallet::init(Path::new(arg(&a, 1)?), &mut c).await?;
            println!("{}", SeedWallet::open(Path::new(arg(&a, 1)?))?.next_address()?);
        }
        "sync" => {
            let w = synced(Path::new(arg(&a, 1)?), &chain().await?).await?;
            println!("spendable {}", w.spendable()?);
        }
        "taddr" => println!("{}", SeedWallet::open(Path::new(arg(&a, 1)?))?.transparent_address()?),
        "fund-address" => println!("{}", SeedWallet::open(Path::new(arg(&a, 1)?))?.funding_address()?),
        "addr" => println!("{}", SeedWallet::open(Path::new(arg(&a, 1)?))?.next_address()?),
        "donate" => {
            let (exch, client, intake, base) = (Path::new(arg(&a, 1)?), arg(&a, 2)?, arg(&a, 3)?, Path::new(arg(&a, 4)?));
            let zats: u64 = arg(&a, 5)?.parse()?;
            let chain = chain().await?;
            let mut w = synced(exch, &chain).await?;
            let mut b = SeedWallet::open(base)?;
            ops::donate(&chain, exch, &mut w, &mut b, intake, client, zats).await?;
            println!("{client} donated {zats} to both desks");
        }
        "split" => {
            let exch = Path::new(arg(&a, 1)?);
            let (n, each): (usize, u64) = (arg(&a, 2)?.parse()?, arg(&a, 3)?.parse()?);
            let chain = chain().await?;
            let mut w = synced(exch, &chain).await?;
            ops::split(&chain, &mut w, n, each).await?;
            println!("split into {n} notes of {each}");
        }
        "decoy" => {
            let (exch, client) = (Path::new(arg(&a, 1)?), arg(&a, 2)?);
            let zats: u64 = arg(&a, 3)?.parse()?;
            let chain = chain().await?;
            let mut w = synced(exch, &chain).await?;
            ops::decoy(&chain, exch, &mut w, None, client, zats).await?;
            println!("{client} withdrew {zats} elsewhere");
        }
        "records" => {
            let exch = Path::new(arg(&a, 1)?);
            print!("{}", ops::records_csv(exch, &SeedWallet::open(exch)?, arg(&a, 2)?)?);
        }
        other => bail!("unknown command {other}"),
    }
    Ok(())
}
