<div align="center">

<img src="public/assets/logo.png" alt="Amnesia" width="84" />

# Amnesia web

### The judge UI: donate, seize, watch it forget.

The public face of the Amnesia demo. Every number on it comes from the running desk (`/v1`) or the lab (`/demo`). Nothing is mocked, nothing is tracked, and nothing loads from a third party.

<br/>

![TanStack Start](https://img.shields.io/badge/TanStack_Start-SPA-FF4154)
![React](https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=black)
![Tailwind](https://img.shields.io/badge/Tailwind-4-06B6D4?logo=tailwindcss&logoColor=white)
![TypeScript](https://img.shields.io/badge/TypeScript-strict-3178C6?logo=typescript&logoColor=white)
![Bun](https://img.shields.io/badge/Runtime-Bun_1.3-000000?logo=bun&logoColor=white)
![Tests](https://img.shields.io/badge/Vitest-8_passing-6E9F18)

**[Open the live site](https://amnesia.taila8827e.ts.net)** · **[Back to the project](../README.md)**

</div>

---

## Routes

| Route | What a judge does there | Data it reads |
|---|---|---|
| `/` | Watches the glass key shatter, follows the four-step judge path, reads the lifecycle and FAQ | `/v1/summary`, `/v1/shred-log`, `/demo/history`, the device's own donation record |
| `/donate` | Makes a lab donation in one click, or gets the onion link and QR to pay for real over Tor | `/demo/donate`, `/demo/runs/{id}`, `/v1/health`, `/v1/summary`, `/demo/status`, `/v1/treasury` |
| `/seize` | Holds to seize the normal wallet and the desk, then finds their own client id named, or gone | `/demo/seize`, `/demo/seizures`, `/demo/seizures/{id}/report.json`, `/demo/history` |
| `/audit` | Reads every shredded epoch, sees their own receipt highlighted once it shreds | `/v1/shred-log`, `/v1/audit/weeks`, `/v1/summary` |
| `/verify` | Copies the binary hash, the Cargo.lock hash and the build recipe, opens the raw endpoints | `/v1/info` |
| `/operate` | Reads how an organisation runs its own desk and copies the donate link for its site | static, from `infra/README.md` |

## How the demo follows a judge

The site remembers one record on the judge's device (`localStorage`, key `amnesia:your-donation`): the client id, issue time, deadline and shred receipt of their latest donation. It never leaves the browser.

1. **Donate** stores the record when the address is issued.
2. **Seize** highlights that client id in both reports and says why it was found.
3. **The shred window** is computed from the deadline and the desk's own phases (`src/lib/forget.ts`).
4. **Gone** is proven, not guessed: the browser downloads the public shred log and looks for the receipt locally (`src/lib/receipt.ts`), so the desk never learns whose receipt is being checked.

---

## Design

| Rule | Why |
|---|---|
| One background (`#07080A`, paper `#F3F1EF`), hairline cells, no gradients | Separation by lines and space, so the data carries the page |
| Gold `#F4B728` only for live data and exposed names | When gold appears, something real is being shown |
| Geist and Geist Mono, self-hosted | No font CDN, no third-party request |
| Motion: line reveals, scramble on hover, the shattering key, the redaction sweep, the shredder | Every animation explains forgetting; all of it respects `prefers-reduced-motion` |
| Layout shift under 0.03 on every route | Skeletons hold the final shape while data loads |

Patterns borrowed and adapted: Vercel's split FAQ, Across's result banner ("mined in 3:45 · keys gone by"), Layer3's segmented quest progress for the judge path.

---

## Privacy and security

- **No analytics, no cookies, no third-party requests.** Fonts and the QR encoder ship with the bundle.
- **Strict CSP** set by the lab: scripts only from this origin plus the exact inline scripts of the build, by hash.
- **No raw HTML.** The QR code is drawn as React `<path>` data from `uqr`'s matrix, never injected markup.
- **Every API response is validated** with zod before use (`src/lib/api.ts`). Unknown run events fail closed.
- **External links** open with `rel="noopener noreferrer"`. The only one is the third-party explorer, shown after a transaction is mined.

---

## Structure

| Path | What |
|---|---|
| `src/routes/` | One file per route, file-based routing |
| `src/lib/api.ts` | Typed client for `/v1` and `/demo`, schemas, query options, the device's donation record |
| `src/lib/forget.ts` | Shred window from the desk's phase timing |
| `src/lib/receipt.ts` | Browser-side shred receipt lookup |
| `src/components/home/` | Hero and glass key (three.js), judge path, redaction, lifecycle, live cards, FAQ |
| `src/components/seize/` | Seize panels and report view |
| `src/components/reactbits/` | Adapted React Bits components: Shredder, HoldButton, StatusMark, DecryptedText, CountUp, Noise |
| `src/components/{DeskPulse,OnionDonate}.tsx` | Live desk numbers; the real donation path over Tor |

---

## Run locally

```bash
bun install
bun dev          # http://localhost:3200, proxies /v1 to the desk on :8082 and /demo to the lab on :8090
bun run test     # 8 tests
bun run build    # dist/client, an SPA shell plus hashed assets
```

## Deploy

The lab serves the build as static files next to its API. On the Mac host one script builds, gzips text assets for the lab's precompressed path, swaps the directory and restarts the lab:

```bash
sh ../infra/mac/deploy-web.sh
```

Measured after deploy: the main script ships at 192 KB gzipped instead of 592 KB, and every route stays under 0.03 layout shift on a 390 px phone.
