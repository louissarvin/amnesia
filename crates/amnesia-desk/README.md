<div align="center">

<img src="../../web/public/assets/logo.png" alt="Amnesia" width="84" />

# amnesia-desk

### The desk: issue, forward, sweep, bury, shred.

The daemon a donor actually pays. It issues one-time Orchard addresses through an onion intake, moves funds on a schedule fixed at issuance, sweeps them to the treasury, and destroys every key of an epoch once its last transaction is buried. It talks to the chain only over Tor and publishes only coarse, non-linkable numbers.

<br/>

![Rust](https://img.shields.io/badge/Rust-1.93.1-000000?logo=rust&logoColor=white)
![librustzcash](https://img.shields.io/badge/zcash__client__backend-0.25.0--pre.2-F4B728)
![Orchard](https://img.shields.io/badge/orchard-0.16-0B0B0C)
![arti](https://img.shields.io/badge/arti-0.43-7D4698)
![axum](https://img.shields.io/badge/axum-0.8.9-000000)
![Tests](https://img.shields.io/badge/tests-4_passing-6E9F18)

**[Back to the project](../../README.md)** · **[Security model](../../SECURITY.md)** · **[Live read API](https://amnesia.taila8827e.ts.net/v1/summary)**

</div>

---

## At a glance

| | |
|---|---|
| Binary | `amnesia-desk` |
| Listens | Donor intake `127.0.0.1:8081` (published only as the onion), read API `127.0.0.1:8082` |
| Chain | lightwalletd (`testnet.zec.rocks` by default), compact blocks only, never `GetTransaction`, always over arti |
| State | One directory per epoch (wallet DB, keyring, schedule) on a RAM disk; a public ledger of shred lines |
| Never logs | Addresses, txids, amounts, keys, slots or IPs. Allowlisted event kinds only |

---

## The epoch lifecycle

Each epoch is one directory with its own `zcash_client_sqlite` wallet holding `cap + 1` accounts, each from its own 32 OS-random bytes. Account 0 is the pool; the rest are receive addresses.

| Phase | Enters when | What the desk does |
|---|---|---|
| `Open` | A new epoch starts (every `epoch_s`) | Issues addresses to donors; each issue draws its forward slot and final slot in advance |
| `Closed` | `epoch_s` has passed | No new addresses. Payments still arrive and forward at their slots |
| `Final` | The deadline (`opened + epoch_s + grace_s`) passes | Waits for the last forward slot, drained receives and no in-flight transactions, or gives up at deadline plus `grace_s` |
| `SweepWait` | `Final` is done | Waits a random `grace_s / 4` to `grace_s`, then until every forward and merge has confirmed |
| `Sweeping` | The pool is fully spendable | Pays the treasury in canonical 1, 2, 5 denominations and moves a random carry to the next epoch |
| `Burying` | The pool holds no more than one fee | Waits until the last transaction is `burial_blocks` deep |
| *shredded* | Buried and idle | Overwrites every file with random bytes, fsyncs, unlinks, removes the directory, zeroizes seeds, appends one shred line |

Payments never trigger anything directly: every transaction happens at a slot drawn at issuance or on the desk-wide Poisson clock, so the chain cannot tell when a payment arrived from when the desk moved it.

---

## Modules

| File | Lines | What it does |
|---|---|---|
| `main.rs` | 222 | Entry point: config, Tor, the tick loop, broadcasts outside the desk lock with a random 20 to 120 s delay, graceful SIGTERM |
| `desk.rs` | 451 | The desk loop: open epochs, scan, forward at slots, merge, emit, carry, bury, shred |
| `epoch.rs` | 301 | One epoch: wallet DB, keyring and schedule in one directory, destroyed as a whole; persisted in-flight txs |
| `wallet.rs` | 333 | The epoch wallet: accounts from OS randomness, Orchard-only addresses, 2-action transactions, no OVK, expiry plus 40 |
| `chain.rs` | 266 | Compact block sync in 100 block chunks, reorg rewind, tree state, broadcast, each on isolated Tor circuits |
| `intake.rs` | 539 | Donor intake (PoW, no-JS path, one-time address page with ZIP 321 URI and shred receipt) and the read API |
| `ledger.rs` | 159 | The public, permanent record: shred lines and weekly emitted totals. Nothing donor-linkable |
| `hardening.rs` | 46 | Core dumps off, `PR_SET_DUMPABLE 0`; strict host check (no swap, private tmpfs) when `AMNESIA_STRICT=1` |
| `log.rs` | 59 | Allowlisted JSON events; a value trace exists only with the `dev-trace` cargo feature |
| `static/` | | `style.css` and `pow.js` for the donor page, served same-origin under a strict CSP |

Pure logic (params, denominations, sweep planning, PoW, buckets, clocks) lives in [`amnesia-core`](../amnesia-core/README.md).

---

## HTTP surface

| Route | Port | Returns |
|---|---|---|
| `GET /`, `GET /donate`, `POST /donate` | 8081 (onion only) | The donor page, the PoW form, the one-time address page. `Accept: text/plain` gets `address=`, `uri=zcash:…`, `valid_until_utc=`, `receipt=` |
| `GET /v1/health` | 8082 | `fresh`, `synced`, `intake` (`open` or `not_synced`), live epoch bucket |
| `GET /v1/summary` | 8082 | Profile, timing, shredded and lost counts, swept bucket |
| `GET /v1/shred-log?limit=&offset=` | 8082 | Shred lines, newest first, up to 1,000 per page |
| `GET /v1/audit/weeks` | 8082 | Weekly forgotten counts and swept totals to 2 significant figures |
| `GET /v1/treasury` | 8082 | Swept-since-boot bucket |
| `GET /v1/info` | 8082 | Version, network, profile, `binary_sha256`, `Cargo.lock` hash, toolchain, build recipe, live params |

Every response is `no-store`. HTML gets a strict CSP and no cookies. No route takes an address and there is no per-address lookup.

---

## Configuration

| Variable | Default | Meaning |
|---|---|---|
| `TREASURY_UA` | *(required)* | Unified address the sweeps pay. Keep its spending key off this host |
| `AMNESIA_PROFILE` | `dev` | `dev`, `demo` or `prod` (table below) |
| `AMNESIA_ROOT` | `var/desk` | Ledger and state root |
| `AMNESIA_EPOCHS_DIR` | under the root | Where epoch directories live: point it at a RAM disk or a `noswap` tmpfs |
| `AMNESIA_LWD_HOST` | `testnet.zec.rocks` | lightwalletd host |
| `AMNESIA_TOR` | `1` | Reads and broadcasts over arti; there is no clearnet fallback |
| `AMNESIA_POW_BITS` | `18` | Proof-of-work difficulty for the donor page (the Mac host runs 16) |
| `AMNESIA_DONOR_ADDR` | `127.0.0.1:8081` | Donor intake listener |
| `AMNESIA_READ_ADDR` | `127.0.0.1:8082` | Read API listener |
| `AMNESIA_STRICT` | `0` | `1` refuses to start on a host with swap or without the private tmpfs |
| `AMNESIA_STATUS`, `AMNESIA_DEV_TRACE` | `0` | Debug output. Never set in production |

### Profiles

| Profile | Epoch | Grace | Burial | Addresses per epoch | Ops per hour | Use |
|---|---|---|---|---|---|---|
| `dev` | 15 min | 15 min | 20 blocks | 10 | 60 | Local development only |
| `demo` | 2 h | 2 h | 600 blocks | 50 | 12 | The public demo, so judges can watch a shred |
| `prod` | 12 h | 12 h | 600 blocks | 500 | 30 | A real desk |

All profiles share a smallest sweep denomination of 0.001 TAZ, a carry of 0.005 to 0.035 TAZ, at most 4 canonical notes per sweep, 10 confirmations before a note counts as spendable (3 in `dev`) and a transaction expiry of 40 blocks.

---

## Build and test

```bash
cargo build --release --locked -p amnesia-desk
cargo test --release -p amnesia-desk
```

Release builds keep `overflow-checks = true`. The running binary publishes its own SHA-256 in `/v1/info`; compare it with your build.
