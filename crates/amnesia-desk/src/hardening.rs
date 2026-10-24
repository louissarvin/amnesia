//! Host hardening. `apply` always runs; `check_host` gates the hosted desk (AMNESIA_STRICT=1).

use std::path::Path;

use anyhow::{bail, Result};

pub fn apply() {
    // SAFETY: plain libc calls with valid arguments, made before any thread is spawned.
    unsafe {
        let zero = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        libc::setrlimit(libc::RLIMIT_CORE, &zero);
        libc::umask(0o077);
        #[cfg(target_os = "linux")]
        libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0);
    }
}

/// Refuses to run unless swap is off and the epochs directory is a private tmpfs.
pub fn check_host(epochs_dir: &Path) -> Result<()> {
    if !cfg!(target_os = "linux") {
        bail!("strict mode needs Linux");
    }
    let swaps = std::fs::read_to_string("/proc/swaps")?;
    if swaps.lines().count() > 1 {
        bail!("swap is on");
    }
    let mounts = std::fs::read_to_string("/proc/mounts")?;
    let dir = std::fs::canonicalize(epochs_dir)?;
    let dir = dir.to_string_lossy();
    let ok = mounts.lines().any(|l| {
        let f: Vec<&str> = l.split_whitespace().collect();
        f.len() >= 4
            && f[1] == dir
            && f[2] == "tmpfs"
            && ["nosuid", "nodev", "noexec"]
                .iter()
                .all(|o| f[3].split(',').any(|x| x == *o))
    });
    if !ok {
        bail!("epochs dir is not a nosuid,nodev,noexec tmpfs");
    }
    Ok(())
}
