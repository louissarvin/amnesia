# Security

Amnesia is a Zcash Testnet donation desk that forgets its donors. Paths are relative to the repo root. Host facts were checked on 2026-10-26.

## 1. Goal and adversaries

The goal is forward unspendability (zcash/zcash#2044). Each donor gets a one-time address. Hours after its deadline, every key that could link it to the desk is destroyed. A late payment is lost; a later seizure finds nothing to link.

| Adversary | Gets | Should not learn |
|-|-|-|
| Seizure after shred | The full host: files, logs, and RAM of the desk process | Which donors paid a shredded epoch |
| Chain observer | All desk transactions | Donors, amounts, which transactions are the desk's |
| lightwalletd operator | Reads and broadcasts, over Tor | The host IP, and a link between reads and broadcasts |
| Exchange records | "Client c withdrew v to address Z at time T" | That Z belonged to the desk, once Z's epoch is shredded. Amount and timing correlation is reduced, not removed (section 4) |

Not protected:

- A live compromise during an epoch. The desk holds every live epoch's keys until that epoch shreds.
- Donors who pay from a transparent address or straight from an exchange. Their own records persist.
- A global passive adversary on Tor.

## 2. Mechanisms

- **One-time keys.** Each epoch creates `cap + 1` accounts, each from 32 OS-random bytes (`crates/amnesia-desk/src/wallet.rs:81-92`). Addresses carry an Orchard receiver only (`wallet.rs:115-117`). Desk transactions contain only Ironwood actions (`wallet.rs:311-315`).
- **Forward slots at issuance.** A forward time is drawn when the address is issued, not when a payment arrives (`epoch.rs:193-211`). Merges and sweeps run on a Poisson clock (`desk.rs:125-128`).
- **Transaction shape.** At most 2 spends and 2 outputs (`wallet.rs:229-232`). Exactly 2 actions and a 10,000 zat ZIP 317 fee are asserted (`wallet.rs:288-290`, `amnesia-core/src/lib.rs:12`). Outputs pass no OVK, the builder equivalent of `OvkPolicy::Discard` (`wallet.rs:280-286`). Expiry is target height plus 40 (`params.rs:35`, `wallet.rs:257`).
- **Denominations and carry.** Sweeps pay the treasury canonical 1, 2, 5 x 10^k notes (`amnesia-core/src/denom.rs`). A random carry between 0.005 and 0.035 TAZ moves to the next epoch (`plan.rs:26-50`, `params.rs:31-33`).
- **Shred.** Once an epoch is swept and its last transaction is 600 blocks deep (`desk.rs:373-399`), every file is overwritten with random bytes, fsynced and unlinked, then the directory is removed (`epoch.rs:249-294`). Seeds and addresses are zeroized (`wallet.rs:47-58`).
- **Memory.** A global allocator zeroes every Rust heap block on free (`main.rs:12-38`). The Mac host sets `MallocScribble=1` for the C heap (`infra/mac/run-desk.sh:11`). The Linux compose sets `MALLOC_PERTURB_` and glibc tunables that disable tcache and fastbins, on desk and tor (`infra/compose.yaml:40-43`, `:72-73`). Core dumps are off (`hardening.rs:7-19`).
- **Tor.** Reads use an isolated arti circuit, replaced every 30 min (`chain.rs:61-71`, `main.rs:101-104`). Each broadcast attempt uses its own isolated circuit (`chain.rs:92-100`). A failed Tor bootstrap is fatal; there is no clearnet fallback (`chain.rs:42-48`). Each broadcast waits a random 20 to 120 s (`main.rs:139-147`).
- **Web surface.** No cookies. HTML gets a strict CSP and `no-store` (`intake.rs:32-33`, `:134-142`); JSON gets `no-store` (`:343-347`). No URL holds an address and there is no lookup route (`:114-132`).
- **Shred receipt.** The address page also returns the epoch id as a receipt (`intake.rs`). It is random and appears publicly only when that epoch's line is added to the shred log. The web UI looks it up by fetching the whole log and matching in the browser, so the desk never learns which receipt is being checked. A donor's device holding the receipt reveals only the epoch, which the deadline on the same page already implies.
- **Logs.** Allowlisted event kinds and fields only (`log.rs:3-34`). The value trace exists only with the `dev-trace` feature (`log.rs:41-59`).
- **Intake.** Fixed-difficulty SHA-256 PoW; challenges are single use, expire in 600 s, capped at 4,096 (`intake.rs:43-44`, `:190-201`, `:261-262`). Without JavaScript, an empty nonce is accepted after 60 s (`:46-48`, `:250-260`). A stale desk refuses to issue (`:275-278`).
- **Reorgs.** On a continuity error the wallet rewinds 10 blocks below the fork, truncates the cache and rescans (`chain.rs:237-247`).
- **Crash safety.** In-flight transactions and pending treasury emits are persisted in `state.json` (`epoch.rs:53-60`, `:236-247`).
- **Integer overflow.** `overflow-checks = true` in release for all three crates (`Cargo.toml:27-36`).

**Current host.** A Mac under launchd (`infra/mac/`). Epochs live on a `ram://` disk at `/Volumes/AmnesiaRAM` (`infra/mac/ramdisk.sh`). Tor 0.4.9.14, built from signed source with the GPL option, reports `pow: yes`; the onion enables PoW and intro DoS defense (`infra/mac/torrc.mac:12-13`). Donor intake (127.0.0.1:8081) is reachable only through the onion. The read API (:8082) and lab (:8090) bind loopback and are published only under `/v1` and `/demo` at `https://amnesia.taila8827e.ts.net` through Tailscale Funnel; `/donate` and `/` return 404 there. Funnel sets `X-Forwarded-For` to the visitor's real IP and drops any client-supplied value (measured Oct 26), so the lab keys its rate limits on it; the lab holds those IPs in memory for cooldowns only and never logs them. Tailscale terminates TLS for this public surface, so Tailscale can see judge traffic to the lab and read API, never donor traffic.

