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

