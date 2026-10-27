import { useQuery } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import { useEffect, useRef, useState } from 'react'
import type { ShredRow } from '@/lib/api'
import type { ShredderHandle } from '@/components/reactbits/Shredder'
import PageHeader from '@/components/PageHeader'
import Eyebrow from '@/components/Eyebrow'
import Shredder from '@/components/reactbits/Shredder'
import { auditWeeksQuery, shredLogQuery, storedDonation, summaryQuery } from '@/lib/api'
import { useShredReceipt } from '@/lib/receipt'
import { cnm } from '@/utils/style'
import { formatTaz, formatUtc, formatUtcDay, shortId } from '@/utils/format'

const PAGE_SIZE = 10

export const Route = createFileRoute('/audit')({
  validateSearch: (search: Record<string, unknown>): { page?: number } => {
    const page = Math.floor(Number(search.page))
    return page > 1 ? { page } : {}
  },
  component: AuditPage,
})

function Badge({ children, title }: { children: React.ReactNode; title?: string }) {
  return (
    <span title={title} className="chip">
      {children}
    </span>
  )
}

const ProfileBadge = ({ profile }: { profile: string }) =>
  profile === 'unknown' ? (
    <Badge title="Written before the desk recorded profiles">unknown</Badge>
  ) : (
    <Badge>{profile}</Badge>
  )

function ShredStage() {
  const { data } = useQuery(shredLogQuery(3, 0))
  const ref = useRef<ShredderHandle>(null)
  const stage = useRef<HTMLDivElement>(null)
  const [run, setRun] = useState(0)
  const rows = data?.rows
  const newestId = rows?.[0]?.epoch_id

  useEffect(() => {
    const el = stage.current
    if (!el || !newestId) return
    let timer: number | undefined
    const io = new IntersectionObserver(([entry]) => {
      if (!entry.isIntersecting) return
      io.disconnect()
      timer = window.setTimeout(() => ref.current?.shred(newestId), 900)
    })
    io.observe(el)
    return () => {
      io.disconnect()
      window.clearTimeout(timer)
    }
  }, [newestId, run])

  return (
    <div ref={stage} className="flex flex-col gap-4">
      {rows ? (
        <Shredder<ShredRow & { id: string }>
          key={run}
          shredRef={ref}
          width={480}
          height={400}
          slitColor="var(--line)"
          color="var(--fg)"
          items={[...rows].reverse().map((r) => ({ ...r, id: r.epoch_id }))}
          renderItem={(r) => (
            <div className="flex h-[72px] flex-col justify-center gap-1 rounded-2xl border border-line bg-surface px-4 font-mono text-xs">
              <span>{shortId(r.epoch_id)}</span>
              <span className="text-muted">
                {r.closed_kind} · {formatUtc(r.closed_hour)}
              </span>
            </div>
          )}
        />
      ) : (
        // Holds the shredder's 400 px stage so the log below does not jump (CLS).
        <p className="flex h-[400px] w-[min(480px,80vw)] items-center justify-center font-mono text-sm text-muted">
          Loading the latest rows…
        </p>
      )}
      <p className="max-w-md text-xs text-muted">
        The rows are real entries from the log. The shredding is a picture: the desk deleted these keys
        when each epoch was shredded.{' '}
        <button type="button" onClick={() => setRun((n) => n + 1)} className="underline underline-offset-4">
          Replay
        </button>
      </p>
    </div>
  )
}

function YourReceipt({ receipt }: { receipt: string }) {
  const row = useShredReceipt(receipt)
  if (row === undefined) return null
  return (
    <p className="mb-6 rounded-2xl border border-gold/40 px-4 py-3 text-sm">
      <span className="font-mono text-[11px] tracking-[0.06em] text-gold-ink uppercase">your receipt </span>
      <span className="font-mono">{shortId(receipt)}</span>
      <span className="text-muted">
        {row
          ? ` is in the log: shredded ${formatUtc(row.closed_hour)}. The keys for your address are gone.`
          : ' is not in the log yet. Its epoch is still live or burying; check back after the time on your donation.'}
      </span>
    </p>
  )
}

