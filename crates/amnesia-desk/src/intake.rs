//! Donor intake (onion in production) and the public read API. No cookies, no third-party
//! assets, no address in any URL, no per-address lookup.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use amnesia_core::{buckets, pow};
use axum::{
    Form, Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use rand::{
    rand_core::{Rng as _, UnwrapErr},
    rngs::SysRng,
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;

use crate::{
    desk::{Desk, now},
    ledger,
    log, wallet,
};

/// Pinned in rust-toolchain.toml; served so a judge can rebuild with the same compiler.
const TOOLCHAIN: &str = include_str!("../../../rust-toolchain.toml");

const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; img-src data:; \
                   form-action 'self'; base-uri 'none'; frame-ancestors 'none'";
const STYLE: &str = include_str!("../static/style.css");
const POW_JS: &str = include_str!("../static/pow.js");

struct Challenge {
    salt: [u8; 16],
    issued: Instant,
    expires: Instant,
}

/// Unsolved challenges held at once; beyond this the form answers 503 instead of growing.
const MAX_CHALLENGES: usize = 4096;

/// Token bucket for address issuance.
pub struct Bucket {
    tokens: f64,
    burst: f64,
    per_s: f64,
    last: Option<Instant>,
}

impl Bucket {
    /// Refills so a full epoch's cap takes at least half the epoch to hand out, burst 5.
    pub fn for_epoch(cap: u32, epoch_s: u64) -> Self {
        let burst = 5.0_f64.min(f64::from(cap.max(1)));
        Self { tokens: burst, burst, per_s: 2.0 * f64::from(cap.max(1)) / epoch_s.max(1) as f64, last: None }
    }

    pub fn take(&mut self, now: Instant) -> bool {
        if let Some(last) = self.last {
            self.tokens = (self.tokens + now.duration_since(last).as_secs_f64() * self.per_s).min(self.burst);
        }
        self.last = Some(now);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Without JavaScript (Tor Browser "Safest") there is no proof of work: waiting this long on
/// the form is the cost instead.
const NOJS_WAIT: Duration = Duration::from_secs(60);

/// What the read API serves, published by the tick loop so reads never wait on the desk lock.
#[derive(Clone, Default)]
pub struct Snapshot {
    pub synced: bool,
    pub live_epochs: usize,
    pub swept_since_boot: u64,
    /// Unix seconds of the last tick that finished without error.
    pub last_ok_tick: u64,
}

/// Fixed for the life of the process.
pub struct Build {
    pub profile: String,
    pub binary_sha256: String,
    /// The exact dependency set this binary was built from.
    pub cargo_lock_sha256: String,
    pub broadcast: &'static str,
}

/// A tick older than this means the desk is stuck, so intake reports itself closed.
const STALE_S: u64 = 1200;

#[derive(Clone)]
pub struct App {
    pub desk: Arc<Mutex<Desk>>,
    pub snap: Arc<RwLock<Snapshot>>,
    challenges: Arc<std::sync::Mutex<HashMap<[u8; 16], Challenge>>>,
    issuance: Arc<std::sync::Mutex<Bucket>>,
    pow_bits: u32,
    ledger: ledger::Ledger,
    params: amnesia_core::params::Params,
    build: Arc<Build>,
}

impl App {
    pub fn new(desk: Arc<Mutex<Desk>>, d: &Desk, pow_bits: u32, build: Build) -> Self {
        Self {
            desk,
            snap: Default::default(),
            challenges: Default::default(),
            issuance: Arc::new(std::sync::Mutex::new(Bucket::for_epoch(d.p.cap, d.p.epoch_s))),
            pow_bits,
            ledger: d.ledger.clone(),
            params: d.p.clone(),
            build: Arc::new(build),
        }
    }

    pub fn publish(&self, d: &Desk, tick_ok: bool) {
        let mut s = self.snap.write().unwrap_or_else(|e| e.into_inner());
        s.synced = d.synced;
        s.live_epochs = d.epochs.len();
        s.swept_since_boot = d.swept_since_boot;
        if tick_ok {
            s.last_ok_tick = now();
        }
    }

    fn fresh(&self) -> bool {
        let s = self.snapshot();
        s.last_ok_tick > 0 && now().saturating_sub(s.last_ok_tick) < STALE_S
    }

    fn snapshot(&self) -> Snapshot {
        self.snap.read().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

pub fn donor_router(app: App) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/donate", get(donate_form).post(donate))
        .route("/static/style.css", get(|| async { asset("text/css", STYLE) }))
        .route("/static/pow.js", get(|| async { asset("text/javascript", POW_JS) }))
        .fallback(|| async { page(StatusCode::NOT_FOUND, "Not found", "<p>Nothing here.</p>") })
        .with_state(app)
}

pub fn read_router(app: App) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/shred-log", get(shred_log))
        .route("/v1/audit/weeks", get(audit_weeks))
        .route("/v1/treasury", get(treasury))
        .route("/v1/info", get(info))
        .route("/v1/summary", get(summary))
        .with_state(app)
}

fn secure_headers(h: &mut HeaderMap, cache: bool) {
    h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP));
    h.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(header::CONNECTION, HeaderValue::from_static("close"));
    let cc = if cache { "public, max-age=86400, immutable" } else { "no-store" };
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static(cc));
}

fn asset(ct: &'static str, body: &'static str) -> Response {
    let mut r = body.into_response();
    secure_headers(r.headers_mut(), true);
    r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static(ct));
    r
}

