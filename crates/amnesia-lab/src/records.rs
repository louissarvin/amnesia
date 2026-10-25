//! The simulated exchange's withdrawal log: what a court order to an exchange returns.

use std::{fs, io::Write as _, path::Path};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Withdrawal {
    pub client_id: String,
    pub recipient_address: String,
    pub value_zats: u64,
    pub txid: String,
    /// "amnesia", "baseline" or "decoy". Lab bookkeeping only: the seize analyzer never reads it.
    pub kind: String,
    #[serde(default)]
    pub height: Option<u32>,
}

pub fn append(dir: &Path, w: &Withdrawal) -> Result<()> {
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("records.jsonl"))?;
    writeln!(f, "{}", serde_json::to_string(w)?)?;
    Ok(())
}

pub fn load(dir: &Path) -> Result<Vec<Withdrawal>> {
    let s = fs::read_to_string(dir.join("records.jsonl")).unwrap_or_default();
    Ok(s.lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect())
}

/// CSV handed to the attacker: `client_id,recipient_address,value_zats,height`. No `kind`.
pub fn to_csv(rows: &[Withdrawal]) -> String {
    let mut out = String::from("client_id,recipient_address,value_zats,height\n");
    for r in rows {
        let h = r.height.map(|h| h.to_string()).unwrap_or_default();
        out.push_str(&format!(
            "{},{},{},{}\n",
            r.client_id, r.recipient_address, r.value_zats, h
        ));
    }
    out
}

pub fn parse_csv(s: &str) -> Vec<Withdrawal> {
    s.lines()
        .skip(1)
        .filter_map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            (f.len() >= 3).then(|| Withdrawal {
                client_id: f[0].to_string(),
                recipient_address: f[1].to_string(),
                value_zats: f[2].parse().unwrap_or(0),
                txid: String::new(),
                kind: String::new(),
                height: f.get(3).and_then(|h| h.parse().ok()),
            })
        })
        .collect()
}
