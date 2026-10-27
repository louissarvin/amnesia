import { useQuery } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { useEffect, useState } from 'react'
import type { YourDonation } from '@/lib/api'
import Eyebrow from '@/components/Eyebrow'
import Headline from '@/components/Headline'
import { storedDonation, summaryQuery } from '@/lib/api'
import { shredWindow } from '@/lib/forget'
import { useShredReceipt } from '@/lib/receipt'
import { formatUtc } from '@/utils/format'
import { cnm } from '@/utils/style'

type State = 'done' | 'now' | 'wait' | 'later'

const left = (seconds: number) => {
  const m = Math.max(0, Math.round(seconds / 60))
  return m >= 60 ? `${Math.floor(m / 60)} h ${m % 60} min` : `${m} min`
}

// Quest-style path through the demo, driven by the donation this device remembers.
export default function JudgePath() {
  const summary = useQuery(summaryQuery)
  const [you, setYou] = useState<YourDonation | null>(null)
  const [now, setNow] = useState(() => Date.now() / 1000)
  useEffect(() => {
    setYou(storedDonation())
    const id = window.setInterval(() => setNow(Date.now() / 1000), 30_000)
    return () => window.clearInterval(id)
  }, [])

  const w = you?.validUntil && summary.data ? shredWindow(you.validUntil, summary.data) : null
  const row = useShredReceipt(you?.receipt)
  // The public log is proof; the time window is only the fallback for donations without a receipt.
  const shredded = row ? true : row === null ? false : Boolean(w && now >= w.to)
  const steps: Array<{ to: '/donate' | '/seize'; title: string; note: string; state: State }> = [
    {
      to: '/donate',
      title: 'Donate',
      note: you ? `Done. You are ${you.client}.` : 'One click, no wallet. Or pay for real over Tor.',
      state: you ? 'done' : 'now',
    },
    {
      to: '/seize',
      title: 'Seize both wallets',
      note: you ? `Find ${you.client} named by both.` : 'The normal wallet and Amnesia, same script.',
      state: !you ? 'later' : shredded ? 'done' : 'now',
    },
    {
      to: '/seize',
      title: 'Wait for the shred',
      note: w
        ? row
          ? `Receipt found in the shred log: keys destroyed ${formatUtc(row.closed_hour)}.`
          : shredded
          ? `Keys destroyed by ${formatUtc(w.to)}.`
          : `Keys gone in at most ${left(w.to - now)}, by ${formatUtc(w.to)}.`
        : 'About 5 to 9 hours after the address deadline.',
      state: !w ? 'later' : shredded ? 'done' : 'wait',
    },
    {
      to: '/seize',
      title: 'Seize again',
      note: you ? `${you.client} should be gone from the Amnesia side.` : 'The same seizure no longer names you.',
      state: shredded ? 'now' : 'later',
    },
  ]

  return (
    <section data-rail="Judge" className="cell px-5 py-16 md:px-10 md:py-24">
      <div className="flex flex-col gap-6 md:flex-row md:items-end md:justify-between">
        <div>
          <Eyebrow live={Boolean(you)}>{you ? `your run, ${you.client}` : 'judge it yourself'}</Eyebrow>
          <Headline className="display mt-5 max-w-2xl text-4xl md:text-6xl">Four steps, one of them is waiting.</Headline>
        </div>
        <p className="max-w-sm text-muted">
          This device remembers your donation id, nothing else, so each step can show where you are.
        </p>
      </div>
      <div aria-hidden="true" className="mt-10 grid grid-cols-4 gap-1.5">
        {steps.map((s) => (
          <span
            key={s.title}
            className={cnm(
              'h-1 rounded-full transition-colors duration-500',
              s.state === 'done' ? 'bg-gold' : s.state === 'later' ? 'bg-line-strong' : 'bg-fg/40',
            )}
          />
        ))}
      </div>
      <ol className="mt-6 grid grid-cols-1 gap-px bg-line sm:grid-cols-2 lg:grid-cols-4">
        {steps.map((s, i) => (
          <li key={s.title} className="bg-bg">
            <Link
              to={s.to}
              className={cnm('group flex h-full flex-col gap-3 p-5 transition-colors hover:bg-line md:p-6', s.state === 'later' && 'opacity-55')}
            >
              <span className="flex items-center justify-between font-mono text-xs text-muted">
                <span className="tabular-nums">{String(i + 1).padStart(2, '0')}</span>
                <span className={cnm('uppercase tracking-[0.06em]', s.state === 'done' && 'text-gold-ink')}>
                  {s.state === 'done' ? 'done' : s.state === 'now' ? 'next' : s.state === 'wait' ? 'waiting' : 'later'}
                </span>
              </span>
              <span className="text-lg font-medium tracking-[-0.02em]">{s.title}</span>
              <span className="text-sm text-muted">{s.note}</span>
              <span aria-hidden="true" className="mt-auto pt-2 font-mono text-xs text-muted transition-transform group-hover:translate-x-0.5">
                →
              </span>
            </Link>
          </li>
        ))}
      </ol>
    </section>
  )
}
