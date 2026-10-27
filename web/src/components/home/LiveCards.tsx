import { useQuery } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { gsap } from 'gsap'
import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import type { SeizureSummary, ShredRow } from '@/lib/api'
import type { ShredderHandle } from '@/components/reactbits/Shredder'
import Shredder from '@/components/reactbits/Shredder'
import { historyQuery, reportQuery, shredLogQuery } from '@/lib/api'
import { formatTaz, formatUtc, shortId } from '@/utils/format'

const ROWS = 8
const CYCLE_MS = 9000
const REDUCED = '(prefers-reduced-motion: reduce)'

function CardHead({ title, note }: { title: string; note: string }) {
  return (
    <div className="flex items-center justify-between gap-4">
      <p className="flex items-center gap-2 font-mono text-[11px] tracking-[0.08em] uppercase">
        <span aria-hidden="true" className="live-dot size-1.5 rounded-full bg-gold" />
        {title}
      </p>
      <p className="font-mono text-[11px] tracking-[0.04em] text-muted uppercase">{note}</p>
    </div>
  )
}

function Status({ children }: { children: ReactNode }) {
  return <p className="font-mono text-sm text-muted">{children}</p>
}

function ShredLogCard() {
  const { data, isError } = useQuery(shredLogQuery(ROWS, 0))
  const shredder = useRef<ShredderHandle>(null)
  const stage = useRef<HTMLDivElement>(null)
  const [inView, setInView] = useState(false)
  const [run, setRun] = useState(0)
  const rows = data?.rows
  const newestId = rows?.[0]?.epoch_id

  useEffect(() => {
    const el = stage.current
    if (!el) return
    const io = new IntersectionObserver(([e]) => setInView(e.isIntersecting))
    io.observe(el)
    return () => io.disconnect()
  }, [])

  useEffect(() => {
    if (!inView || !newestId || window.matchMedia(REDUCED).matches) return
    const shred = window.setTimeout(() => shredder.current?.shred(newestId), 1400)
    const next = window.setTimeout(() => setRun((n) => n + 1), CYCLE_MS)
    return () => {
      window.clearTimeout(shred)
      window.clearTimeout(next)
    }
  }, [inView, newestId, run])

  return (
    <article className="spot flex flex-col gap-5 rounded-3xl border border-line p-6 md:p-8">
      <CardHead title="Shred log, live" note={data ? `${data.total} epochs` : '/v1/shred-log'} />
      <div ref={stage} className="flex flex-1 justify-center">
        {rows ? (
          <Shredder<ShredRow & { id: string }>
            key={run}
            shredRef={shredder}
            width={460}
            height={470}
            gap={8}
            fallHeight={80}
            slitColor="var(--line-strong)"
            color="var(--fg)"
            items={[...rows].reverse().map((r) => ({ ...r, id: r.epoch_id }))}
            renderItem={(r) => (
              <div className="flex h-10 items-center justify-between gap-3 rounded-xl border border-line-strong bg-bg px-3 font-mono text-[11px]">
                <span>{shortId(r.epoch_id)}</span>
                <span className="hidden text-muted sm:inline">{formatUtc(r.closed_hour)}</span>
                <span>{r.closed_kind}</span>
              </div>
            )}
          />
        ) : (
          <Status>{isError ? 'The shred log is unavailable right now.' : 'Loading the latest rows…'}</Status>
        )}
      </div>
      <p className="text-xs text-muted">
        Real rows from the log, newest on the slit. The shredding is a picture: the desk deleted these keys when each
        epoch was shredded.
      </p>
    </article>
  )
}

const newest = (list: Array<SeizureSummary>, target: SeizureSummary['target']) =>
  list.filter((s) => s.target === target).sort((a, b) => b.created - a.created)[0]

interface DemoRow {
  label?: string
  value?: string
}

const BAR_WIDTHS = ['w-3/4', 'w-2/3', 'w-5/6', 'w-1/2', 'w-4/5', 'w-3/5', 'w-2/3', 'w-3/4']
const HIDDEN = 'inset(0 100% 0 0)'

function Cell({ row, i, tone, attr }: { row: DemoRow; i: number; tone: string; attr: string }) {
  return (
    <div {...{ [attr]: '' }} className="flex h-8 items-center justify-between gap-2 border-b border-line font-mono text-[11px]">
      {row.label ? (
        <>
          <span className={tone}>{row.label}</span>
          <span className={tone}>{row.value}</span>
        </>
      ) : (
        <span aria-hidden="true" className={`h-2.5 rounded-sm ${tone === 'text-gold-ink' ? 'bg-gold-ink/70' : 'bg-fg/70'} ${BAR_WIDTHS[i % BAR_WIDTHS.length]}`} />
      )}
    </div>
  )
}

function Column({ title, count, children }: { title: string; count: string; children: ReactNode }) {
  return (
    <div className="flex flex-col">
      <p className="mb-2 flex items-baseline justify-between font-mono text-[11px] tracking-[0.06em] uppercase">
        <span>{title}</span>
        <span className="text-muted">{count}</span>
      </p>
      {children}
    </div>
  )
}

