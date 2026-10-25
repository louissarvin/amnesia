//! Public, permanent record: shred lines and weekly emitted totals. Nothing donor-linkable.

use std::{
    collections::BTreeMap,
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct ShredLine {
    pub epoch_id: String,
    /// Unix seconds rounded down to the hour.
    pub opened_hour: u64,
    pub closed_kind: String,
    pub closed_hour: u64,
    pub issued_bucket: String,
    /// Params profile the desk ran when the epoch closed. Absent on rows written before
    /// Oct 26 2026, most of which ran the short dev profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

#[derive(Clone)]
pub struct Ledger {
    dir: PathBuf,
}

pub fn hour(t: u64) -> u64 {
    t / 3600 * 3600
}

/// Monday 00:00 UTC of the week containing `t`.
pub fn week_start(t: u64) -> u64 {
    let day = t / 86_400;
    // 1970-01-01 was a Thursday: day 0 is weekday 3 counting Monday as 0.
    (day - (day + 3) % 7) * 86_400
}

impl Ledger {
    pub fn open(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    pub fn append_shred(&self, line: &ShredLine) -> Result<()> {
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join("shred.jsonl"))?;
        writeln!(f, "{}", serde_json::to_string(line)?)?;
        f.sync_all()?;
        Ok(())
    }

    /// Every epoch is announced when it opens, so one that vanishes (crash, reboot of a RAM-only
    /// host) can be reported as lost instead of silently disappearing.
    pub fn append_open(&self, epoch_id: &str, opened_hour: u64) -> Result<()> {
        let mut f = fs::OpenOptions::new().create(true).append(true).open(self.dir.join("open.jsonl"))?;
        writeln!(f, "{}", serde_json::json!({ "epoch_id": epoch_id, "opened_hour": opened_hour }))?;
        f.sync_all()?;
        Ok(())
    }

    /// Appends a `lost` line for each announced epoch that is neither closed nor still live.
    pub fn record_lost(&self, live: &[String], now_hour: u64, profile: &str) -> Result<usize> {
        let closed: std::collections::BTreeSet<String> = self.shred_lines()?.into_iter().map(|l| l.epoch_id).collect();
        let opens = fs::read_to_string(self.dir.join("open.jsonl")).unwrap_or_default();
        let mut n = 0;
        for l in opens.lines() {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(l) else { continue };
            let (Some(id), Some(h)) = (v["epoch_id"].as_str(), v["opened_hour"].as_u64()) else { continue };
            if closed.contains(id) || live.iter().any(|x| x == id) {
                continue;
            }
            self.append_shred(&ShredLine {
                epoch_id: id.into(),
                opened_hour: h,
                closed_kind: "lost".into(),
                closed_hour: now_hour,
                issued_bucket: "unknown".into(),
                profile: Some(profile.into()),
            })?;
            n += 1;
        }
        Ok(n)
    }

    pub fn shred_lines(&self) -> Result<Vec<ShredLine>> {
        let s = fs::read_to_string(self.dir.join("shred.jsonl")).unwrap_or_default();
        Ok(s.lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect())
    }

    fn weeks_path(&self) -> PathBuf {
        self.dir.join("weeks.json")
    }

    /// Exact weekly sums stay on the host; only 2 significant figures are ever served.
    pub fn add_emitted(&self, now: u64, zats: u64) -> Result<()> {
        let mut w = self.weeks()?;
        *w.entry(week_start(now)).or_default() += zats;
        let tmp = self.dir.join("weeks.json.tmp");
        fs::write(&tmp, serde_json::to_vec(&w)?)?;
        fs::rename(tmp, self.weeks_path())?;
        Ok(())
    }

    pub fn weeks(&self) -> Result<BTreeMap<u64, u64>> {
        Ok(fs::read(self.weeks_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanished_epochs_become_lost_once() {
        let dir = std::env::temp_dir().join(format!("amnesia-ledger-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let l = Ledger::open(&dir).unwrap();
        for id in ["a", "b", "c"] {
            l.append_open(id, 3600).unwrap();
        }
        l.append_shred(&ShredLine {
            epoch_id: "a".into(),
            opened_hour: 3600,
            closed_kind: "shredded".into(),
            closed_hour: 7200,
            issued_bucket: "0".into(),
            profile: None,
        })
        .unwrap();
        // "b" is still live, "c" vanished.
        assert_eq!(l.record_lost(&["b".into()], 10_800, "test").unwrap(), 1);
        assert_eq!(l.record_lost(&["b".into()], 10_800, "test").unwrap(), 0);
        let lost: Vec<_> = l.shred_lines().unwrap().into_iter().filter(|x| x.closed_kind == "lost").collect();
        assert_eq!(lost.len(), 1);
        assert_eq!(lost[0].epoch_id, "c");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn week_starts_on_monday() {
        // 2026-10-24 12:00 UTC is a Saturday; the week starts Monday 2026-10-19.
        assert_eq!(week_start(1_792_843_200), 1_792_368_000);
        assert_eq!(week_start(1_792_368_000), 1_792_368_000);
    }
}
