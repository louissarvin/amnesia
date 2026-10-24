//! Allowlisted events only (spec 18 §5.15). Never addresses, txids, amounts, keys, slots or IPs.

const KINDS: &[&str] = &[
    "boot",
    "tor_ready",
    "read_source",
    "sync_lag",
    "epoch_open",
    "epoch_close",
    "epoch_shred",
    "epoch_lost",
    "broadcast_fail",
    "intake_busy",
    "fatal",
    "shutdown",
    "slow_tick",
];
const FIELDS: &[&str] = &["epoch_id", "bucket", "code"];

pub fn event(kind: &str, fields: &[(&str, &str)]) {
    if !KINDS.contains(&kind) {
        return;
    }
    let mut line = format!("{{\"kind\":\"{kind}\"");
    for (k, v) in fields.iter().filter(|(k, _)| FIELDS.contains(k)) {
        let v: String = v
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || "_-+.".contains(*c))
            .collect();
        line.push_str(&format!(",\"{k}\":\"{v}\""));
    }
    line.push('}');
    println!("{line}");
}

/// Error text can carry library detail (heights, values); only a coarse code is ever logged.
pub fn fatal(code: &str) {
    event("fatal", &[("code", code)]);
}

static DEV_TRACE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Dev-only op trace to stderr. Exists only in builds with the `dev-trace` feature, so a
/// hosted binary cannot write values or txids to disk whatever its environment says.
pub fn enable_dev_trace() {
    if cfg!(feature = "dev-trace") {
        DEV_TRACE.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(feature = "dev-trace")]
pub fn dev(msg: impl FnOnce() -> String) {
    if DEV_TRACE.load(std::sync::atomic::Ordering::Relaxed) {
        eprintln!("[dev] {}", msg());
    }
}

#[cfg(not(feature = "dev-trace"))]
pub fn dev(_msg: impl FnOnce() -> String) {}