fn page(status: StatusCode, title: &str, body: &str) -> Response {
    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{title}</title><link rel=\"stylesheet\" href=\"/static/style.css\"></head>\
         <body><main><h1>{title}</h1>{body}</main></body></html>"
    );
    let mut r = (status, html).into_response();
    secure_headers(r.headers_mut(), false);
    r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
    r
}

fn text(status: StatusCode, body: String) -> Response {
    let mut r = (status, body).into_response();
    secure_headers(r.headers_mut(), false);
    r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain; charset=utf-8"));
    r
}

async fn index() -> Response {
    page(
        StatusCode::OK,
        "Donate privately",
        "<p>This desk gives you a one-time shielded address. A few hours after its deadline, \
         the desk destroys every key that could tell anyone you paid it, including us.</p>\
         <p>The trade-off: a payment that arrives after the deadline is lost for good. \
         That is the price of a desk that cannot betray you.</p>\
         <p>Pay from your own shielded wallet, with no memo. If you pay straight from an \
         exchange, remember that the exchange keeps its own record forever.</p>\
         <p><a href=\"/donate\">Get an address</a></p>",
    )
}

async fn donate_form(State(app): State<App>) -> Response {
    let mut id = [0u8; 16];
    let mut salt = [0u8; 16];
    UnwrapErr(SysRng).fill_bytes(&mut id);
    UnwrapErr(SysRng).fill_bytes(&mut salt);
    // Fixed difficulty: a difficulty that rose with issuance would publish how many donors
    // the open epoch has, to anyone who loads this form.
    let bits = app.pow_bits;
    {
        let mut ch = app.challenges.lock().unwrap_or_else(|e| e.into_inner());
        let t = Instant::now();
        ch.retain(|_, c| c.expires > t);
        if ch.len() >= MAX_CHALLENGES {
            log::event("intake_busy", &[]);
            return page(StatusCode::SERVICE_UNAVAILABLE, "Busy", "<p>Try again in a few minutes.</p>");
        }
        ch.insert(id, Challenge { salt, issued: t, expires: t + Duration::from_secs(600) });
    }
    let body = format!(
        "<form method=\"post\" action=\"/donate\" id=\"f\" data-salt=\"{}\" data-id=\"{}\" data-bits=\"{}\">\
         <input type=\"hidden\" name=\"challenge_id\" value=\"{}\">\
         <input type=\"hidden\" name=\"nonce\" value=\"\">\
         <button type=\"submit\">Show me a one-time address</button></form>\
         <noscript><p>JavaScript is off. That works too: wait one minute on this page, then press the button.</p></noscript>\
         <script src=\"/static/pow.js\" defer></script>",
        hex::encode(salt),
        hex::encode(id),
        bits,
        hex::encode(id),
    );
    page(StatusCode::OK, "Get an address", &body)
}

