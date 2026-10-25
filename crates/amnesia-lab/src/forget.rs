//! Keeps the lab's seizure copies from outliving the desk: once the desk shreds or loses an
//! epoch, every stored seizure holding that epoch's files is overwritten and unlinked.

use std::{
    collections::{BTreeSet, HashSet},
    fs,
    io::{ErrorKind, Write as _},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use rand::{
    rand_core::{Rng as _, UnwrapErr},
    rngs::SysRng,
};
use serde::Deserialize;

/// Per-seizure record of the desk epochs its image held, written at seize time.
pub const EPOCHS_FILE: &str = "epochs.json";

/// Desk epoch ids are 16 random bytes, lowercase hex.
pub fn is_epoch_id(s: &str) -> bool {
    s.len() == 32
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Epoch ids named by any path component. Matching any depth covers both a seized desk root
/// (`epochs/<id>/`) and a seized epochs dir (`<id>/`); a false match only deletes more.
pub fn epoch_ids<'a>(paths: impl IntoIterator<Item = &'a str>) -> BTreeSet<String> {
    paths
        .into_iter()
        .flat_map(|p| p.split(['/', '\\']))
        .filter(|c| is_epoch_id(c))
        .map(str::to_string)
        .collect()
}

/// Epoch ids in a seize manifest (`sha256  len  relpath` per line).
pub fn epoch_ids_from_manifest(manifest: &str) -> BTreeSet<String> {
    // A malformed line is scanned whole: over-matching is the safe direction.
    epoch_ids(
        manifest
            .lines()
            .map(|l| l.splitn(3, "  ").nth(2).unwrap_or(l)),
    )
}

/// Epoch ids under `image`, from the file tree itself.
pub fn epoch_ids_in_tree(image: &Path) -> Result<BTreeSet<String>> {
    let mut paths = vec![];
    walk(image, &mut paths)?;
    let rel: Vec<String> = paths
        .iter()
        .map(|p| {
            p.strip_prefix(image)
                .unwrap_or(p)
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    Ok(epoch_ids(rel.iter().map(String::as_str)))
}

fn walk(d: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for e in fs::read_dir(d)? {
        let p = e?.path();
        out.push(p.clone());
        if fs::symlink_metadata(&p)?.is_dir() {
            walk(&p, out)?;
        }
    }
    Ok(())
}

#[derive(Deserialize)]
struct ShredLog {
    rows: Vec<ShredRow>,
}

#[derive(Deserialize)]
struct ShredRow {
    epoch_id: String,
    #[serde(default)]
    closed_kind: Option<String>,
}

/// Epoch ids the desk reports as shredded or lost, from a `GET /v1/shred-log` body.
pub fn closed_epochs(body: &str) -> Result<HashSet<String>> {
    let log: ShredLog = serde_json::from_str(body).context("shred-log json")?;
    Ok(log
        .rows
        .into_iter()
        .filter(|r| matches!(r.closed_kind.as_deref(), Some("shredded" | "lost")))
        .map(|r| r.epoch_id.to_ascii_lowercase())
        .collect())
}

/// Epochs a stored seizure holds: `epochs.json`, else the manifest, else the bundle listing.
/// None if none is readable.
pub fn seizure_epochs(base: &Path) -> Option<BTreeSet<String>> {
    if let Some(ids) = fs::read(base.join(EPOCHS_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice::<Vec<String>>(&b).ok())
    {
        return Some(
            ids.iter()
                .map(|s| s.to_ascii_lowercase())
                .filter(|s| is_epoch_id(s))
                .collect(),
        );
    }
    if let Ok(m) = fs::read_to_string(base.join("manifest.txt")) {
        return Some(epoch_ids_from_manifest(&m));
    }
    let out = std::process::Command::new("tar")
        .arg("-tf")
        .arg(base.join("bundle.tar"))
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(epoch_ids(String::from_utf8_lossy(&out.stdout).lines()))
}

/// Whether a seizure must go: it holds a closed epoch, or its contents cannot be determined.
pub fn too_old(created: std::time::SystemTime, now: std::time::SystemTime, max_age: std::time::Duration) -> bool {
    now.duration_since(created).is_ok_and(|age| age > max_age)
}

pub fn should_forget(held: Option<&BTreeSet<String>>, closed: &HashSet<String>) -> bool {
    held.is_none_or(|ids| ids.iter().any(|id| closed.contains(id)))
}

/// One pass over `seizures/`. Caller must hold the seizure lock so no run is mid-write.
/// Leftover scratch dirs are always shredded; with `closed`, whole seizures are judged too.
/// Returns how many seizures were removed and the first error; one bad dir never stops the pass.
pub fn forget_seizures(
    dir: &Path,
    closed: Option<&HashSet<String>>,
    max_age: std::time::Duration,
) -> (usize, Option<anyhow::Error>) {
    let (mut removed, mut first) = (0, None);
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => return (0, Some(e.into())),
    };
    for e in rd.flatten() {
        let base = e.path();
        if !fs::symlink_metadata(&base).is_ok_and(|m| m.is_dir()) {
            continue;
        }
        // A crashed run can leave raw copies behind; no run is live while the lock is held.
        let mut r = shred_tree(&base.join("image")).and(shred_tree(&base.join("public")));
        // Fail closed: past the longest epoch lifetime every epoch in it is gone, even if the
        // desk's shred log cannot be read right now.
        let old = fs::metadata(&base)
            .and_then(|m| m.created().or_else(|_| m.modified()))
            .is_ok_and(|t| too_old(t, std::time::SystemTime::now(), max_age));
        if old || closed.is_some_and(|c| should_forget(seizure_epochs(&base).as_ref(), c)) {
            r = r.and(shred_tree(&base));
            removed += 1;
        }
        if let Err(e) = r {
            first.get_or_insert(e);
        }
    }
    (removed, first)
}

/// Overwrites every regular file with random bytes and fsyncs before unlinking, like the desk's
/// epoch shred. Symlinks are unlinked, never followed. Missing paths are fine.
/// Keeps going past a failed overwrite so nothing stays linked, then reports the first error.
pub fn shred_tree(path: &Path) -> Result<()> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    if !meta.is_dir() {
        let r = if meta.is_file() {
            overwrite_file(path, meta.len())
        } else {
            Ok(())
        };
        fs::remove_file(path)?;
        return r;
    }
    let mut first: Option<anyhow::Error> = None;
    for e in fs::read_dir(path)? {
        let r = e
            .map_err(anyhow::Error::from)
            .and_then(|e| shred_tree(&e.path()));
        if let Err(e) = r {
            first.get_or_insert(e);
        }
    }
    fs::remove_dir_all(path)?;
    first.map_or(Ok(()), Err)
}

fn overwrite_file(path: &Path, len: u64) -> Result<()> {
    let open = || fs::OpenOptions::new().write(true).open(path);
    let mut f = match open() {
        Ok(f) => f,
        // fs::copy keeps source modes, so a read-only copy needs its owner write bit back.
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            make_writable(path)?;
            open()?
        }
        Err(e) => return Err(e.into()),
    };
    let mut buf = vec![0u8; usize::try_from(len).unwrap_or(usize::MAX).min(1 << 20)];
    UnwrapErr(SysRng).fill_bytes(&mut buf);
    let mut left = len;
    while left > 0 {
        let n = usize::try_from(left).unwrap_or(usize::MAX).min(buf.len());
        f.write_all(&buf[..n])?;
        left -= n as u64;
    }
    f.sync_all()?;
    Ok(())
}

#[cfg(unix)]
fn make_writable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn make_writable(path: &Path) -> Result<()> {
    let mut p = fs::metadata(path)?.permissions();
    p.set_readonly(false);
    fs::set_permissions(path, p)?;
    Ok(())
}

/// Names that may never appear in a public bundle, at any depth: spending keys, raw RAM and
/// the Tor state dir.
pub fn forbidden_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    matches!(n.as_str(), "keyring.bin" | "seed.hex" | "memory" | "arti") || n.ends_with(".mem")
}

