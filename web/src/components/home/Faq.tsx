import { useQuery } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import Eyebrow from '@/components/Eyebrow'
import Headline from '@/components/Headline'
import { summaryQuery } from '@/lib/api'

const ITEMS = [
  {
    q: 'What if the desk is seized while an epoch is live?',
    a: 'Then that epoch is exposed. The desk holds the keys of every live epoch until it shreds, and one process holds all of them, so a live compromise can name those donors. The protection starts at the shred: a seizure after it finds nothing that links a donor to the desk. The lab shows both cases.',
  },
  {
    q: 'Why Testnet?',
    a: 'Amnesia is a research prototype with known limits, and testnet lets anyone donate, seize and break it without risking funds. Some parameters are fixed to current testnet rules, like the 10,000 zat fee and a 40 block expiry, and would need work before anything real.',
  },
  {
    q: 'What does the lab simulate?',
    a: 'The donors and the exchange. Lab bots donate, and the exchange records ("client c withdrew v to address Z") are synthetic. The seizure itself runs against the real desk files. On the current Mac host it covers files only, not RAM.',
  },
  {
    q: 'Can a judge verify the binary?',
    a: '/v1/info reports the binary SHA-256 and the build recipe, and the hash matched the installed binary when checked on Oct 26. There is no hardware attestation: tee is none and the git commit is unknown. The shred log is self-reported and unsigned.',
  },
  {
    q: 'Why are amounts in round denominations?',
    a: 'Sweeps pay the treasury in canonical 1, 2 and 5 notes of a power of ten, and a small random carry moves to the next epoch. That makes an exchange record of an exact withdrawal say less about which donations it summed. It reduces amount and timing correlation, it does not remove it.',
  },
  {
    q: 'What is not protected?',
    a: 'A live compromise during an epoch, donors who pay from a transparent address or straight from an exchange (their own records persist), and anyone facing a global passive adversary on Tor. The SECURITY file in the repo lists the rest.',
  },
]

const CHECKS = [
  { to: '/seize', label: 'Seize both wallets yourself' },
  { to: '/audit', label: 'Read the shred log' },
  { to: '/verify', label: 'Compare the binary hash' },
] as const

export default function Faq() {
  const summary = useQuery(summaryQuery)

  return (
    <section data-rail="FAQ" className="cell grid grid-cols-1 gap-12 px-5 py-16 md:grid-cols-12 md:gap-8 md:px-10 md:py-24">
      <div className="min-w-0 md:col-span-5">
        <div className="flex flex-col gap-10 md:sticky md:top-28">
          <div>
            <Eyebrow>questions judges ask</Eyebrow>
            <Headline className="display mt-5 max-w-md text-4xl md:text-6xl">Straight answers.</Headline>
            <p className="mt-6 max-w-sm text-muted">
              Including the parts that are not protected. Every claim here can be checked against the running
              desk.
            </p>
          </div>
          <ul className="max-w-sm border-t border-line">
            {CHECKS.map((c) => (
              <li key={c.to} className="border-b border-line">
                <Link
                  to={c.to}
                  className="group flex items-center justify-between gap-4 py-4 font-mono text-xs tracking-[0.04em] uppercase transition-colors hover:text-fg"
                >
                  <span data-scramble>{c.label}</span>
                  <span aria-hidden="true" className="text-muted transition-transform group-hover:translate-x-0.5">
                    →
                  </span>
                </Link>
              </li>
            ))}
          </ul>
          {summary.data ? (
            <Eyebrow live>
              {summary.data.epochs_shredded} epochs shredded / {summary.data.epochs_lost} lost
            </Eyebrow>
          ) : null}
        </div>
      </div>
      <div className="min-w-0 border-t border-line md:col-span-7">
        {ITEMS.map((item, i) => (
          <details key={item.q} name="faq" open={i === 0} className="faq group border-b border-line">
            <summary className="grid grid-cols-[2rem_1fr_auto] items-baseline gap-3 py-6 md:grid-cols-[3.5rem_1fr_auto] md:gap-4">
              <span className="font-mono text-xs text-muted tabular-nums">{String(i + 1).padStart(2, '0')}</span>
              <span className="text-lg font-medium tracking-[-0.02em] text-balance md:text-xl">{item.q}</span>
              <span aria-hidden="true" className="faq-plus font-mono text-xl leading-none text-muted">
                +
              </span>
            </summary>
            <p className="faq-body max-w-2xl pb-8 pl-11 text-muted md:pl-[4.5rem]">{item.a}</p>
          </details>
        ))}
      </div>
    </section>
  )
}
