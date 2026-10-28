<div align="center">

<img src="../../web/public/assets/logo.png" alt="Amnesia" width="84" />

# Amnesia on a Mac

### The host the live demo runs on today.

Five launchd agents run the desk, the lab, the donor onion, a keep-awake and a watchdog; the Tailscale app carries the public judge URL. Live epochs sit on a `ram://` disk that is never written to the SSD. The public judge surface goes out through Tailscale Funnel; the donor intake goes out only through Tor.

<br/>

![macOS](https://img.shields.io/badge/macOS-launchd-000000?logo=apple&logoColor=white)
![Tor](https://img.shields.io/badge/tor-0.4.9.14_(PoW)-7D4698?logo=torproject&logoColor=white)
![Tailscale](https://img.shields.io/badge/Tailscale-Funnel-242424?logo=tailscale&logoColor=white)
![Status](https://img.shields.io/badge/Status-live_since_24_Oct_2026-F4B728)

**[Back to the project](../../README.md)** · **[Linux stack](../README.md)** · **[Live demo](https://amnesia.taila8827e.ts.net)**

</div>

---

## Agents

The runtime lives in `~/amnesia-host`, not under `~/Documents`: macOS TCC blocks launchd agents from reading `~/Documents`.

| Agent | Runs | Notes |
|-|-|-|
| `com.amnesia.desk` | `run-desk.sh` | Demo profile, Tor broadcast, KeepAlive. Epochs on a 256 MB `ram://` disk (`ramdisk.sh`, wired memory, Spotlight off); refuses to start on SSD. `MallocScribble=1` scrubs freed SQLite heap. ExitTimeOut 900 s: SIGTERM waits for the tick |
| `com.amnesia.lab` | `run-lab.sh` | Demo API on 127.0.0.1:8090, bots at 1 donation/h |
| `com.amnesia.tor` | `run-tor.sh` + `torrc.mac` | Donor onion to 127.0.0.1:8081. Tor 0.4.9.14 built from the signed source tarball with `--enable-gpl` (Homebrew's build lacks the PoW module), installed in `~/amnesia-host/tor`. Same hardening as `infra/vps/torrc` |
| Tailscale (Standalone app, launch at login) | `tailscale funnel --bg --https=443 --set-path=/v1 http://127.0.0.1:8082/v1` and the same for `/demo` to :8090 | Public judge URL `https://amnesia.taila8827e.ts.net`. Tailnet policy needs the `funnel` nodeAttr. Donor intake is never published here |
| `com.amnesia.awake` | `caffeinate -i -s -m` | `-s` only holds on AC power: keep the Mac plugged in, lid open |
| `com.amnesia.watchdog` | `watchdog.sh` every 300 s | Alerts on a stale desk, low float, battery, disk under 5 GB. It restarts a service only when that service's port stops answering |

---

## Install

```sh
mkdir -p ~/amnesia-host/bin ~/amnesia-host/var
cargo build --release
cp _spike/zcash-devtool/target/release/{amnesia-desk,amnesia-lab,seize} ~/amnesia-host/bin/
cp infra/mac/*.sh ~/amnesia-host/var/
cp infra/mac/*.plist ~/Library/LaunchAgents/
for a in desk lab awake watchdog; do launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.amnesia.$a.plist; done
```

Deploy a new binary: copy it into `bin/` (keep the old one as `.prev`), then `launchctl kickstart -k gui/$(id -u)/com.amnesia.desk` (or `.lab`). The desk shuts down gracefully on SIGTERM and resyncs over Tor on boot; check `/v1/health` until `fresh` is true.

Deploy the web UI in one step (build, gzip text assets, swap, restart the lab):

```sh
sh infra/mac/deploy-web.sh
```

On an 8 GB Mac, build Rust gently so the desk keeps its CPU: `nice -n 19 cargo build --release -j 1 -p amnesia-lab`.

---

## Checks

- `curl 127.0.0.1:8082/v1/health`: `fresh` is false when no tick has succeeded for 15 min. Intake then reports `not_synced`.
- `curl 127.0.0.1:8082/v1/info`: `binary_sha256` must match `shasum -a 256 ~/amnesia-host/bin/amnesia-desk`.
- `var/watchdog.log`: one JSON line per alert.

---

## Erasure on the Mac: what holds and what does not

- Epoch files live only on the RAM disk. A reboot loses live epochs; the ledger then records them as `lost`.
- The ledger, Tor state, logs and lab wallets live on the SSD. None of them is donor-linked.
- macOS swap is encrypted but cannot be turned off. Rust heap is zeroed on free; libmalloc scribbles the C heap.
- For the strongest claim, run the compose stack on a Linux VPS: tmpfs with `noswap`, `init_on_free=1`.

---

## Building tor with PoW

Follow https://support.torproject.org/little-t-tor/verify-little-t-tor: fetch the release keys by fingerprint, then `gpgv` the `.sha256sum.asc` and `shasum -a 256 -c`. Then:

```sh
./configure --enable-gpl --prefix=$HOME/amnesia-host/tor \
  --with-openssl-dir=$(brew --prefix openssl@3) --with-libevent-dir=$(brew --prefix libevent) \
  --disable-asciidoc --disable-unittests --disable-manpage --disable-html-manual
make -j2 && make install
~/amnesia-host/tor/bin/tor --list-modules   # must show "pow: yes"
```

---

## Operating lessons

Learned on this host, so the next operator does not have to:

| What happened | Effect | Rule |
|---|---|---|
| Unplugged on 27 Oct 2026, the Mac slept for about 2 h | Demo offline; the desk woke 2 h behind and briefly refused addresses | Keep it on AC power, lid open. `caffeinate -s` holds only on AC |
| Swap near full and load over 60 from other apps on the same Mac | Slow ticks, intake `not_synced`, Rust builds killed | Run nothing heavy next to the desk; stop unrelated Docker containers |
| Wi-Fi outage on 26 Oct 2026 | Whole-range compact block downloads over Tor kept failing and restarting | Fixed in code: sync in 100 block chunks |
| A testnet reorg at height 4,542,700 | Wallets wedged on a continuity error | Fixed in code: rewind 10 blocks below the fork and rescan |
