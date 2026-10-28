<div align="center">

<img src="../../web/public/assets/logo.png" alt="Amnesia" width="84" />

# amnesia-core

### The pure logic: no IO, no chain, randomness injected.

Everything the desk decides that can be decided without a network: profiles, the transaction shape, sweep denominations, the carry, Poisson clocks, the proof of work, and the coarse buckets that are the only numbers the desk ever publishes. Unit-tested on its own.

<br/>

![Rust](https://img.shields.io/badge/Rust-1.93.1-000000?logo=rust&logoColor=white)
![no IO](https://img.shields.io/badge/IO-none-0B0B0C)
![Tests](https://img.shields.io/badge/tests-16_passing-6E9F18)

**[Back to the project](../../README.md)** · **[The desk that uses it](../amnesia-desk/README.md)**

</div>

---

## Modules

| File | What it decides | Tests |
|---|---|---|
| `lib.rs` | `FEE_2_ACTIONS = 10_000`: the ZIP 317 fee for the only transaction shape the desk builds | |
| `params.rs` | The three profiles, `dev`, `demo` and `prod`, and every timing and amount knob | |
| `plan.rs` | Sweep planning and op selection. Every op spends at most 2 notes and creates at most 2 outputs, so its Orchard bundle is exactly 2 actions | 5 |
| `denom.rs` | Canonical 1, 2, 5 times a power of ten denominations, and decomposing a total into at most `max_canonical` of them | 3 |
| `clock.rs` | Exponential gaps for the Poisson op clock, and uniform slots drawn at issuance | 2 |
| `rng.rs` | Bias-free uniform integers and 53-bit floats from any injected RNG | 2 |
| `pow.rs` | The intake's issuance-flooding guard: `SHA-256(salt, id, nonce)` needs `bits` leading zero bits | 2 |
| `buckets.rs` | Coarse public values: amounts to 2 significant figures, issued counts as `0`, `1-9`, `10-49`, `50+`, swept totals as `0`, `lt_0.1`, `0.1-1`, `1-10`, `10+` | 2 |

---

## Why it is separate

- **Testable without a chain.** Planning and denominations are where a privacy bug would hide (an odd amount, a third action, a predictable time). Here they are plain functions over integers, tested with seeded RNGs across edge values such as one fee, two fees, the carry bounds and the smallest denomination.
- **Randomness is a parameter.** Every function that draws takes `&mut R: Rng`, so production passes the OS RNG and tests pass a seeded one.
- **The public surface is defined in one place.** If a number is not produced by `buckets.rs`, the desk does not publish it.

---

## Build and test

```bash
cargo test --release -p amnesia-core     # 16 tests
```
