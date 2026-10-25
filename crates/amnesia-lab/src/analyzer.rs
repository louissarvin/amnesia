//! The seizure attack. Input: every byte from a seized host (a directory image) plus the
//! exchange's withdrawal log. Output: which exchange clients can be named as donors.
//!
//! It trusts nothing the desk says about itself: it walks raw files, opens anything that is a
//! SQLite database, parses keyrings and seeds, and greps every byte for address strings.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use amnesia_desk::wallet;
use anyhow::Result;
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;

use crate::records::Withdrawal;

#[derive(Serialize, Default)]
pub struct Report {
    pub files_scanned: usize,
    pub bytes_scanned: u64,
    pub sqlite_dbs: usize,
    pub keyring_keys: usize,
    pub seed_files: usize,
    pub receipts: Vec<Receipt>,
    pub raw_addresses: usize,
    /// Raw RAM of the live desk process, when the seizure captured it.
    pub memory_bytes: u64,
    pub memory_addresses: usize,
    pub records_total: usize,
    pub linked: Vec<Linked>,
    pub unlinked: usize,
    /// Present when the image holds the organisation's treasury wallet.
    pub amount_attack: Option<crate::amount::AmountReport>,
}

#[derive(Serialize, Clone)]
pub struct Receipt {
    pub address: String,
    pub value_zats: u64,
    pub height: Option<u32>,
    pub source: String,
}

#[derive(Serialize)]
pub struct Linked {
    pub client_id: String,
    pub value_zats: u64,
    pub height: Option<u32>,
    pub how: String,
}

const SQLITE_MAGIC: &[u8] = b"SQLite format 3\0";

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        match fs::symlink_metadata(&p) {
            Ok(m) if m.is_dir() => walk(&p, out),
            Ok(m) if m.is_file() => out.push(p),
            _ => {}
        }
    }
}

/// Every `utest1...` run of Bech32 characters that decodes to a shielded unified address.
fn scan_addresses(bytes: &[u8], found: &mut BTreeSet<String>) {
    let pat = b"utest1";
    let mut i = 0;
    while i + pat.len() <= bytes.len() {
        if &bytes[i..i + pat.len()] == pat {
            let mut j = i + pat.len();
            while j < bytes.len() && (bytes[j].is_ascii_lowercase() || bytes[j].is_ascii_digit()) {
                j += 1;
            }
            if let Ok(s) = std::str::from_utf8(&bytes[i..j])
                && wallet::is_valid_shielded(s)
            {
                found.insert(s.to_string());
            }
            i = j;
        } else {
            i += 1;
        }
    }
}

fn sqlite_receipts(path: &Path, source: &str) -> Vec<Receipt> {
    let uri = format!("file:{}?immutable=1", path.display());
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI;
    let Ok(conn) = Connection::open_with_flags(uri, flags) else {
        return vec![];
    };
    let mut out = vec![];
    for table in ["ironwood_received_notes", "orchard_received_notes"] {
        let sql = format!(
            "SELECT a.address, n.value, t.mined_height FROM {table} n \
             JOIN addresses a ON a.id = n.address_id \
             LEFT JOIN transactions t ON t.id_tx = n.transaction_id \
             WHERE n.is_change = 0"
        );
        let Ok(mut stmt) = conn.prepare(&sql) else {
            continue;
        };
        let rows = stmt.query_map([], |r| {
            Ok(Receipt {
                address: r.get(0)?,
                value_zats: r.get::<_, i64>(1)? as u64,
                height: r.get::<_, Option<i64>>(2)?.map(|h| h as u32),
                source: source.to_string(),
            })
        });
        if let Ok(rows) = rows {
            out.extend(rows.flatten());
        }
    }
    out
}

/// Keyring layout: per key 16 B uuid + 32 B seed + u16 BE length + address.
fn keyring_count(bytes: &[u8]) -> usize {
    let (mut i, mut n) = (0, 0);
    while i + 50 <= bytes.len() {
        let len = u16::from_be_bytes([bytes[i + 48], bytes[i + 49]]) as usize;
        i += 50 + len;
        n += 1;
    }
    n
}

