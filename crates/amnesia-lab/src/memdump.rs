//! Raw memory capture of the running desk for the seizure demo (Linux only). Reads every
//! readable mapping of the process through /proc/PID/mem, so `seize` searches live RAM too.
//! Needs CAP_SYS_PTRACE and a shared PID namespace with the desk; demo-only, disclosed.

use std::{fs, path::Path};

use anyhow::{anyhow, Result};

/// First process whose `comm` is `name`.
pub fn find_pid(name: &str) -> Option<u32> {
    fs::read_dir("/proc").ok()?.flatten().find_map(|e| {
        let pid: u32 = e.file_name().to_str()?.parse().ok()?;
        let comm = fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
        (comm.trim() == name).then_some(pid)
    })
}

/// Writes all readable regions to `out` and returns the bytes captured.
#[cfg(target_os = "linux")]
pub fn dump(pid: u32, out: &Path) -> Result<u64> {
    use std::{io::Write, os::unix::fs::FileExt};
    let maps = fs::read_to_string(format!("/proc/{pid}/maps"))?;
    let mem = fs::File::open(format!("/proc/{pid}/mem"))?;
    let mut f = fs::File::create(out)?;
    let mut total = 0u64;
    let mut buf = vec![0u8; 1 << 20];
    for line in maps.lines() {
        let mut parts = line.split_whitespace();
        let (Some(range), Some(perms)) = (parts.next(), parts.next()) else {
            continue;
        };
        if !perms.starts_with('r') {
            continue;
        }
        let (a, b) = range.split_once('-').ok_or(anyhow!("bad maps line"))?;
        let (start, end) = (u64::from_str_radix(a, 16)?, u64::from_str_radix(b, 16)?);
        let mut off = start;
        while off < end {
            let n = ((end - off) as usize).min(buf.len());
            match mem.read_at(&mut buf[..n], off) {
                Ok(0) | Err(_) => break,
                Ok(k) => {
                    f.write_all(&buf[..k])?;
                    total += k as u64;
                    off += k as u64;
                }
            }
        }
    }
    f.sync_all()?;
    Ok(total)
}

#[cfg(not(target_os = "linux"))]
pub fn dump(_pid: u32, _out: &Path) -> Result<u64> {
    Err(anyhow!("memory capture needs Linux"))
}
