import { gsap } from 'gsap'
import { ScrollTrigger } from 'gsap/ScrollTrigger'
import { useEffect, useRef } from 'react'
import Eyebrow from '@/components/Eyebrow'
import Headline from '@/components/Headline'

gsap.registerPlugin(ScrollTrigger)

const GLYPHS = '0123456789abcdefghjkmnpqrstuvwxyz'
const AHEAD = 4

// Illustration only: shaped like desk receipts, generated from a fixed seed, never real addresses.
function sampleAddress(seed: number) {
  let s = seed * 2654435761
  let out = 'utest1'
  for (let i = 0; i < 38; i++) {
    s = (s * 1103515245 + 12345) >>> 0
    out += GLYPHS[s % GLYPHS.length]
  }
  return out
}

const ROWS = [
  ['2026-10-12 09:00', '1.00 TAZ'],
  ['2026-10-12 09:14', '0.50 TAZ'],
  ['2026-10-12 10:02', '0.20 TAZ'],
  ['2026-10-12 11:47', '0.20 TAZ'],
  ['2026-10-12 12:31', '0.50 TAZ'],
  ['2026-10-12 15:08', '1.00 TAZ'],
].map(([when, amount], i) => ({ when, amount, address: sampleAddress(i + 7) }))

interface RowParts {
  done: HTMLSpanElement
  cursor: HTMLSpanElement
  ahead: HTMLSpanElement
  rest: HTMLSpanElement
  tag: HTMLSpanElement
}

// Paints one row at local progress t: redacted behind the gold cursor, noise just ahead of it.
function paint(parts: RowParts, address: string, t: number) {
  const len = address.length
  const k = Math.round(Math.min(1, Math.max(0, t)) * len)
  if (k >= len) {
    parts.done.textContent = '█'.repeat(len)
    parts.cursor.textContent = ''
    parts.ahead.textContent = ''
    parts.rest.textContent = ''
    parts.tag.textContent = 'redacted'
    return
  }
  let noise = ''
  for (let i = k + 1; i < Math.min(len, k + 1 + AHEAD); i++) {
    noise += GLYPHS[Math.floor(Math.random() * GLYPHS.length)]
  }
  parts.done.textContent = '█'.repeat(k)
  parts.cursor.textContent = k > 0 ? address[k] : ''
  parts.ahead.textContent = k > 0 ? noise : ''
  parts.rest.textContent = address.slice(k > 0 ? k + 1 + noise.length : 0)
  parts.tag.textContent = ''
}

export default function Redaction() {
  const listRef = useRef<HTMLOListElement>(null)
  const partsRef = useRef<Array<RowParts | null>>([])

  useEffect(() => {
    const list = listRef.current
    if (!list) return
    const all = (t: number) => partsRef.current.forEach((p, i) => p && paint(p, ROWS[i].address, t))
    const mm = gsap.matchMedia()
    mm.add('(prefers-reduced-motion: no-preference)', () => {
      all(0)
      const st = ScrollTrigger.create({
        trigger: list,
        start: 'top 75%',
        end: 'bottom 35%',
        scrub: true,
        onUpdate: (self) => {
          // Rows redact one after another, each overlapping the next a little.
          const span = ROWS.length * 0.7 + 0.3
          partsRef.current.forEach((p, i) => {
            if (p) paint(p, ROWS[i].address, (self.progress * span - i * 0.7) / 1)
          })
        },
      })
      return () => st.kill()
    })
    mm.add('(prefers-reduced-motion: reduce)', () => all(1))
    return () => mm.revert()
  }, [])

  return (
    <section data-rail="Address" className="cell px-5 py-24 md:px-10 md:py-32">
      <div className="flex flex-col items-center text-center">
        <Eyebrow>illustration, not real data</Eyebrow>
        <Headline className="display mt-6 max-w-3xl text-4xl sm:text-5xl md:text-6xl">
          Every address is used once, then redacted.
        </Headline>
      </div>
      <ol
        ref={listRef}
        aria-label="Example ledger rows, every address redacted"
        className="card spot mx-auto mt-14 max-w-4xl divide-y divide-line overflow-hidden rounded-2xl border border-line font-mono text-[12px] sm:text-[13px]"
      >
        {ROWS.map((row, i) => (
          <li
            key={row.when}
            className="flex items-center gap-4 px-5 py-4 sm:gap-8 sm:px-7"
            ref={(el) => {
              if (!el) return
              const q = (sel: string) => el.querySelector<HTMLSpanElement>(sel)
              const done = q('[data-p=done]')
              const cursor = q('[data-p=cursor]')
              const ahead = q('[data-p=ahead]')
              const rest = q('[data-p=rest]')
              const tag = q('[data-p=tag]')
              partsRef.current[i] = done && cursor && ahead && rest && tag ? { done, cursor, ahead, rest, tag } : null
            }}
          >
            <span className="text-muted hidden shrink-0 tabular-nums sm:inline">{row.when}</span>
            <span className="shrink-0 tabular-nums">+{row.amount}</span>
            <span aria-hidden="true" className="min-w-0 flex-1 truncate">
              <span data-p="done" className="text-fg/25" />
              <span data-p="cursor" className="bg-gold text-bg" />
              <span data-p="ahead" className="text-gold-ink" />
              <span data-p="rest">{row.address}</span>
            </span>
            {/* Outside the truncated address, so the label is never cut off on narrow screens. */}
            <span aria-hidden="true" data-p="tag" className="text-muted shrink-0 uppercase tracking-[0.08em] empty:hidden" />
          </li>
        ))}
      </ol>
      <p className="text-muted mx-auto mt-6 max-w-4xl text-center font-mono text-[11px] tracking-[0.04em] uppercase">
        A shredded epoch leaves no address to read. The public shred log keeps only that the epoch existed.
      </p>
    </section>
  )
}