pub fn analyze(image: &Path, records: &[Withdrawal]) -> Result<Report> {
    let mut files = vec![];
    walk(image, &mut files);
    let mut r = Report {
        records_total: records.len(),
        ..Default::default()
    };
    let mut raw = BTreeSet::new();
    for f in &files {
        let Ok(bytes) = fs::read(f) else { continue };
        r.files_scanned += 1;
        r.bytes_scanned += bytes.len() as u64;
        let rel = f.strip_prefix(image).unwrap_or(f).display().to_string();
        let name = f.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if bytes.starts_with(SQLITE_MAGIC) {
            r.sqlite_dbs += 1;
            r.receipts.extend(sqlite_receipts(f, &rel));
        }
        if name == "keyring.bin" {
            r.keyring_keys += keyring_count(&bytes);
        }
        if name == "seed.hex" {
            r.seed_files += 1;
        }
        // Public bundles carry viewing keys in place of seeds: same linking power.
        if name.ends_with(".ufvk.txt") {
            r.keyring_keys += bytes.split(|&b| b == b'\n').filter(|l| !l.is_empty()).count();
        }
        if Path::new(&rel).components().next().is_some_and(|c| c.as_os_str() == "memory") {
            let mut in_mem = BTreeSet::new();
            scan_addresses(&bytes, &mut in_mem);
            r.memory_bytes += bytes.len() as u64;
            r.memory_addresses += in_mem.len();
        }
        scan_addresses(&bytes, &mut raw);
    }
    r.raw_addresses = raw.len();
    let by_receipt: BTreeMap<&str, &Receipt> =
        r.receipts.iter().map(|x| (x.address.as_str(), x)).collect();
    let mut linked = vec![];
    for w in records {
        let a = w.recipient_address.as_str();
        if by_receipt.contains_key(a) {
            linked.push(Linked {
                client_id: w.client_id.clone(),
                value_zats: w.value_zats,
                height: w.height,
                how: "receipt in wallet db".into(),
            });
        } else if raw.contains(a) {
            linked.push(Linked {
                client_id: w.client_id.clone(),
                value_zats: w.value_zats,
                height: w.height,
                how: "address string in raw bytes".into(),
            });
        }
    }
    r.unlinked = records.len() - linked.len();
    let treasury: Vec<(u64, u32)> = r
        .receipts
        .iter()
        .filter(|x| Path::new(&x.source).components().any(|c| c.as_os_str() == "treasury"))
        .filter_map(|x| x.height.map(|h| (x.value_zats, h)))
        .collect();
    if !treasury.is_empty() {
        let unlinked: Vec<&Withdrawal> = records
            .iter()
            .filter(|w| !by_receipt.contains_key(w.recipient_address.as_str()) && !raw.contains(&w.recipient_address))
            .collect();
        r.amount_attack = Some(crate::amount::attack(&treasury, &unlinked, crate::amount::Band::default()));
    }
    r.linked = linked;
    Ok(r)
}

pub fn summary(name: &str, r: &Report) -> String {
    let mut s = format!(
        "{name}\n  scanned .................. {} files, {} bytes, {} sqlite dbs\n  \
         keys found ............... {} keyring keys, {} seed files\n  \
         receipts recoverable ..... {}\n  \
         raw RAM .................. {} bytes, {} addresses in memory\n  \
         exchange withdrawals ..... {}   linked: {}   unlinked: {}\n",
        r.files_scanned,
        r.bytes_scanned,
        r.sqlite_dbs,
        r.keyring_keys,
        r.seed_files,
        r.receipts.len(),
        r.memory_bytes,
        r.memory_addresses,
        r.records_total,
        r.linked.len(),
        r.unlinked
    );
    for l in &r.linked {
        let h = l
            .height
            .map(|h| h.to_string())
            .unwrap_or_else(|| "unmined".into());
        s.push_str(&format!(
            "    {}  {} zats  height {}  ({})\n",
            l.client_id, l.value_zats, h, l.how
        ));
    }
    if let Some(a) = &r.amount_attack {
        s.push_str(&format!(
            "  amount attack ............ {} sweeps, {} candidates   pinned: {}   excluded: {}   ambiguous: {}\n",
            a.sweeps, a.candidates, a.pinned, a.excluded, a.ambiguous
        ));
        for w in a.windows.iter().filter(|w| !w.pinned.is_empty()) {
            s.push_str(&format!("    pinned at sweep {}: {}\n", w.sweep_height, w.pinned.join(", ")));
        }
    }
    s
}