/// Errors if any file or dir under `root` has a forbidden name. Unreadable dirs also error.
pub fn check_publishable(root: &Path) -> Result<()> {
    let mut paths = vec![];
    walk(root, &mut paths)?;
    for p in paths {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy())
            .ok_or(anyhow!("unnamed path"))?;
        if forbidden_name(&name) {
            let rel = p.strip_prefix(root).unwrap_or(&p).display().to_string();
            bail!("forbidden path in public bundle: {rel}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "0123456789abcdef0123456789abcdef";
    const B: &str = "fedcba9876543210fedcba9876543210";

    fn tmp(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("amnesia-lab-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn epoch_id_shape() {
        assert!(is_epoch_id(A));
        assert!(!is_epoch_id(&A.to_uppercase()));
        assert!(!is_epoch_id(&A[..31]));
        assert!(!is_epoch_id("0123456789abcdef"));
        assert!(!is_epoch_id(&format!("{A}0")));
        assert!(!is_epoch_id("g123456789abcdef0123456789abcdef"));
    }

    #[test]
    fn ids_from_manifest_both_layouts() {
        let sha = "a".repeat(64);
        let m = format!(
            "{sha}  10  host0/{A}/data.sqlite\n{sha}  20  host1/epochs/{B}/keyring.bin\n\
             {sha}  5  host1/ledger/shred.jsonl\n{sha}  9  memory/desk.mem\n"
        );
        let ids = epoch_ids_from_manifest(&m);
        assert_eq!(ids, BTreeSet::from([A.to_string(), B.to_string()]));
        // The 64 hex sha field is never taken for an id.
        assert_eq!(
            epoch_ids_from_manifest(&format!("{sha}  1  treasury/seed.hex\n")).len(),
            0
        );
    }

    #[test]
    fn ids_from_tar_listing() {
        let listing = format!(
            "./\n./image/\n./image/host0/{A}/\n./image/host0/{A}/data.sqlite\n./records.csv\n"
        );
        assert_eq!(epoch_ids(listing.lines()), BTreeSet::from([A.to_string()]));
    }

    #[test]
    fn shred_log_kinds() {
        let body = format!(
            r#"{{"rows":[{{"epoch_id":"{A}","closed_kind":"shredded","opened_hour":0}},
            {{"epoch_id":"{}","closed_kind":"lost"}},{{"epoch_id":"{}","closed_kind":"other"}}]}}"#,
            B.to_uppercase(),
            "1".repeat(32)
        );
        let c = closed_epochs(&body).unwrap();
        assert_eq!(c, HashSet::from([A.to_string(), B.to_string()]));
        assert!(closed_epochs("not json").is_err());
    }

    #[test]
    fn forget_rule() {
        let closed = HashSet::from([A.to_string()]);
        assert!(should_forget(None, &closed));
        assert!(should_forget(
            Some(&BTreeSet::from([A.to_string(), B.to_string()])),
            &closed
        ));
        assert!(!should_forget(
            Some(&BTreeSet::from([B.to_string()])),
            &closed
        ));
        assert!(!should_forget(Some(&BTreeSet::new()), &closed));
    }

    #[test]
    fn forget_pass_on_disk() {
        let d = tmp("forget");
        let mk = |id: &str, files: &[(&str, String)]| {
            let b = d.join(id);
            fs::create_dir_all(b.join("image/host0")).unwrap();
            fs::write(b.join("image/host0/x"), b"raw").unwrap();
            for (n, c) in files {
                fs::write(b.join(n), c).unwrap();
            }
        };
        mk("s1", &[(EPOCHS_FILE, format!("[\"{A}\"]"))]);
        mk("s2", &[(EPOCHS_FILE, format!("[\"{B}\"]"))]);
        mk(
            "s3",
            &[(
                "manifest.txt",
                format!("{}  1  host0/{A}/f\n", "0".repeat(64)),
            )],
        );
        mk("s4", &[("summary.json", "{}".into())]);
        mk("s5", &[(EPOCHS_FILE, "[]".into())]);
        let closed = HashSet::from([A.to_string()]);
        let (n, e) = forget_seizures(&d, None, std::time::Duration::from_secs(86_400));
        assert!(n == 0 && e.is_none());
        assert!(!d.join("s1/image").exists());
        let (n, e) = forget_seizures(&d, Some(&closed), std::time::Duration::from_secs(86_400));
        assert!(n == 3 && e.is_none());
        assert!(!d.join("s1").exists() && !d.join("s3").exists() && !d.join("s4").exists());
        assert!(d.join("s2/epochs.json").exists() && d.join("s5").exists());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn shred_handles_read_only_and_missing() {
        let d = tmp("shred");
        let f = d.join("sub/seed.hex");
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(&f, b"secret").unwrap();
        let mut p = fs::metadata(&f).unwrap().permissions();
        p.set_readonly(true);
        fs::set_permissions(&f, p).unwrap();
        shred_tree(&d).unwrap();
        assert!(!d.exists());
        shred_tree(&d).unwrap();
    }

    #[test]
    fn forbidden_names() {
        for n in [
            "keyring.bin",
            "seed.hex",
            "desk.mem",
            "x.MEM",
            "memory",
            "arti",
            "Seed.Hex",
        ] {
            assert!(forbidden_name(n), "{n}");
        }
        for n in [
            "keyring.bin.ufvk.txt",
            "seed.hex.ufvk.txt",
            "memo",
            "records.csv",
            "artifact",
            "mem",
        ] {
            assert!(!forbidden_name(n), "{n}");
        }
    }

    #[test]
    fn publishable_check_any_depth() {
        let d = tmp("publish");
        fs::create_dir_all(d.join("image/host0/a/b")).unwrap();
        fs::write(d.join("image/host0/a/b/seed.hex.ufvk.txt"), b"ok").unwrap();
        check_publishable(&d).unwrap();
        fs::create_dir_all(d.join("image/host0/a/b/arti")).unwrap();
        assert!(check_publishable(&d).is_err());
        fs::remove_dir_all(d.join("image/host0/a/b/arti")).unwrap();
        fs::write(d.join("image/host0/a/b/keyring.bin"), b"k").unwrap();
        assert!(check_publishable(&d).is_err());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn age_limit_fails_closed() {
        use std::time::{Duration, SystemTime};
        let now = SystemTime::now();
        let day = Duration::from_secs(24 * 3600);
        assert!(too_old(now - day - Duration::from_secs(1), now, day));
        assert!(!too_old(now - Duration::from_secs(60), now, day));
        // A clock set backwards must not wipe fresh seizures.
        assert!(!too_old(now + Duration::from_secs(60), now, day));
    }
}