#[derive(Deserialize)]
struct DonateForm {
    challenge_id: String,
    nonce: String,
}

fn wants_text(h: &HeaderMap) -> bool {
    h.get(header::ACCEPT).and_then(|v| v.to_str().ok()).is_some_and(|v| v.starts_with("text/plain"))
}

fn cross_site(h: &HeaderMap) -> bool {
    h.get("sec-fetch-site")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v != "same-origin" && v != "none")
}

async fn donate(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<DonateForm>, axum::extract::rejection::FormRejection>,
) -> Response {
    // A malformed body gets our own page, so the security headers are never skipped.
    let Ok(Form(f)) = form else {
        return page(StatusCode::BAD_REQUEST, "Something went wrong", "<p>Please start again.</p>");
    };
    let plain = wants_text(&headers);
    if cross_site(&headers) || f.nonce.len() > 64 {
        return page(StatusCode::BAD_REQUEST, "Something went wrong", "<p>Please start again.</p>");
    }
    let Some(id) = hex::decode(&f.challenge_id).ok().and_then(|v| <[u8; 16]>::try_from(v).ok()) else {
        return page(StatusCode::BAD_REQUEST, "Something went wrong", "<p>Please start again.</p>");
    };
    let nojs = f.nonce.is_empty();
    let ch = {
        let mut map = app.challenges.lock().unwrap_or_else(|e| e.into_inner());
        // An early no-JS submit keeps its challenge, so the donor can simply press again.
        if nojs && map.get(&id).is_some_and(|c| c.issued.elapsed() < NOJS_WAIT) {
            return page(
                StatusCode::TOO_EARLY,
                "A little longer",
                "<p>Without JavaScript the desk asks you to wait one minute. Go back and press the button again.</p>",
            );
        }
        // Single use: the challenge is consumed whether or not the proof is valid.
        map.remove(&id)
    };
    let Some(ch) = ch.filter(|c| c.expires > Instant::now()) else {
        return page(
            StatusCode::GONE,
            "This page was shown once",
            "<p>Use the address you copied: it stays valid until its deadline. \
             Or <a href=\"/donate\">get a new one</a>.</p>",
        );
    };
    if !nojs && !pow::verify(&ch.salt, &id, f.nonce.as_bytes(), app.pow_bits) {
        return page(StatusCode::BAD_REQUEST, "Something went wrong", "<p>Please start again.</p>");
    }
    // Paced issuance, counted only for paid-up requests: buying up an epoch to leave one real
    // donor (n-1) takes most of the epoch, and a flood shows as "Busy" instead of passing silently.
    if !app.issuance.lock().unwrap_or_else(|e| e.into_inner()).take(Instant::now()) {
        log::event("intake_busy", &[]);
        return page(StatusCode::TOO_MANY_REQUESTS, "Busy", "<p>Many people are asking right now. Go back and press the button again in a minute.</p>");
    }
    // A stuck desk would issue addresses it cannot forward before their keys are shredded.
    if !app.fresh() {
        return page(StatusCode::SERVICE_UNAVAILABLE, "Starting up", "<p>Try again in a few minutes.</p>");
    }
    let Ok(mut desk) = tokio::time::timeout(Duration::from_secs(30), app.desk.lock()).await else {
        return page(StatusCode::SERVICE_UNAVAILABLE, "Busy", "<p>Try again in a minute.</p>");
    };
    if !desk.synced {
        return page(StatusCode::SERVICE_UNAVAILABLE, "Starting up", "<p>Try again in a minute.</p>");
    }
    let p = desk.p.clone();
    let t = now();
    // The epoch id is the donor's shred receipt: random, and public only once the epoch is shredded.
    let issued = match desk.open_epoch_mut(t) {
        Some(e) => {
            let id = e.st.id.clone();
            e.issue(t, &p).map(|o| o.map(|(a, d)| (a, d, id)))
        }
        None => Ok(None),
    };
    drop(desk);
    let (address, deadline, epoch_id) = match issued {
        Ok(Some(x)) => x,
        Ok(None) => {
            log::event("intake_busy", &[]);
            return page(StatusCode::TOO_MANY_REQUESTS, "Busy", "<p>No addresses left this round. Try again in an hour.</p>");
        }
        Err(_) => {
            log::fatal("issue");
            return page(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong", "<p>Please try again later.</p>");
        }
    };
    if !wallet::is_valid_shielded(&address) || !address.chars().all(|c| c.is_ascii_alphanumeric()) {
        log::fatal("address_check");
        return page(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong", "<p>Please try again later.</p>");
    }
    let deadline_hour = ledger::hour(deadline);
    let hours = deadline.saturating_sub(t) / 3600;
    if plain {
        return text(
            StatusCode::OK,
            format!("address={address}\nuri=zcash:{address}\nvalid_until_utc={deadline_hour}\nreceipt={epoch_id}\n"),
        );
    }
    let body = format!(
        "<p>Send to this one-time address:</p><p><code>{address}</code></p>\
         <p><a href=\"zcash:{address}\">Open in wallet</a></p>\
         <p>Valid until about <strong>{}</strong> UTC (roughly {hours} hours from now). \
         <strong>Payments after that are lost.</strong></p>\
         <p>Copy it now: this page is shown once and the desk keeps no way to show it again.</p>\
         <p>Pay with no memo. A few hours after the deadline the key for this address is destroyed.</p>\
         <p>Your shred receipt: <code>{epoch_id}</code>. Once it appears in the public shred log, \
         the keys for this address are gone. It names your epoch, never you.</p>",
        utc_hour(deadline_hour)
    );
    page(StatusCode::OK, "Your address", &body)
}

/// Formats Unix seconds as "YYYY-MM-DD HH:00" without a date library.
fn utc_hour(t: u64) -> String {
    let days = (t / 86_400) as i64;
    let h = (t % 86_400) / 3600;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {h:02}:00")
}

fn json_resp(v: serde_json::Value) -> Response {
    let mut r = Json(v).into_response();
    r.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

async fn health(State(app): State<App>) -> Response {
    let s = app.snapshot();
    // Only a yes/no: an exact tick age would show when the desk is busy with an op.
    let fresh = app.fresh();
    json_resp(json!({
        "synced": s.synced,
        "fresh": fresh,
        "live_epochs_bucket": match s.live_epochs { 0 => "0", 1..=4 => "1-4", _ => "5+" },
        "intake": if s.synced && fresh { "open" } else { "not_synced" },
        "read_source": "zecrocks",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

#[derive(Deserialize)]
struct Page {
    limit: Option<usize>,
    offset: Option<usize>,
}

/// Newest first, paged: the log only grows.
async fn shred_log(State(app): State<App>, q: Result<Query<Page>, QueryRejection>) -> Response {
    let Ok(Query(q)) = q else {
        return (StatusCode::BAD_REQUEST, "bad query").into_response();
    };
    let mut rows = app.ledger.shred_lines().unwrap_or_default();
    rows.reverse();
    let total = rows.len();
    let (offset, limit) = (q.offset.unwrap_or(0), q.limit.unwrap_or(200).min(1000));
    json_resp(json!({
        "total": total,
        "offset": offset,
        "rows": rows.iter().skip(offset).take(limit).map(|r| json!({
            "epoch_id": r.epoch_id,
            "opened_hour": r.opened_hour,
            "closed_kind": r.closed_kind,
            "closed_hour": r.closed_hour,
            "issued_bucket": r.issued_bucket,
            "profile": r.profile.as_deref().unwrap_or("unknown"),
        })).collect::<Vec<_>>(),
    }))
}

/// Landing numbers, all already public in the shred log or health: nothing new leaks here.
async fn summary(State(app): State<App>) -> Response {
    let rows = app.ledger.shred_lines().unwrap_or_default();
    let count = |kind: &str, profile: Option<&str>| {
        rows.iter().filter(|r| r.closed_kind == kind && (profile.is_none() || r.profile.as_deref() == profile)).count()
    };
    let s = app.snapshot();
    let p = &app.params;
    json_resp(json!({
        "profile": app.build.profile,
        "epochs_shredded": count("shredded", None),
        "epochs_shredded_this_profile": count("shredded", Some(app.build.profile.as_str())),
        "epochs_lost": count("lost", None),
        "last_closed_hour": rows.iter().map(|r| r.closed_hour).max(),
        "live_epochs_bucket": match s.live_epochs { 0 => "0", 1..=4 => "1-4", _ => "5+" },
        "intake": if s.synced && app.fresh() { "open" } else { "not_synced" },
        "swept_bucket": buckets::swept_bucket(s.swept_since_boot),
        "epoch_s": p.epoch_s,
        "grace_s": p.grace_s,
        "burial_blocks": p.burial_blocks,
    }))
}

async fn audit_weeks(State(app): State<App>) -> Response {
    let current = ledger::week_start(now());
    let shreds = app.ledger.shred_lines().unwrap_or_default();
    let weeks: Vec<_> = app
        .ledger
        .weeks()
        .unwrap_or_default()
        .into_iter()
        .filter(|(w, _)| *w < current)
        .map(|(w, z)| {
            let forgotten = shreds.iter().filter(|s| ledger::week_start(s.closed_hour) == w).count();
            json!({ "week_start": w, "total_2sf_zats": buckets::round_2sf(z).to_string(), "epochs_forgotten": forgotten })
        })
        .collect();
    json_resp(json!({ "weeks": weeks }))
}

async fn info(State(app): State<App>) -> Response {
    let p = &app.params;
    json_resp(json!({
        "network": "testnet",
        "version": env!("CARGO_PKG_VERSION"),
        "git_commit": option_env!("AMNESIA_GIT_COMMIT").unwrap_or("unknown"),
        "profile": app.build.profile,
        "binary_sha256": app.build.binary_sha256,
        "build": {
            "cargo_lock_sha256": app.build.cargo_lock_sha256,
            "toolchain": TOOLCHAIN,
            "reproduce": "Offline vendored build from infra/images/rust.Dockerfile, or a locked release cargo build of package amnesia-desk with this toolchain; compare the sha256 of the binary with binary_sha256",
        },
        "tee": "none",
        "params": {
            "epoch_s": p.epoch_s,
            "grace_s": p.grace_s,
            "burial_blocks": p.burial_blocks,
            "cap": p.cap,
            "merge_rate_per_h": p.merge_rate_per_h,
            "d_min_zats": p.d_min,
            "carry_min_zats": p.carry_min,
            "carry_max_zats": p.carry_max,
            "max_canonical": p.max_canonical,
            "tx_expiry_delta": p.tx_expiry_delta,
            "actions_per_tx": 2,
            "fee_zats": amnesia_core::FEE_2_ACTIONS,
            "ovk": "discard",
            "address_revision": "r0",
            "broadcast": app.build.broadcast,
        },
    }))
}

async fn treasury(State(app): State<App>) -> Response {
    json_resp(json!({ "swept_bucket": buckets::swept_bucket(app.snapshot().swept_since_boot) }))
}

#[cfg(test)]
mod tests {
    use super::{Bucket, utc_hour};

    #[test]
    fn issuance_is_paced() {
        // Demo profile: 50 addresses per 2 h epoch, so one every 72 s after a burst of 5.
        let mut b = Bucket::for_epoch(50, 7200);
        let t0 = std::time::Instant::now();
        assert!((0..5).all(|_| b.take(t0)));
        assert!(!b.take(t0));
        assert!(!b.take(t0 + std::time::Duration::from_secs(60)));
        assert!(b.take(t0 + std::time::Duration::from_secs(80)));
    }

    #[test]
    fn formats_hours() {
        assert_eq!(utc_hour(0), "1970-01-01 00:00");
        assert_eq!(utc_hour(1_792_843_200), "2026-10-24 12:00");
    }
}
