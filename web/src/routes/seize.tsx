import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { useEffect, useState } from 'react'
import type { SeizeTarget, SeizureSummary } from '@/lib/api'
import DeskPulse from '@/components/DeskPulse'
import PageHeader from '@/components/PageHeader'
import Eyebrow from '@/components/Eyebrow'
import SeizePanel from '@/components/seize/SeizePanel'
import { useYourDonation } from '@/components/seize/useYourDonation'
import { errorCopy, historyQuery, runQuery, seizuresQuery, startSeizure } from '@/lib/api'
import { formatBytes, formatUtc, shortId } from '@/utils/format'

export const Route = createFileRoute('/seize')({
  component: SeizePage,
})

const newest = (list: Array<SeizureSummary> | undefined, target: SeizeTarget) =>
  list?.filter((s) => s.target === target).sort((a, b) => b.created - a.created)[0]

function SeizePage() {
  const queryClient = useQueryClient()
  const you = useYourDonation()
  const seizures = useQuery(seizuresQuery)
  const [active, setActive] = useState<{ target: SeizeTarget; runId: string } | null>(null)
  const [error, setError] = useState<{ target: SeizeTarget; message: string } | null>(null)

  const start = useMutation({
    mutationFn: startSeizure,
    onMutate: () => setError(null),
    onSuccess: ({ run_id }, target) => setActive({ target, runId: run_id }),
    onError: (e, target) => setError({ target, message: errorCopy(e) }),
  })

  const run = useQuery({ ...runQuery(active?.runId ?? ''), enabled: active !== null })
  const finished = run.data?.done === true && run.data.events.some((e) => e.kind === 'report')

  useEffect(() => {
    if (!finished) return
    void Promise.all([
      queryClient.invalidateQueries(seizuresQuery),
      queryClient.invalidateQueries(historyQuery),
    ]).then(() => setActive(null))
  }, [finished, queryClient])

  const busy = start.isPending || active !== null
  const panel = (target: SeizeTarget) => ({
    you,
    target,
    latest: newest(seizures.data?.seizures, target),
    run: active?.target === target ? run.data : undefined,
    starting: (start.isPending && start.variables === target) || (active?.target === target && !run.data),
    busy,
    error: error?.target === target ? error.message : undefined,
    onSeize: () => start.mutate(target),
  })

  return (
    <>
      <PageHeader eyebrow="seizure lab" title={<>Take both wallets. <span className="text-muted">Count the names.</span></>}>
        The lab copies every byte a host holds, scans it for keys and receipts, and matches them against what
        a court order to the exchange would return. Same script, two targets.
      </PageHeader>
      <DeskPulse />

      <section className="cell grid gap-4 p-4 md:grid-cols-2 md:p-8">
        <SeizePanel
          {...panel('baseline')}
          title="Normal wallet"
          blurb="One wallet, one history. Every donation ever received is still in it."
        />
        <SeizePanel
          {...panel('amnesia')}
          title="Amnesia desk"
          blurb="Epoch keys are deleted on a schedule. Only epochs that are still live can be read."
        />
      </section>

      <History />
    </>
  )
}

const HISTORY_ROWS = 10

function History() {
  const history = useQuery(historyQuery)
  const current = useQuery(seizuresQuery)
  const downloadable = new Set(current.data?.seizures.map((s) => s.id))
  const [all, setAll] = useState(false)
  const rows = history.data?.seizures
  const shown = all ? rows : rows?.slice(0, HISTORY_ROWS)

  return (
    <section className="cell px-5 py-16 md:px-10 md:py-20">
      <Eyebrow>every seizure ever run</Eyebrow>
      {history.isError ? (
        <p className="mt-6 font-mono text-sm text-muted">History is unavailable right now.</p>
      ) : !rows ? (
        <p className="mt-6 font-mono text-sm text-muted">Loading…</p>
      ) : rows.length === 0 ? (
        <p className="mt-6 font-mono text-sm text-muted">No seizures yet.</p>
      ) : (
        <div className="mt-6 overflow-x-auto">
          <table className="w-full min-w-176 border-collapse text-left font-mono text-xs">
            <thead className="text-muted">
              <tr className="border-b border-line">
                {['when', 'target', 'named', 'of', 'keys', 'live epochs', 'size', 'pinned', 'ambiguous', ''].map((h) => (
                  <th key={h} scope="col" className="py-3 pr-4 font-normal uppercase tracking-[0.06em]">
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {shown?.map((s) => (
                <tr key={s.id} className="border-b border-line">
                  <td className="py-3 pr-4 whitespace-nowrap">{formatUtc(s.created)}</td>
                  <td className="py-3 pr-4">{s.target}</td>
                  <td className="py-3 pr-4 text-gold-ink">{s.linked}</td>
                  <td className="py-3 pr-4">{s.records_total}</td>
                  <td className="py-3 pr-4">{s.keys_found}</td>
                  <td className="py-3 pr-4">{s.epochs_in_image}</td>
                  <td className="py-3 pr-4 whitespace-nowrap">{formatBytes(s.bytes)}</td>
                  <td className="py-3 pr-4">{s.target === 'amnesia' ? s.amount_pinned : '-'}</td>
                  <td className="py-3 pr-4">{s.target === 'amnesia' ? s.amount_ambiguous : '-'}</td>
                  <td className="py-3 whitespace-nowrap">
                    {downloadable.has(s.id) ? (
                      <a href={`/demo/seizures/${encodeURIComponent(s.id)}/bundle.tar`} download className="chip hover:bg-line">
                        bundle
                      </a>
                    ) : (
                      <span className="text-muted">{shortId(s.id, 6, 0)}</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {rows && rows.length > HISTORY_ROWS ? (
        <button type="button" onClick={() => setAll((v) => !v)} className="mt-4 font-mono text-xs underline underline-offset-4">
          {all ? 'Show the latest ten' : `Show all ${rows.length}`}
        </button>
      ) : null}
      <p className="mt-4 max-w-2xl text-xs text-muted">
        Counts only. The lab forgets a seized bundle once the desk has shredded every epoch it holds.
      </p>
    </section>
  )
}