function Log({ page, receipt }: { page: number; receipt?: string }) {
  const offset = (page - 1) * PAGE_SIZE
  const { data, isError } = useQuery(shredLogQuery(PAGE_SIZE, offset))
  const last = data ? Math.max(1, Math.ceil(data.total / PAGE_SIZE)) : page

  if (isError) return <p className="font-mono text-sm text-muted">The shred log is unavailable right now.</p>
  if (!data) return <p className="font-mono text-sm text-muted">Loading…</p>

  return (
    <>
      <div className="overflow-x-auto">
        <table className="w-full min-w-176 border-collapse text-left font-mono text-xs">
          <thead className="text-muted">
            <tr className="border-b border-line">
              {['epoch', 'opened', 'closed', 'outcome', 'addresses issued', 'profile'].map((h) => (
                <th key={h} scope="col" className="py-3 pr-4 font-normal tracking-[0.06em] uppercase">
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {data.rows.map((r) => (
              <tr key={r.epoch_id} className={cnm('border-b border-line', r.epoch_id === receipt && 'bg-gold/10 text-gold-ink')}>
                <td className="py-3 pr-4" title={r.epoch_id}>
                  {shortId(r.epoch_id)}
                </td>
                <td className="py-3 pr-4 whitespace-nowrap">{formatUtc(r.opened_hour)}</td>
                <td className="py-3 pr-4 whitespace-nowrap">{formatUtc(r.closed_hour)}</td>
                <td className="py-3 pr-4">{r.closed_kind}</td>
                <td className="py-3 pr-4">{r.issued_bucket}</td>
                <td className="py-3">
                  <ProfileBadge profile={r.profile} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <nav aria-label="Pages" className="mt-4 flex items-center gap-6 font-mono text-xs">
        {page > 1 ? (
          <Link to="/audit" search={{ page: page > 2 ? page - 1 : undefined }} className="underline underline-offset-4">
            Newer
          </Link>
        ) : null}
        <span className="text-muted">
          page {page} of {last} · {data.total} epochs
        </span>
        {page < last ? (
          <Link to="/audit" search={{ page: page + 1 }} className="underline underline-offset-4">
            Older
          </Link>
        ) : null}
      </nav>
      <p className="mt-6 max-w-2xl text-xs text-muted">
        Times are rounded to the hour and address counts are bucketed, on purpose. Rows marked unknown were
        written before the desk recorded profiles, so we cannot say which profile produced them.
      </p>
    </>
  )
}

function Weeks() {
  const { data, isError } = useQuery(auditWeeksQuery)
  if (isError) return <p className="font-mono text-sm text-muted">Weekly totals are unavailable right now.</p>
  if (!data) return <p className="font-mono text-sm text-muted">Loading…</p>
  if (data.weeks.length === 0) return <p className="font-mono text-sm text-muted">No completed weeks yet.</p>

  return (
    <div className="overflow-x-auto">
      <table className="w-full min-w-96 border-collapse text-left font-mono text-xs">
        <thead className="text-muted">
          <tr className="border-b border-line">
            {['week of', 'epochs forgotten', 'swept (2 significant figures)'].map((h) => (
              <th key={h} scope="col" className="py-3 pr-4 font-normal tracking-[0.06em] uppercase">
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {data.weeks.map((w) => (
            <tr key={w.week_start} className="border-b border-line">
              <td className="py-3 pr-4">{formatUtcDay(w.week_start)}</td>
              <td className="py-3 pr-4">{w.epochs_forgotten}</td>
              <td className="py-3">{formatTaz(w.total_2sf_zats)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

function AuditPage() {
  const { page = 1 } = Route.useSearch()
  const summary = useQuery(summaryQuery)
  // Read after mount: this device's receipt, never sent anywhere.
  const [receipt, setReceipt] = useState<string>()
  useEffect(() => setReceipt(storedDonation()?.receipt), [])

  return (
    <>
      <PageHeader
        eyebrow={summary.data ? `shred log · ${summary.data.epochs_shredded} shredded, ${summary.data.epochs_lost} lost` : 'shred log'}
        title={<>Every epoch the desk has <span className="text-muted">shredded.</span></>}
      >
        A lost epoch vanished from the host before the desk could record its shred. The log is public and read
        straight from the desk.
      </PageHeader>

      <section className="cell flex justify-center px-5 py-12 md:py-16">
        <div className="card p-6 md:p-8">
          <ShredStage />
        </div>
      </section>

      <section className="cell px-5 py-16 md:px-10 md:py-20">
        <Eyebrow>all epochs, newest first</Eyebrow>
        <div className="mt-6">
          {receipt ? <YourReceipt receipt={receipt} /> : null}
          <Log page={page} receipt={receipt} />
        </div>
      </section>

      <section className="cell px-5 py-16 md:px-10 md:py-20">
        <Eyebrow>weekly totals</Eyebrow>
        <div className="mt-6">
          <Weeks />
        </div>
      </section>
    </>
  )
}
