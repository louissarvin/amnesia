<div align="center">

<img src="web/public/favicon.svg" alt="Amnesia" width="96" />

# Amnesia

### A donation desk that forgets.

Amnesia is a Zcash donation desk built to forget its donors. Every donor gets a one-time shielded address. A few hours after that address's deadline, the desk destroys every key that could link the payment back to it. Seize the server after that and there is nothing left to read: not in the wallet files, not in the logs, not in the process memory. A normal wallet, seized the same way, names every donor it ever had.

<br/>

![Zcash](https://img.shields.io/badge/Zcash-Testnet_(NU7)-F4B728)
![Orchard](https://img.shields.io/badge/Pool-Orchard_only-0B0B0C)
![Rust](https://img.shields.io/badge/Rust-1.93.1-000000?logo=rust&logoColor=white)
![librustzcash](https://img.shields.io/badge/librustzcash-zcash__client__backend_0.25-F4B728)
![Tor](https://img.shields.io/badge/Tor-arti_0.43_+_tor_0.4.9.14_(PoW)-7D4698?logo=torproject&logoColor=white)
![axum](https://img.shields.io/badge/axum-0.8-000000)
![TanStack Start](https://img.shields.io/badge/Frontend-TanStack_Start-FF4154)
![React](https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=black)
![License](https://img.shields.io/badge/License-MIT_or_Apache_2.0-green)

**[Live demo](https://amnesia.taila8827e.ts.net)** · **[Security model](SECURITY.md)** · **[Run your own desk](https://amnesia.taila8827e.ts.net/operate)**

</div>

---

## The problem

Anyone who takes donations keeps a list of donors, whether they mean to or not. A normal wallet stores every incoming payment, the address it arrived on, and the time. That database is the donor list. It outlives the campaign, it outlives the people who promised to protect it, and it is the first thing taken when a server is seized, subpoenaed, breached or sold.

This is not hypothetical:

| What happened | Source | Date |
|---|---|---|
| "GiveSendGo suffers hack related to 'Freedom Convoy', data leaked": donor names, emails, zip codes and IP addresses | [Global News](https://globalnews.ca/news/8618116/givesendgo-suffers-hack-related-to-freedom-convoy-data-leaked) | Feb 14, 2022 |
| US Supreme Court strikes down California's demand for charities' donor lists (Schedule B), 6 to 3, citing the chilling effect even of confidential disclosure | [Congressional Research Service](https://www.everycrsreport.com/reports/LSB10621.html) | Jul 1, 2021 |
| *NAACP v. Alabama*: the Court blocks a state from forcing the NAACP to hand over its membership list, because exposure invites harassment and reprisal | [Congressional Research Service](https://www.everycrsreport.com/reports/LSB10621.html) | 1958 |

Courts can protect a list. Breaches and seizures ignore that protection. The only list that cannot leak is one that no longer exists.

Zcash already hides payments on chain. It does not help once the receiver's own wallet is taken: the wallet can decrypt everything it ever received. The protocol request for this, forward unspendability with keys you can provably throw away, has been open for years: [zcash/zcash#2044](https://github.com/zcash/zcash/issues/2044). Amnesia builds it as a product.

