//! `amnesia-lab serve`: the demo backend. Runs the exchange bot and the baseline desk around
//! the clock, and serves the judge actions (donate, seize) behind a small JSON API.
//!
//! Everything here is the simulated exchange and the attacker. It never runs inside the desk.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use amnesia_core::{clock, rng::uniform_u64};
use amnesia_desk::{
    chain::Chain,
    desk::now,
    wallet,
};
use anyhow::{Context, Result, anyhow};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path as UrlPath, Request, State, rejection::JsonRejection},
    http::{HeaderMap, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use rand::{
    rand_core::{Rng as _, UnwrapErr},
    rngs::SysRng,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use zcash_client_backend::data_api::WalletRead;
use zcash_keys::keys::UnifiedSpendingKey;

use crate::{
    analyzer,
    forget::{self, EPOCHS_FILE},
    intake_client,
    limits::{Limits, Rule, client_key},
    ops, records,
    seedwallet::SeedWallet,
};

const FEE: u64 = amnesia_core::FEE_2_ACTIONS;

pub struct Config {
    pub root: PathBuf,
    pub addr: String,
    pub intake: String,
    /// Desk read API, for the shred-log the lab forgets by.
    pub desk_read: String,
    /// Built judge UI (TanStack Start client dir). Unset: the lab serves the API only.
    pub web_dir: Option<PathBuf>,
    pub seize_sources: Vec<PathBuf>,
    pub donations_per_h: f64,
    /// Decoy withdrawals per hour: cheap (one fee) cover for judge donations when bots are off.
    pub decoys_per_h: f64,
    pub faucet: bool,
    pub bots: bool,
}

impl Config {
    pub fn from_env() -> Self {
        let get = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
        Self {
            root: PathBuf::from(get("AMNESIA_LAB_ROOT", "var/lab")),
            addr: get("AMNESIA_LAB_ADDR", "127.0.0.1:8090"),
            intake: get("AMNESIA_DESK_INTAKE", "127.0.0.1:8081"),
            desk_read: get("AMNESIA_DESK_READ", "127.0.0.1:8082"),
            web_dir: std::env::var("AMNESIA_WEB_DIR").ok().filter(|v| !v.is_empty()).map(PathBuf::from),
            seize_sources: get("AMNESIA_SEIZE_SOURCES", "var/desk")
                .split(',')
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .collect(),
            donations_per_h: get("AMNESIA_LAB_DONATIONS_PER_H", "1.5").parse().unwrap_or(1.5),
            decoys_per_h: get("AMNESIA_LAB_DECOYS_PER_H", "3").parse().unwrap_or(3.0),
            faucet: get("AMNESIA_FAUCET", "0") == "1",
            bots: get("AMNESIA_LAB_BOTS", "1") == "1",
        }
    }
}

struct Wallets {
    exch: SeedWallet,
    baseline: SeedWallet,
    treasury: SeedWallet,
    elsewhere: SeedWallet,
    exch_address: String,
}

#[derive(Serialize, Clone)]
struct Event {
    t: u64,
    kind: String,
    detail: Value,
}

#[derive(Serialize, Clone, Default)]
struct Run {
    id: String,
    kind: String,
    created: u64,
    events: Vec<Event>,
    done: bool,
    /// A fixed code only; the cause goes to the lab log.
    error: Option<&'static str>,
}

#[derive(Serialize, Deserialize, Clone)]
struct SeizureSummary {
    id: String,
    target: String,
    created: u64,
    files: usize,
    bytes: u64,
    keys_found: usize,
    receipts: usize,
    records_total: usize,
    linked: usize,
    #[serde(default)]
    epochs_in_image: usize,
    #[serde(default)]
    raw_addresses: usize,
    #[serde(default)]
    memory_bytes: u64,
    #[serde(default)]
    amount_sweeps: usize,
    #[serde(default)]
    amount_pinned: usize,
    #[serde(default)]
    amount_ambiguous: usize,
}

#[derive(Clone)]
struct App {
    cfg: Arc<Config>,
    chain: Arc<Chain>,
    w: Arc<Mutex<Wallets>>,
    runs: Arc<std::sync::Mutex<HashMap<String, Run>>>,
    limits: Arc<std::sync::Mutex<Limits>>,
    /// Held for a whole seizure and for each forget pass: one seizure at a time, never judged
    /// half-written.
    seize_lock: Arc<Mutex<()>>,
    /// Attacker CSV per target with its build time, so record downloads skip the wallet lock.
    csv: Arc<Mutex<HashMap<&'static str, (Instant, Bytes)>>>,
    /// Exchange (spendable zats, notes) as of the last sync, so status never waits on the wallets.
    float: Arc<std::sync::Mutex<Option<(u64, usize)>>>,
    downloads: Arc<tokio::sync::Semaphore>,
}

fn dir(cfg: &Config, name: &str) -> PathBuf {
    cfg.root.join(name)
}

/// Creates the three seed wallets on first start and prints the treasury address the desk needs.
pub async fn init_all(cfg: &Config, chain: &Chain) -> Result<String> {
    let mut c = chain.connect().await?;
    for name in ["exchange", "baseline", "treasury", "elsewhere"] {
        let d = dir(cfg, name);
        if !d.join("seed.hex").exists() {
            SeedWallet::init(&d, &mut c).await?;
        }
    }
    let mut t = SeedWallet::open(&dir(cfg, "treasury"))?;
    let path = dir(cfg, "treasury.ua");
    if let Ok(ua) = fs::read_to_string(&path) {
        return Ok(ua.trim().to_string());
    }
    let ua = t.next_address()?;
    fs::write(path, &ua)?;
    Ok(ua)
}

pub async fn serve(cfg: Config) -> Result<()> {
    let tor = std::env::var("AMNESIA_TOR").ok().filter(|v| v == "1").map(|_| cfg.root.join("arti"));
    let host = std::env::var("AMNESIA_LWD_HOST").unwrap_or_else(|_| "testnet.zec.rocks".into());
    // A Mac that wakes without network must not crash-loop the lab: wait for the read source.
    let chain = loop {
        let r = async {
            let chain = Chain::new(&host, tor.as_deref()).await?;
            init_all(&cfg, &chain).await?;
            anyhow::Ok(chain)
        }
        .await;
        match r {
            Ok(c) => break Arc::new(c),
            Err(e) => {
                fail("lab_start_retry", &e);
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        }
    };
    let mut exch = SeedWallet::open(&dir(&cfg, "exchange"))?;
    let exch_address = match fs::read_to_string(dir(&cfg, "exchange.ua")) {
        Ok(a) => a.trim().to_string(),
        Err(_) => {
            let a = exch.next_address()?;
            fs::write(dir(&cfg, "exchange.ua"), &a)?;
            a
        }
    };
    let w = Wallets {
        exch,
        baseline: SeedWallet::open(&dir(&cfg, "baseline"))?,
        treasury: SeedWallet::open(&dir(&cfg, "treasury"))?,
        elsewhere: SeedWallet::open(&dir(&cfg, "elsewhere"))?,
        exch_address,
    };
    fs::create_dir_all(dir(&cfg, "seizures"))?;
    let app = App {
        cfg: Arc::new(cfg),
        chain,
        w: Arc::new(Mutex::new(w)),
        runs: Default::default(),
        limits: Default::default(),
        seize_lock: Default::default(),
        csv: Default::default(),
        float: Default::default(),
        downloads: Arc::new(tokio::sync::Semaphore::new(4)),
    };
    tokio::spawn(sync_loop(app.clone()));
    tokio::spawn(forget_loop(app.clone()));
    if app.cfg.bots {
        tokio::spawn(bot_loop(app.clone()));
        tokio::spawn(auto_seize_loop(app.clone()));
    }
    if app.cfg.faucet {
        tokio::spawn(faucet_loop(app.clone()));
    }
    let router = Router::new()
        .route("/demo/status", get(status))
        .route("/demo/history", get(history))
        .route("/demo/donate", post(donate))
        .route("/demo/seize", post(seize))
        .route("/demo/runs/{id}", get(run))
        .route("/demo/seizures", get(seizures))
        .route("/demo/seizures/{id}/report.json", get(seizure_report))
        .route("/demo/seizures/{id}/bundle.tar", get(seizure_bundle))
        .route("/demo/records/{target}", get(records_csv))
        .with_state(app.clone());
    let router = match &app.cfg.web_dir {
        Some(dir) => {
            let site = crate::web::Site::load(dir)?;
            router.fallback(move |uri: axum::http::Uri, headers: HeaderMap| {
                let site = site.clone();
                let gzip = crate::web::accepts_gzip(&headers);
                async move { site.serve(&uri, gzip).await }
            })
        }
        None => router,
    }
    .layer(middleware::from_fn(require_demo_header));
    let listener = tokio::net::TcpListener::bind(&app.cfg.addr).await?;
    println!("{{\"kind\":\"lab_ready\"}}");
    axum::serve(listener, router).await?;
    Ok(())
}

async fn sync_all(app: &App) -> Result<()> {
    let mut c = app.chain.connect().await?;
    let mut w = app.w.lock().await;
    let root = &app.cfg.root;
    w.exch.sync(&mut c, &root.join("exchange")).await?;
    w.baseline.sync(&mut c, &root.join("baseline")).await?;
    w.treasury.sync(&mut c, &root.join("treasury")).await?;
    w.elsewhere.sync(&mut c, &root.join("elsewhere")).await?;
    // Faucet payouts arrive as transparent outputs: record and shield them into Ironwood.
    if w.exch.refresh_utxos(&mut c).await? > 0
        && let Some(txid) = w.exch.shield(&app.chain).await?
    {
        println!("{}", json!({ "kind": "faucet_shielded", "txid": txid.to_string() }));
    }
    let float = (w.exch.spendable()?, exchange_notes(&w)?);
    *app.float.lock().unwrap_or_else(|e| e.into_inner()) = Some(float);
    Ok(())
}

/// Lab-only: the cause is logged so the operator can act. The desk never logs error text.
fn fail(kind: &str, e: &anyhow::Error) {
    let err: String = format!("{e:#}").chars().take(300).collect();
    println!("{}", json!({ "kind": kind, "err": err }));
}

async fn sync_loop(app: App) {
    let mut last_recycle: Option<Instant> = None;
    loop {
        // Bounded: a stream stalled by a network drop must not freeze the lab (and its float).
        match tokio::time::timeout(Duration::from_secs(900), sync_all(&app)).await {
            Ok(Err(e)) => fail("lab_sync_failed", &e),
            Err(_) => fail("lab_sync_failed", &anyhow!("sync timed out after 900 s")),
            Ok(Ok(())) => {}
        }
        for target in TARGETS {
            if let Err(e) = refresh_csv(&app, target).await {
                fail("lab_records_failed", &e);
            }
        }
        if last_recycle.is_none_or(|t| t.elapsed() > Duration::from_secs(30 * 60)) {
            last_recycle = Some(Instant::now());
            if let Err(e) = recycle_treasury(&app).await {
                fail("lab_recycle_failed", &e);
            }
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

const TARGETS: [&str; 2] = ["amnesia", "baseline"];
const CSV_TTL: Duration = Duration::from_secs(60);

async fn refresh_csv(app: &App, target: &'static str) -> Result<Bytes> {
    let csv = {
        let w = app.w.lock().await;
        Bytes::from(ops::records_csv(&app.cfg.root.join("exchange"), &w.exch, target)?)
    };
    app.csv.lock().await.insert(target, (Instant::now(), csv.clone()));
    Ok(csv)
}

/// At most one rebuild per minute. A stale copy is served rather than queueing behind a
/// donation that holds the wallet for minutes.
async fn cached_csv(app: &App, target: &'static str) -> Result<Bytes> {
    let mut cache = app.csv.lock().await;
    let stale = match cache.get(target) {
        Some((t, csv)) if t.elapsed() < CSV_TTL => return Ok(csv.clone()),
        Some((_, csv)) => Some(csv.clone()),
        None => None,
    };
    let w = match (app.w.try_lock(), stale) {
        (Ok(w), _) => w,
        (Err(_), Some(csv)) => return Ok(csv),
        (Err(_), None) => app.w.lock().await,
    };
    let csv = Bytes::from(ops::records_csv(&app.cfg.root.join("exchange"), &w.exch, target)?);
    drop(w);
    cache.insert(target, (Instant::now(), csv.clone()));
    Ok(csv)
}

/// Every minute: forget seizures holding epochs the desk has shredded or lost, so no copy on
/// the host outlives the desk's own erasure.
/// Longer than any demo epoch lives (2 h open, 2 h grace, sweep, 600-block burial is about
/// 18 h), so an expired seizure can only hold epochs the desk has already shredded.
const SEIZURE_MAX_AGE: Duration = Duration::from_secs(24 * 3600);

async fn forget_loop(app: App) {
    loop {
        let closed = closed_epochs(&app.cfg.desk_read).await;
        if let Err(e) = &closed {
            fail("lab_shred_log_failed", e);
        }
        let dir = app.cfg.root.join("seizures");
        let guard = app.seize_lock.clone().lock_owned().await;
        let r = tokio::task::spawn_blocking(move || {
            let out = forget::forget_seizures(&dir, closed.ok().as_ref(), SEIZURE_MAX_AGE);
            drop(guard);
            out
        })
        .await;
        match r {
            Ok((n, err)) => {
                if n > 0 {
                    println!("{}", json!({ "kind": "lab_seizures_forgotten", "count": n }));
                }
                if let Some(e) = err {
                    fail("lab_forget_failed", &e);
                }
            }
            Err(e) => fail("lab_forget_failed", &e.into()),
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

async fn closed_epochs(desk_read: &str) -> Result<std::collections::HashSet<String>> {
    let req = format!("GET /v1/shred-log HTTP/1.1\r\nHost: {desk_read}\r\nConnection: close\r\n\r\n");
    let (code, body) = tokio::time::timeout(Duration::from_secs(10), intake_client::http(desk_read, req))
        .await
        .context("shred-log timed out")??;
    if code != 200 {
        return Err(anyhow!("shred-log returned {code}"));
    }
    forget::closed_epochs(&body)
}

/// Sends swept treasury funds and decoy receipts back to the exchange so the bots never run
/// dry. Demo-only.
async fn recycle_treasury(app: &App) -> Result<()> {
    let mut w = app.w.lock().await;
    let to = w.exch_address.clone();
    // The baseline keeps its receipt history after spending, which is what the attack reads.
    let Wallets { treasury, elsewhere, baseline, .. } = &mut *w;
    for (name, wallet) in [("treasury", treasury), ("elsewhere", elsewhere), ("baseline", baseline)] {
        let r = async {
            let spendable = wallet.spendable()?;
            let notes = wallet::spendable_notes(&wallet.db, wallet.account, &[])?.len() as u64;
            // ZIP 317: 5,000 per action; inputs plus a payment and a change output.
            let fee = 5_000 * (notes + 2).max(2);
            if spendable >= 500_000 + fee {
                wallet.pay(&app.chain, &to, spendable - fee).await?;
            }
            anyhow::Ok(())
        }
        .await;
        if let Err(e) = r {
            fail(&format!("lab_recycle_{name}_failed"), &e);
        }
    }
    Ok(())
}

fn exchange_notes(w: &Wallets) -> Result<usize> {
    Ok(wallet::spendable_notes(&w.exch.db, w.exch.account, &[])?.len())
}

/// Keeps several spendable exchange notes so a donation's two withdrawals do not wait on
/// change. Half the balance is fanned out, in notes no smaller than the largest donation.
async fn ensure_float(app: &App, w: &mut Wallets) -> Result<()> {
    let notes = exchange_notes(w)?;
    let bal = w.exch.spendable()?;
    // Split early and wide: the split spends the big note, so its change is pending for a few blocks.
    let n = (bal / 2 / 400_000).min(16) as usize;
    if notes < 6 && n >= 2 {
        ops::split(&app.chain, &mut w.exch, n, bal / 2 / n as u64).await?;
    }
    Ok(())
}

const BOT_RESERVE_ZATS: u64 = 1_500_000;

fn client_name() -> String {
    format!("c{:02}", uniform_u64(&mut UnwrapErr(SysRng), 1, 40))
}

fn amount() -> u64 {
    // 0.001 to 0.01 TAZ, the demo donation band (spec 12 §7.1).
    uniform_u64(&mut UnwrapErr(SysRng), 100_000, 1_000_000) / 1000 * 1000
}

async fn bot_loop(app: App) {
    let rate = |per_h: f64| ((per_h * 100.0).round() as u32).max(1);
    let rng = &mut UnwrapErr(SysRng);
    // Exponential gaps at 100x the hourly rate, scaled back: keeps fractional rates.
    let gap = |per_h: f64, rng: &mut UnwrapErr<SysRng>| clock::exp_gap_s(rng, rate(per_h)) * 100;
    let mut next_donation = now() + gap(app.cfg.donations_per_h, rng);
    let mut next_decoy = now() + gap(app.cfg.decoys_per_h, rng);
    loop {
        tokio::time::sleep(Duration::from_secs(15)).await;
        let t = now();
        let mut w = app.w.lock().await;
        if let Err(e) = ensure_float(&app, &mut w).await {
            fail("lab_float_failed", &e);
        }
        // Bots stop short of the reserve so a judge's donation always has float.
        let low = w.exch.spendable().map_or(true, |b| b < BOT_RESERVE_ZATS);
        if low && (t >= next_donation || t >= next_decoy) {
            println!("{}", json!({ "kind": "bot_paused_low_float" }));
            next_donation = next_donation.max(t + gap(app.cfg.donations_per_h, rng));
            next_decoy = next_decoy.max(t + gap(app.cfg.decoys_per_h, rng));
        }
        // A rate of 0 turns that bot off (each bot donation costs about ten fees end to end).
        if !low && app.cfg.donations_per_h > 0.0 && t >= next_donation {
            next_donation = t + gap(app.cfg.donations_per_h, rng);
            // Bots share the judges' hourly cap: together they must not fill one epoch.
            if app.limits.lock().unwrap_or_else(|e| e.into_inner()).donations.try_take(Instant::now()) {
                let Wallets { exch, baseline, .. } = &mut *w;
                let r = ops::donate(&app.chain, &app.cfg.root.join("exchange"), exch, baseline, &app.cfg.intake, &client_name(), amount()).await;
                if let Err(e) = r {
                    fail("bot_donation_failed", &e);
                }
            } else {
                println!("{}", json!({ "kind": "bot_paused_donation_cap" }));
            }
        }
        if !low && app.cfg.decoys_per_h > 0.0 && t >= next_decoy {
            next_decoy = t + gap(app.cfg.decoys_per_h, rng);
            let Wallets { exch, elsewhere, .. } = &mut *w;
            let r = ops::decoy(&app.chain, &app.cfg.root.join("exchange"), exch, Some(elsewhere), &client_name(), amount()).await;
            if let Err(e) = r {
                fail("bot_decoy_failed", &e);
            }
        }
    }
}

/// Valar Testnet faucet: one claim per IP per 24 h. Demo float only.
const FAUCET: &str = "https://faucet.testnet.valargroup.dev/api/claim";

async fn curl_json(args: &[&str]) -> Result<Value> {
    let out = tokio::process::Command::new("curl").args(["-s", "-m", "30"]).args(args).output().await?;
    Ok(serde_json::from_slice(&out.stdout)?)
}

/// Valar Testnet faucet: 0.125 TAZ per address and per IP each 24 h, to a transparent address
/// (it cannot pay Ironwood-only ones). `sync_all` shields what arrives. Demo float only.
async fn faucet_loop(app: App) {
    loop {
        let to = { app.w.lock().await.exch.transparent_address() };
        let result = async {
            let to = to?;
            let body = json!({ "address": to }).to_string();
            let claim = curl_json(&["-X", "POST", "-H", "Content-Type: application/json", "-d", &body, FAUCET]).await?;
            let id = claim["id"].as_str().ok_or(anyhow!("faucet: {claim}"))?.to_string();
            // Poll the claim until the faucet broadcasts or rejects it.
            for _ in 0..40 {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let st = curl_json(&[&format!("{FAUCET}/{id}")]).await?;
                match st["status"].as_str() {
                    Some("queued") | Some("pending") | None => continue,
                    Some(s) => return anyhow::Ok(json!({ "status": s, "message": st["message"] })),
                }
            }
            anyhow::Ok(json!({ "status": "timeout" }))
        }
        .await;
        match result {
            Ok(v) => println!("{}", json!({ "kind": "faucet_claim", "result": v })),
            Err(e) => fail("faucet_claim_failed", &e),
        }
        tokio::time::sleep(Duration::from_secs(24 * 3600 + 600)).await;
    }
}

fn new_id() -> String {
    let mut b = [0u8; 8];
    UnwrapErr(SysRng).fill_bytes(&mut b);
    hex::encode(b)
}

fn push(app: &App, id: &str, kind: &str, detail: Value) {
    if let Some(r) = app.runs.lock().expect("runs lock").get_mut(id) {
        r.events.push(Event { t: now(), kind: kind.into(), detail });
    }
}

fn finish(app: &App, id: &str, err: Option<&'static str>) {
    if let Some(r) = app.runs.lock().expect("runs lock").get_mut(id) {
        r.done = true;
        r.error = err;
    }
}

/// Global and per-client cooldowns, so a public demo cannot drain the float or flood the desk.
/// Returns the 429 to send, if any.
fn admit(app: &App, rule: &Rule, headers: &HeaderMap) -> Option<Response> {
    let r = app.limits.lock().expect("limits lock").admit(Instant::now(), rule, &client_key(headers));
    r.err().map(|code| err(StatusCode::TOO_MANY_REQUESTS, code))
}

/// CSRF guard: a custom header forces a CORS preflight, which fails because the lab sends no
/// CORS headers. Applies to every method that is not a plain read.
async fn require_demo_header(req: Request, next: Next) -> Response {
    let read = matches!(*req.method(), Method::GET | Method::HEAD);
    if !read && req.headers().get("x-amnesia-demo").is_none_or(|v| v.as_bytes() != b"1") {
        return err(StatusCode::FORBIDDEN, "missing_demo_header");
    }
    next.run(req).await
}

fn err(code: StatusCode, msg: &str) -> Response {
    (code, Json(json!({ "error": msg }))).into_response()
}

fn start_run(app: &App, kind: &str) -> String {
    let id = new_id();
    let run = Run { id: id.clone(), kind: kind.into(), created: now(), ..Default::default() };
    let mut runs = app.runs.lock().expect("runs lock");
    let cutoff = now().saturating_sub(24 * 3600);
    runs.retain(|_, r| r.created >= cutoff);
    runs.insert(id.clone(), run);
    id
}

const DONATE: Rule = Rule { global: "donate", global_secs: 20, client: "donate", client_secs: 20, donation: true };

async fn donate(State(app): State<App>, headers: HeaderMap) -> Response {
    if let Some(r) = admit(&app, &DONATE, &headers) {
        return r;
    }
    let id = start_run(&app, "donate");
    let rid = id.clone();
    tokio::spawn(async move {
        let r = donate_run(&app, &rid).await;
        if let Err(e) = &r {
            fail("lab_donation_failed", e);
        }
        finish(&app, &rid, r.err().map(|_| "donation_failed"));
    });
    Json(json!({ "run_id": id })).into_response()
}

fn append_history(root: &Path, s: &SeizureSummary) -> Result<()> {
    use std::io::Write as _;
    let mut f = fs::OpenOptions::new().create(true).append(true).open(root.join("history.jsonl"))?;
    writeln!(f, "{}", serde_json::to_string(s)?)?;
    Ok(())
}

/// Every seizure ever run, newest first, as counts only.
async fn history(State(app): State<App>) -> Response {
    let text = fs::read_to_string(app.cfg.root.join("history.jsonl")).unwrap_or_default();
    let mut rows: Vec<SeizureSummary> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    rows.reverse();
    rows.truncate(500);
    Json(json!({ "seizures": rows })).into_response()
}

/// Up to 10 minutes for pending change to confirm before a judge donation gives up.
const FLOAT_WAIT_TRIES: u32 = 40;

async fn donate_run(app: &App, id: &str) -> Result<()> {
    let client = format!("you-{}", &id[..4]);
    let zats = amount();
    // Two withdrawals of `zats`, each with a fee. Change from earlier sends confirms within a few blocks.
    let need = 2 * (zats + FEE);
    let mut waited = false;
    for _ in 0..FLOAT_WAIT_TRIES {
        if app.w.lock().await.exch.spendable()? >= need {
            break;
        }
        if !waited {
            push(app, id, "float_wait", json!({}));
            waited = true;
        }
        tokio::time::sleep(Duration::from_secs(15)).await;
    }
    let mut w = app.w.lock().await;
    let Wallets { exch, baseline, .. } = &mut *w;
    let d = ops::donate(&app.chain, &app.cfg.root.join("exchange"), exch, baseline, &app.cfg.intake, &client, zats).await?;
    drop(w);
    push(app, id, "issued", json!({ "client": client, "value_zats": zats, "amnesia_address": d.amnesia_address, "valid_until": d.valid_until, "receipt": d.receipt }));
    push(app, id, "broadcast", json!({ "amnesia_txid": d.amnesia_txid.to_string(), "baseline_txid": d.baseline_txid.to_string() }));
    for _ in 0..60 {
        tokio::time::sleep(Duration::from_secs(15)).await;
        let (a, b) = {
            let w = app.w.lock().await;
            (w.exch.db.get_tx_height(d.amnesia_txid)?, w.exch.db.get_tx_height(d.baseline_txid)?)
        };
        if let (Some(a), Some(b)) = (a, b) {
            push(app, id, "mined", json!({ "amnesia_height": u32::from(a), "baseline_height": u32::from(b) }));
            return Ok(());
        }
    }
    Err(anyhow!("not mined in 15 minutes"))
}

#[derive(Deserialize)]
struct SeizeReq {
    target: String,
}

/// Keeps a fresh seizure pair on hand, so the judge UI always has a real report to show.
/// Reports still die with their epochs (forget loop); this only replaces them. Costs no TAZ.
async fn auto_seize_loop(app: App) {
    loop {
        for target in ["amnesia", "baseline"] {
            // Wait for any judge-run seizure instead of competing with it.
            let guard = app.seize_lock.clone().lock_owned().await;
            let id = start_run(&app, "seize");
            let r = seize_run(&app, &id, target).await;
            drop(guard);
            if let Err(e) = &r {
                fail("lab_auto_seizure_failed", e);
            }
            finish(&app, &id, r.err().map(|_| "seizure_failed"));
        }
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}

async fn seize(State(app): State<App>, headers: HeaderMap, req: Result<Json<SeizeReq>, JsonRejection>) -> Response {
    let target = match req.as_ref().map(|r| r.target.as_str()) {
        Ok("amnesia") => "amnesia",
        Ok("baseline") => "baseline",
        _ => return err(StatusCode::BAD_REQUEST, "target is amnesia or baseline"),
    };
    // Lock before the cooldowns, so a busy reply does not start the caller's cooldown.
    let Ok(guard) = app.seize_lock.clone().try_lock_owned() else {
        return err(StatusCode::TOO_MANY_REQUESTS, "seizure_in_progress");
    };
    let global = if target == "amnesia" { "seize_a" } else { "seize_b" };
    if let Some(r) = admit(&app, &Rule { global, global_secs: 15, client: "seize", client_secs: 15, donation: false }, &headers) {
        return r;
    }
    let id = start_run(&app, "seize");
    let rid = id.clone();
    tokio::spawn(async move {
        let r = seize_run(&app, &rid, target).await;
        drop(guard);
        if let Err(e) = &r {
            fail("lab_seizure_failed", e);
        }
        finish(&app, &rid, r.err().map(|_| "seizure_failed"));
    });
    Json(json!({ "run_id": id })).into_response()
}

/// Copies whatever bytes exist right now. Files that vanish mid-copy (a shred) are skipped.
fn copy_tree(src: &Path, dst: &Path, files: &mut usize, bytes: &mut u64) {
    copy_tree_except(src, dst, &|_| false, files, bytes);
}

/// `copy_tree`, leaving out every entry whose name matches `skip`, at any depth.
fn copy_tree_except(src: &Path, dst: &Path, skip: &dyn Fn(&str) -> bool, files: &mut usize, bytes: &mut u64) {
    let Ok(rd) = fs::read_dir(src) else { return };
    let _ = fs::create_dir_all(dst);
    for e in rd.flatten() {
        if skip(&e.file_name().to_string_lossy()) {
            continue;
        }
        let p = e.path();
        let to = dst.join(e.file_name());
        match fs::symlink_metadata(&p) {
            Ok(m) if m.is_dir() => copy_tree_except(&p, &to, skip, files, bytes),
            Ok(m) if m.is_file() => {
                if let Ok(n) = fs::copy(&p, &to) {
                    *files += 1;
                    *bytes += n;
                }
            }
            _ => {}
        }
    }
}

/// Shreds a seizure's raw working copies (seeds, RAM dumps) however `seize_run` returns, and the
/// whole seizure dir unless the run completed.
struct SeizeScratch {
    base: PathBuf,
    done: bool,
}

impl Drop for SeizeScratch {
    fn drop(&mut self) {
        let paths = if self.done { vec![self.base.join("image"), self.base.join("public")] } else { vec![self.base.clone()] };
        for p in paths {
            if let Err(e) = forget::shred_tree(&p) {
                fail("lab_seize_cleanup_failed", &e);
                let _ = fs::remove_dir_all(&p);
            }
        }
    }
}

async fn seize_run(app: &App, id: &str, target: &str) -> Result<()> {
    let base = app.cfg.root.join("seizures").join(id);
    let image = base.join("image");
    let mut scratch = SeizeScratch { base: base.clone(), done: false };
    fs::create_dir_all(&image)?;
    let (mut files, mut bytes) = (0usize, 0u64);
    // The seizure takes every byte the organisation holds, including the treasury seed.
    let sources: Vec<(String, PathBuf)> = if target == "amnesia" {
        let mut s: Vec<_> = app
            .cfg
            .seize_sources
            .iter()
            .enumerate()
            .map(|(i, p)| (format!("host{i}"), p.clone()))
            .collect();
        s.push(("treasury".into(), app.cfg.root.join("treasury")));
        s
    } else {
        vec![("baseline".into(), app.cfg.root.join("baseline"))]
    };
    {
        let _w = app.w.lock().await;
        for (name, src) in &sources {
            copy_tree(src, &image.join(name), &mut files, &mut bytes);
        }
    }
    // What the forget pass deletes this seizure by once the desk shreds any of these epochs.
    let epochs = forget::epoch_ids_in_tree(&image)?;
    fs::write(base.join(EPOCHS_FILE), serde_json::to_vec(&epochs)?)?;
    if target == "amnesia" && std::env::var("AMNESIA_SEIZE_MEMORY").is_ok_and(|v| v == "1") {
        let out = image.join("memory");
        fs::create_dir_all(&out)?;
        let captured = crate::memdump::find_pid("amnesia-desk")
            .ok_or(anyhow!("desk process not visible"))
            .and_then(|pid| crate::memdump::dump(pid, &out.join("desk.mem")));
        match captured {
            Ok(n) => {
                files += 1;
                bytes += n;
                push(app, id, "memory_captured", json!({ "bytes": n }));
            }
            Err(e) => {
                fail("lab_memory_unavailable", &e);
                push(app, id, "memory_unavailable", json!({ "reason": "memory_unavailable" }));
            }
        }
    }
    let manifest = manifest(&image)?;
    fs::write(base.join("manifest.txt"), &manifest)?;
    push(app, id, "bundle_ready", json!({ "files": files, "bytes": bytes, "includes_treasury_seed": target == "amnesia" }));
    let csv = {
        let w = app.w.lock().await;
        ops::records_csv(&app.cfg.root.join("exchange"), &w.exch, target)?
    };
    fs::write(base.join("records.csv"), &csv)?;
    push(app, id, "scanning", json!({}));
    let rows = records::parse_csv(&csv);
    let img = image.clone();
    let report = tokio::task::spawn_blocking(move || analyzer::analyze(&img, &rows)).await??;
    fs::write(base.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    let summary = SeizureSummary {
        id: id.into(),
        target: target.into(),
        created: now(),
        files: report.files_scanned,
        bytes: report.bytes_scanned,
        keys_found: report.keyring_keys + report.seed_files,
        receipts: report.receipts.len(),
        records_total: report.records_total,
        linked: report.linked.len(),
        epochs_in_image: forget::seizure_epochs(&base).map_or(0, |e| e.len()),
        raw_addresses: report.raw_addresses,
        memory_bytes: report.memory_bytes,
        amount_sweeps: report.amount_attack.as_ref().map_or(0, |a| a.sweeps),
        amount_pinned: report.amount_attack.as_ref().map_or(0, |a| a.pinned),
        amount_ambiguous: report.amount_attack.as_ref().map_or(0, |a| a.ambiguous),
    };
    fs::write(base.join("summary.json"), serde_json::to_vec(&summary)?)?;
    // Counts only, no address or key: outlives the seizure itself, which is forgotten with
    // its epochs, so judges can see every past result.
    append_history(&app.cfg.root, &summary)?;
    build_public_bundle(&base)?;
    // Keep the report, manifest and bundle; the working copies are large and hold raw keys.
    scratch.done = true;
    drop(scratch);
    // Seizures sit on a 256 MB RAM disk on the Mac host (they hold live epoch copies): keep few.
    prune_seizures(&app.cfg.root.join("seizures"), 4);
    push(app, id, "report", serde_json::to_value(&summary)?);
    Ok(())
}

/// Keeps the newest `keep` seizures so public demo clicks cannot fill the disk.
fn prune_seizures(dir: &Path, keep: usize) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut all: Vec<_> = rd
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    all.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, p) in all.into_iter().skip(keep) {
        if let Err(e) = forget::shred_tree(&p) {
            fail("lab_prune_failed", &e);
        }
    }
}

/// SHA-256 of every file in the image, so a downloaded bundle can be checked.
fn manifest(image: &Path) -> Result<String> {
    let mut paths = vec![];
    collect(image, &mut paths);
    paths.sort();
    let mut out = String::new();
    for p in paths {
        let b = fs::read(&p)?;
        let rel = p.strip_prefix(image).unwrap_or(&p).display().to_string();
        out.push_str(&format!("{}  {}  {}\n", hex::encode(Sha256::digest(&b)), b.len(), rel));
    }
    Ok(out)
}

fn collect(d: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(d) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// The downloadable bundle swaps spending seeds for full viewing keys: identical linking power,
/// but a visitor cannot drain the demo's Testnet funds (spec 12 §4.7). On error the stage and
/// any partial tar are removed.
fn build_public_bundle(base: &Path) -> Result<()> {
    let public = base.join("public");
    let r = stage_and_pack(base, &public);
    if r.is_err() {
        if let Err(e) = forget::shred_tree(&public) {
            fail("lab_stage_cleanup_failed", &e);
        }
        let _ = fs::remove_file(base.join("bundle.tar"));
    }
    r
}

fn stage_and_pack(base: &Path, public: &Path) -> Result<()> {
    let (mut f, mut b) = (0, 0);
    // Raw RAM holds live spending keys: analysed here, published only as a hash. The Tor
    // directory cache is public consensus data and only bloats the download. Key files are
    // staged only to be swapped for viewing keys below.
    let skip = |n: &str| forget::forbidden_name(n) && !matches!(n, "keyring.bin" | "seed.hex");
    copy_tree_except(&base.join("image"), &public.join("image"), &skip, &mut f, &mut b);
    let mut paths = vec![];
    collect(public, &mut paths);
    for p in paths {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        let seeds: Vec<Vec<u8>> = match name.as_str() {
            "keyring.bin" => keyring_seeds(&fs::read(&p)?),
            "seed.hex" => vec![hex::decode(fs::read_to_string(&p)?.trim())?],
            _ => continue,
        };
        let mut ufvks = String::new();
        for s in seeds {
            let usk = UnifiedSpendingKey::from_seed(&amnesia_desk::PARAMS, &s, zip32::AccountId::ZERO)?;
            ufvks.push_str(&usk.to_unified_full_viewing_key().encode(&amnesia_desk::PARAMS)?);
            ufvks.push('\n');
        }
        fs::write(p.with_file_name(format!("{name}.ufvk.txt")), ufvks)?;
        forget::shred_tree(&p)?;
    }
    fs::copy(base.join("records.csv"), public.join("records.csv"))?;
    fs::copy(base.join("manifest.txt"), public.join("manifest-raw.txt"))?;
    // Last line of defence: whatever the steps above missed, these names never ship.
    forget::check_publishable(public)?;
    let status = std::process::Command::new("tar")
        .arg("-cf")
        .arg(base.join("bundle.tar"))
        .arg("-C")
        .arg(public)
        .arg(".")
        .status()
        .context("tar")?;
    if !status.success() {
        return Err(anyhow!("tar failed"));
    }
    Ok(())
}

fn keyring_seeds(bytes: &[u8]) -> Vec<Vec<u8>> {
    let (mut i, mut out) = (0, vec![]);
    while i + 50 <= bytes.len() {
        out.push(bytes[i + 16..i + 48].to_vec());
        let len = u16::from_be_bytes([bytes[i + 48], bytes[i + 49]]) as usize;
        i += 50 + len;
    }
    out
}

async fn run(State(app): State<App>, UrlPath(id): UrlPath<String>) -> Response {
    match app.runs.lock().expect("runs lock").get(&id) {
        Some(r) => Json(r.clone()).into_response(),
        None => err(StatusCode::NOT_FOUND, "unknown run"),
    }
}

fn valid_id(id: &str) -> bool {
    id.len() == 16 && id.chars().all(|c| c.is_ascii_hexdigit())
}

async fn seizures(State(app): State<App>) -> Response {
    let mut out: Vec<SeizureSummary> = vec![];
    if let Ok(rd) = fs::read_dir(app.cfg.root.join("seizures")) {
        for e in rd.flatten() {
            if let Ok(b) = fs::read(e.path().join("summary.json"))
                && let Ok(s) = serde_json::from_slice(&b)
            {
                out.push(s);
            }
        }
    }
    out.sort_by_key(|s| std::cmp::Reverse(s.created));
    out.truncate(50);
    Json(json!({ "seizures": out })).into_response()
}

async fn seizure_report(State(app): State<App>, UrlPath(id): UrlPath<String>) -> Response {
    if !valid_id(&id) {
        return err(StatusCode::BAD_REQUEST, "bad id");
    }
    match fs::read(app.cfg.root.join("seizures").join(&id).join("report.json")) {
        Ok(b) => ([(header::CONTENT_TYPE, "application/json")], b).into_response(),
        Err(_) => err(StatusCode::NOT_FOUND, "unknown seizure"),
    }
}

async fn seizure_bundle(State(app): State<App>, UrlPath(id): UrlPath<String>) -> Response {
    if !valid_id(&id) {
        return err(StatusCode::BAD_REQUEST, "bad id");
    }
    // Bundles are about 100 MB: stream them in 64 KiB chunks, at most 4 at a time, so a few
    // concurrent judges cannot push the host into swap.
    let Ok(permit) = app.downloads.clone().try_acquire_owned() else {
        return err(StatusCode::TOO_MANY_REQUESTS, "downloads_busy");
    };
    let path = app.cfg.root.join("seizures").join(&id).join("bundle.tar");
    let Ok(file) = tokio::fs::File::open(&path).await else {
        return err(StatusCode::NOT_FOUND, "unknown seizure");
    };
    let len = file.metadata().await.map(|m| m.len()).unwrap_or(0);
    let stream = futures_util::stream::unfold((file, permit), |(mut f, permit)| async move {
        use tokio::io::AsyncReadExt as _;
        let mut buf = vec![0u8; 64 * 1024];
        match f.read(&mut buf).await {
            Ok(0) => None,
            Ok(n) => {
                buf.truncate(n);
                Some((Ok::<_, std::io::Error>(buf), (f, permit)))
            }
            Err(e) => Some((Err(e), (f, permit))),
        }
    });
    (
        [
            (header::CONTENT_TYPE, "application/x-tar".to_string()),
            (header::CONTENT_LENGTH, len.to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"amnesia-seizure-{id}.tar\"")),
        ],
        axum::body::Body::from_stream(stream),
    )
        .into_response()
}

async fn records_csv(State(app): State<App>, UrlPath(target): UrlPath<String>) -> Response {
    let target = match target.trim_end_matches(".csv") {
        "amnesia" => "amnesia",
        "baseline" => "baseline",
        _ => return err(StatusCode::BAD_REQUEST, "target is amnesia or baseline"),
    };
    match cached_csv(&app, target).await {
        Ok(csv) => ([(header::CONTENT_TYPE, "text/csv")], csv).into_response(),
        Err(e) => {
            fail("lab_records_failed", &e);
            err(StatusCode::SERVICE_UNAVAILABLE, "records_unavailable")
        }
    }
}

async fn status(State(app): State<App>) -> Response {
    let rows = records::load(&app.cfg.root.join("exchange")).unwrap_or_default();
    let count = |k: &str| rows.iter().filter(|r| r.kind == k).count();
    let float = *app.float.lock().unwrap_or_else(|e| e.into_inner());
    let (spendable, notes) = (float.map(|f| f.0), float.map(|f| f.1));
    Json(json!({
        "donations": count("amnesia"),
        "decoys": count("decoy"),
        "exchange_spendable_zats": spendable,
        "exchange_notes": notes,
        "fee": FEE,
    }))
    .into_response()
}
