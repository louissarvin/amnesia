//! Desk daemon entry point.


use std::{path::PathBuf, sync::Arc, time::Duration};

use amnesia_core::params::Params;
use anyhow::{Context, Result, anyhow};
use tokio::sync::Mutex;
use amnesia_desk::{chain, desk, hardening, intake, ledger, log};
use rand::{rand_core::UnwrapErr, rngs::SysRng};

/// Wipes every heap block before it is freed. Donor addresses pass through buffers we do not
/// own (hyper, axum, librustzcash); a RAM seizure after a shred must not find them.
struct ZeroOnFree;

// SAFETY: forwards to the system allocator unchanged; `dealloc` only writes within the
// block it is about to release, whose size is given by `layout`.
unsafe impl std::alloc::GlobalAlloc for ZeroOnFree {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        unsafe { std::alloc::System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: std::alloc::Layout) -> *mut u8 {
        unsafe { std::alloc::System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        unsafe {
            std::ptr::write_bytes(ptr, 0, layout.size());
            std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
            std::alloc::System.dealloc(ptr, layout)
        }
    }
    // `realloc` keeps the default (alloc, copy, dealloc), so the old block is wiped too.
}

#[global_allocator]
static ALLOC: ZeroOnFree = ZeroOnFree;

fn env_or(k: &str, d: &str) -> String {
    std::env::var(k).unwrap_or_else(|_| d.to_string())
}

#[tokio::main]
async fn main() -> Result<()> {
    hardening::apply();
    let strict = env_or("AMNESIA_STRICT", "0") == "1";
    let profile = env_or("AMNESIA_PROFILE", "dev");
    let p = Params::by_name(&profile).ok_or(anyhow!("unknown AMNESIA_PROFILE"))?;
    let root = PathBuf::from(env_or("AMNESIA_ROOT", "var/desk"));
    let epochs_dir = std::env::var("AMNESIA_EPOCHS_DIR").map(PathBuf::from).unwrap_or(root.join("epochs"));
    if strict {
        hardening::check_host(&epochs_dir)?;
    } else if env_or("AMNESIA_DEV_TRACE", "0") == "1" {
        log::enable_dev_trace();
    }
    let treasury = std::env::var("TREASURY_UA").context("TREASURY_UA unset")?;
    let host = env_or("AMNESIA_LWD_HOST", "testnet.zec.rocks");
    let tor_dir = (env_or("AMNESIA_TOR", "1") == "1").then(|| root.join("arti"));
    let pow_bits: u32 = env_or("AMNESIA_POW_BITS", "18").parse()?;
    // SQLite temp files, if any, stay inside the erasure unit.
    unsafe_set_sqlite_tmp(&epochs_dir);

    log::event("boot", &[("code", &profile)]);
    let chain = chain::Chain::new(&host, tor_dir.as_deref()).await?;
    let ledger = ledger::Ledger::open(&root.join("ledger"))?;
    let d = desk::Desk::new(p, &profile, epochs_dir, ledger, chain, &treasury)?;
    let build = intake::Build {
        profile: profile.clone(),
        binary_sha256: binary_sha256().unwrap_or_else(|_| "unknown".into()),
        cargo_lock_sha256: {
            use sha2::Digest as _;
            hex::encode(sha2::Sha256::digest(include_bytes!("../../../Cargo.lock")))
        },
        broadcast: if d.chain.uses_tor() { "tor_isolated_circuit" } else { "direct" },
    };
    let desk = Arc::new(Mutex::new(d));
    let app = intake::App::new(desk.clone(), &*desk.lock().await, pow_bits, build);

    let donor = tokio::net::TcpListener::bind(env_or("AMNESIA_DONOR_ADDR", "127.0.0.1:8081")).await?;
    let read = tokio::net::TcpListener::bind(env_or("AMNESIA_READ_ADDR", "127.0.0.1:8082")).await?;
    let donor_app = intake::donor_router(app.clone());
    let read_app = intake::read_router(app.clone());
    tokio::spawn(async move { axum::serve(donor, donor_app).await });
    tokio::spawn(async move { axum::serve(read, read_app).await });

    // SIGTERM is honoured only between ticks, so a broadcast is never cut off before `record`.
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    tokio::spawn({
        let stop = stop.clone();
        async move {
            term.recv().await;
            stop.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    });
    let read_chain = desk.lock().await.chain.clone();
    let mut client = None;
    let mut connected_at = std::time::Instant::now();
    loop {
        if stop.load(std::sync::atomic::Ordering::SeqCst) {
            log::event("shutdown", &[]);
            return Ok(());
        }
        // A fresh read circuit every 30 min, so one long-lived circuit is not a stable handle.
        if connected_at.elapsed() > Duration::from_secs(30 * 60) {
            client = None;
        }
        if client.is_none() {
            client = read_chain.connect().await.ok();
            connected_at = std::time::Instant::now();
            if client.is_none() {
                log::event("read_source", &[("code", "connect_failed")]);
            }
        }
        if let Some(c) = client.as_mut() {
            let dirs = desk.lock().await.epoch_dirs();
            let synced = match tokio::time::timeout(Duration::from_secs(600), sync_epochs(c, &dirs)).await {
                Ok(r) => r,
                Err(_) => Err(anyhow!("sync timed out")),
            };
            let mut d = desk.lock().await;
            let held = std::time::Instant::now();
            let res = match synced {
                Ok(tip) => match tokio::time::timeout(Duration::from_secs(600), d.tick(c, tip)).await {
                    Ok(r) => r,
                    Err(_) => Err(anyhow!("tick timed out")),
                },
                Err(e) => Err(e),
            };
            let outbox = d.take_outbox();
            let chain = d.chain.clone();
            drop(d);
            // Intake waits on this lock; a long hold means donors may see "Busy". Local log only:
            // a public per-tick timing would show when the desk does work.
            let secs = held.elapsed().as_secs();
            if secs >= 20 {
                let code = if secs >= 120 { "120s" } else if secs >= 60 { "60s" } else { "20s" };
                log::event("slow_tick", &[("code", code)]);
            }
            let mut results = Vec::with_capacity(outbox.len());
            for out in outbox {
                // Random lag between build and send, so the moment a tx appears does not mark
                // the tick that built it. Cut short on SIGTERM so shutdown stays bounded.
                let lag = amnesia_core::rng::uniform_u64(&mut UnwrapErr(SysRng), 20, 120);
                for _ in 0..lag {
                    if stop.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                let ok = broadcast(&chain, &out.raw).await;
                results.push((out, ok));
            }
            let mut d = desk.lock().await;
            for (out, ok) in results {
                d.record(out, ok);
            }
            if let Err(e) = &res {
                log::fatal(error_code(e));
                log::dev(|| format!("tick error: {e:#}"));
                client = None;
            }
            app.publish(&d, res.is_ok());
            if env_or("AMNESIA_STATUS", "0") == "1" {
                eprintln!("{}", desk::status_line(&d).unwrap_or_default());
            }
        }
        for _ in 0..20 {
            if stop.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}

/// Scans every epoch wallet to the chain tip on its own connection, outside the desk lock.
/// Safe because the main loop is the only writer and runs this strictly between ticks.
async fn sync_epochs(c: &mut chain::Client, dirs: &[PathBuf]) -> Result<zcash_protocol::consensus::BlockHeight> {
    let tip = chain::tip(c).await.context("tip")?;
    for (i, dir) in dirs.iter().enumerate() {
        let mut db = amnesia_desk::wallet::open_db(dir)?;
        chain::sync(c, &mut db, dir, tip).await.with_context(|| format!("epoch #{i} of {}", dirs.len()))?;
    }
    Ok(tip)
}

async fn broadcast(chain: &chain::Chain, raw: &[u8]) -> bool {
    for _ in 0..3 {
        if chain.broadcast(raw.to_vec()).await.is_ok() {
            return true;
        }
    }
    false
}

/// Published in /v1/info so a judge can compare the running binary with a local build.
fn binary_sha256() -> Result<String> {
    use sha2::Digest as _;
    let bytes = std::fs::read(std::env::current_exe()?)?;
    Ok(hex::encode(sha2::Sha256::digest(&bytes)))
}

/// Coarse code only: library errors can carry heights or values.
fn error_code(e: &anyhow::Error) -> &'static str {
    let s = format!("{e:#}");
    if s.contains("transport") || s.contains("status") || s.contains("connect") {
        "read_source"
    } else if s.contains("scan") {
        "scan"
    } else if s.contains("build") || s.contains("anchor") || s.contains("witness") {
        "build"
    } else {
        "tick"
    }
}

fn unsafe_set_sqlite_tmp(dir: &std::path::Path) {
    // SAFETY: called once at startup before any other thread reads the environment.
    unsafe { std::env::set_var("SQLITE_TMPDIR", dir) };
}