// Self-playing loop: scan line, names type in on the left, redaction bars wipe in on the right, hold, reset.
function Demo({ left, right, leftCount, rightCount, note }: { left: Array<DemoRow>; right: Array<DemoRow>; leftCount: string; rightCount: string; note: string }) {
  const stage = useRef<HTMLDivElement>(null)

  useLayoutEffect(() => {
    const el = stage.current
    if (!el) return
    const mm = gsap.matchMedia()
    mm.add('(prefers-reduced-motion: no-preference)', () => {
      const l = el.querySelectorAll('[data-l]')
      const r = el.querySelectorAll('[data-r]')
      const scan = el.querySelector('[data-scan]')
      gsap.set([l, r], { clipPath: HIDDEN })
      const tl = gsap.timeline({ paused: true, repeat: -1, repeatDelay: 0.5 })
      tl.fromTo(scan, { y: 0, opacity: 1 }, { y: () => el.offsetHeight, opacity: 0.4, duration: 1.2, ease: 'power2.inOut' }, 0)
      tl.to(l, { clipPath: 'inset(0 0% 0 0)', duration: 0.4, ease: 'steps(10)', stagger: 0.18 }, 0.5)
      tl.to(r, { clipPath: 'inset(0 0% 0 0)', duration: 0.6, ease: 'expo.out', stagger: 0.12 }, '>-0.3')
      tl.to({}, { duration: 3 })
      const io = new IntersectionObserver(([e]) => tl.paused(!e.isIntersecting))
      io.observe(el)
      return () => {
        io.disconnect()
        tl.kill()
        gsap.set([l, r], { clearProps: 'clipPath' })
      }
    })
    return () => mm.revert()
  }, [])

  return (
    <div ref={stage} className="relative">
      <div aria-hidden="true" data-scan className="absolute inset-x-0 top-0 z-10 h-px bg-gold motion-reduce:hidden" />
      <div className="grid gap-6 sm:grid-cols-2">
        <Column title="Normal wallet" count={leftCount}>
          {left.map((row, i) => (
            <Cell key={i} row={row} i={i} tone="text-gold-ink" attr="data-l" />
          ))}
        </Column>
        <Column title="Amnesia desk" count={rightCount}>
          {right.map((row, i) => (
            <Cell key={i} row={row} i={i} tone="" attr="data-r" />
          ))}
        </Column>
      </div>
      <p className="mt-4 font-mono text-[11px] text-muted">{note}</p>
    </div>
  )
}

function SeizureCard() {
  const history = useQuery(historyQuery)
  const base = history.data ? newest(history.data.seizures, 'baseline') : undefined
  const amn = history.data ? newest(history.data.seizures, 'amnesia') : undefined
  const baseReport = useQuery({ ...reportQuery(base?.id ?? ''), enabled: Boolean(base) })
  const amnReport = useQuery({ ...reportQuery(amn?.id ?? ''), enabled: Boolean(amn) })

  let body: ReactNode
  if (history.isError) body = <Status>The lab is unreachable right now.</Status>
  else if (!history.data) body = <Status>Loading the last seizure…</Status>
  else if (!base || !amn) body = <Status>No seizure of both kinds yet. Run one in the lab and it shows up here.</Status>
  else if (baseReport.data && amnReport.data) {
    const b = baseReport.data
    const a = amnReport.data
    const left: Array<DemoRow> = b.linked.slice(0, ROWS).map((x) => ({ label: x.client_id, value: formatTaz(x.value_zats) }))
    const live: Array<DemoRow> = a.linked.slice(0, ROWS).map((x) => ({ label: x.client_id, value: formatTaz(x.value_zats) }))
    const right = [...live, ...Array.from({ length: Math.max(0, left.length - live.length) }, (): DemoRow => ({}))]
    body = (
      <Demo
        key={base.id + amn.id}
        left={left}
        right={right}
        leftCount={`${b.linked.length} named`}
        rightCount={`${a.linked.length} live, ${a.unlinked} unlinked`}
        note="Names are from the latest reports. Bars are records the seizure could not link."
      />
    )
  } else if (baseReport.isError || amnReport.isError) {
    const n = Math.min(base.linked, ROWS)
    body = (
      <Demo
        key={`counts-${base.id}-${amn.id}`}
        left={Array.from({ length: n }, (): DemoRow => ({}))}
        right={Array.from({ length: n }, (): DemoRow => ({}))}
        leftCount={`${base.linked} named`}
        rightCount={`${amn.linked} live, 0 after shred`}
        note="The reports have expired, so this shows counts from /demo/history. Bars stand for records, not names."
      />
    )
  } else body = <Status>Loading the reports…</Status>

  return (
    <article data-rail="Seize" className="spot flex flex-col gap-6 rounded-3xl border border-line p-6 md:p-8">
      <CardHead title="Last seizure" note={amn ? formatUtc(amn.created) : '/demo/history'} />
      <div className="flex-1">{body}</div>
      <Link to="/seize" className="pill self-start">
        Seize it yourself
        <span aria-hidden="true" className="arrow">
          →
        </span>
      </Link>
    </article>
  )
}

export default function LiveCards() {
  return (
    <section className="cell grid gap-4 px-4 py-16 md:px-8 md:py-24 lg:grid-cols-2">
      <ShredLogCard />
      <SeizureCard />
    </section>
  )
}
