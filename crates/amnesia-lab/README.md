<div align="center">

<img src="../../web/public/assets/logo.png" alt="Amnesia" width="84" />

# amnesia-lab

### The disclosed attacker, and the demo around it.

The lab is everything the desk is up against, built so a judge can run it. It plays a custodial exchange with real testnet funds, keeps a normal wallet for comparison, seizes both, and reports who can be named. It also serves the judge UI and its JSON API. A real deployment has no lab.

<br/>

![Rust](https://img.shields.io/badge/Rust-1.93.1-000000?logo=rust&logoColor=white)
![axum](https://img.shields.io/badge/axum-0.8.9-000000)
![ZIP 32](https://img.shields.io/badge/baseline-ZIP_32_seed_wallet-F4B728)
![Tests](https://img.shields.io/badge/tests-23_passing-6E9F18)

**[Back to the project](../../README.md)** · **[Try a seizure](https://amnesia.taila8827e.ts.net/seize)** · **[Lab API](https://amnesia.taila8827e.ts.net/demo/status)**

</div>

---

## What it plays

| Role | How it is real | What is simulated |
|---|---|---|
| The exchange | A ZIP 32 seed wallet on Zcash Testnet, paying real shielded withdrawals | Its clients (`c01`, `you-4a86`, …) are synthetic ids |
| The exchange's records | Every withdrawal it actually sent: client, recipient, amount, txid, and the block height from its own wallet | Nothing: this is what a court order to an exchange returns |
| Donors | Bots donate about once an hour and send decoy withdrawals; judges donate with one click | The bots |
| The normal wallet ("baseline") | A ZIP 32 seed wallet that receives each donation exactly as a best-practice desk would. Its `seed.hex` sits on disk on purpose: that is what a seizure finds | Nothing |
| The seizure | Copies the desk's files and RAM-disk epochs (and, on Linux with the ramproof override, its process memory) | On the Mac host: files only |

---

## The seizure, as an attacker would run it

The analyzer trusts nothing the desk says about itself:

1. **Walk the raw image.** Every file in the copy, opened as bytes.
2. **Open anything that is a SQLite database**, parse keyrings and seeds, and decrypt every received note it can.
3. **Grep every byte for address strings**, in case a key is gone but an address survived somewhere.
4. **Join with the exchange's records.** A withdrawal is *linked* when its recipient shows up as a receipt in a wallet DB or as an address in raw bytes. The report says which way, per client.
5. **Amount attack on what was already shredded.** With the treasury seed, group the canonical notes the desk swept in into sweeps, and ask which sets of unlinked withdrawals could sum to each one under the carry model. A withdrawal is *pinned* only if exactly one sweep can explain it.

> Real, 27 Oct 2026, 15:45 UTC: normal wallet 25 of 75 named, Amnesia 12 of 80 (live epochs only). Amount attack: 16 sweeps, 65 candidates, **0 pinned**; one sweep fit 13,181 different subsets.

Seizure bundles are evidence of live keys, so the lab forgets them too: once the desk shreds or loses an epoch, every stored seizure holding that epoch's files is overwritten and unlinked, and nothing is kept past 24 hours.

---

## Modules

| File | Lines | What it does |
|---|---|---|
| `daemon.rs` | 1054 | `amnesia-lab serve`: bots, float upkeep, hourly auto-seizure pair, judge actions, the demo API, static UI |
| `analyzer.rs` | 249 | The seizure attack over a raw directory image plus the withdrawal log |
| `amount.rs` | 201 | Amount attack on shredded epochs, enumerating consistent subsets per sweep |
| `forget.rs` | 438 | Shreds stored seizures once their epochs are gone, or after 24 h |
| `seedwallet.rs` | 272 | The exchange and baseline wallets: sync, pay, shield transparent faucet funds |
| `ops.rs` | 129 | Donate (fetch an address through the desk's intake, pay both wallets), decoy, split the float |
| `intake_client.rs` | 61 | Gets an address exactly like a donor's browser: GET the form, solve the PoW, POST, read address, deadline and receipt |
| `records.rs` | 64 | The exchange's withdrawal log, as JSON lines and CSV |
| `limits.rs` | 259 | Global and per-client cooldowns, and a rolling cap of 6 lab donations per hour so the lab cannot fill an epoch by itself |
| `memdump.rs` | 57 | Linux only: reads the desk's memory through `/proc/PID/mem` for the RAM proof |
| `web.rs` | 211 | Serves the web build under a strict CSP built from the build's own inline script hashes, with precompressed gzip |
| `bin/amnesia-lab.rs` | 89 | The CLI below |
| `bin/seize.rs` | 27 | `seize IMAGE_DIR RECORDS_CSV [--json]`: run the attack offline on a downloaded bundle |

---

## Demo API

Every POST needs the header `X-Amnesia-Demo: 1`. Rate limits key on the last `X-Forwarded-For` entry, which Tailscale Funnel sets to the visitor's real IP; IPs are kept in memory for cooldowns only and never logged.

| Route | Does |
|---|---|
| `POST /demo/donate` | Starts a judge donation run; returns `run_id` |
| `POST /demo/seize` `{"target":"amnesia"\|"baseline"}` | Starts a seizure run |
| `GET /demo/runs/{id}` | Run events: `float_wait`, `issued` (client, amount, address, deadline, receipt), `broadcast`, `mined`, `bundle_ready`, `scanning`, `report` |
| `GET /demo/seizures` | Current seizures with counts |
| `GET /demo/seizures/{id}/report.json` | The full report, including the amount attack |
| `GET /demo/seizures/{id}/bundle.tar` | The seized bundle, streamed, for offline `seize` |
| `GET /demo/records/{target}.csv` | The exchange log an attacker of that target receives |
| `GET /demo/history` | Every seizure ever run, counts only |
| `GET /demo/status` | Donations, decoys, spendable float, notes, fee |

---

## CLI

```bash
amnesia-lab serve                                      # the daemon behind the public demo
amnesia-lab init-all                                   # create exchange, baseline and treasury wallets
amnesia-lab init DIR                                   # create one seed wallet, print its first address
amnesia-lab sync DIR                                   # sync one wallet, print spendable zats
amnesia-lab addr DIR | taddr DIR | fund-address DIR    # next shielded, transparent, or funding address
amnesia-lab donate EXCH_DIR CLIENT INTAKE BASE_DIR ZATS # one donation to both desks
amnesia-lab split EXCH_DIR N EACH_ZATS                 # fan the exchange float out into N notes
amnesia-lab decoy EXCH_DIR CLIENT ZATS                 # one withdrawal to an unrelated address
amnesia-lab records EXCH_DIR amnesia|baseline          # the CSV an attacker of that target receives
seize IMAGE_DIR RECORDS_CSV --json                     # the attack, offline, on a downloaded bundle
```

## Configuration

| Variable | Default | Meaning |
|---|---|---|
| `AMNESIA_LAB_ROOT` | `var/lab` | Lab wallets, records, history |
| `AMNESIA_LAB_ADDR` | `127.0.0.1:8090` | Demo API and UI listener |
| `AMNESIA_DESK_INTAKE` | `127.0.0.1:8081` | The desk's donor intake |
| `AMNESIA_DESK_READ` | `127.0.0.1:8082` | The desk's read API |
| `AMNESIA_SEIZE_SOURCES` | `var/desk` | Comma-separated paths a seizure copies (on the Mac: the desk root and the epochs RAM disk) |
| `AMNESIA_WEB_DIR` | unset | Serve the web build from here |
| `AMNESIA_LAB_BOTS` | `1` | Run the bots |
| `AMNESIA_LAB_DONATIONS_PER_H` | `1.5` | Bot donation rate (the Mac host runs 1) |
| `AMNESIA_LAB_DECOYS_PER_H` | `3` | Bot decoy rate (the Mac host runs 2) |
| `AMNESIA_FAUCET` | `0` | Claim from the testnet faucet into a transparent address and shield it |
| `AMNESIA_SEIZE_MEMORY` | | Linux only: include the desk's RAM in seizures |
| `AMNESIA_LWD_HOST`, `AMNESIA_TOR` | `testnet.zec.rocks`, off | lightwalletd host; Tor for the lab's own reads |

---

## Reliability built in

Learned from running it publicly:
- **Float wait.** If change from earlier sends is still confirming, a judge donation waits up to 10 minutes (`float_wait` event) instead of failing. The float is split early and wide (below 6 notes, into up to 16).
- **Busy desk.** An intake `503` while the desk lock is held by a slow tick is retried for about 3 minutes.
- **Never a stale report.** An amnesia and baseline seizure pair runs every hour, so `/seize` always has a fresh real report.

## Build and test

```bash
cargo build --release --locked -p amnesia-lab
cargo test --release -p amnesia-lab      # 23 tests
```
