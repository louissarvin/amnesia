import { useQuery } from '@tanstack/react-query'
import { config } from '@/config'
import { healthQuery, labStatusQuery, summaryQuery, treasuryQuery } from '@/lib/api'
import { formatDuration } from '@/utils/format'

const SWEPT: Record<string, string> = {
  '0': 'nothing yet',
  'lt_0.1': 'under 0.1 TAZ',
  '0.1-1': '0.1 to 1 TAZ',
  '1-10': '1 to 10 TAZ',
  '10+': 'over 10 TAZ',
}

function Cell({ label, value, note }: { label: string; value?: string; note?: string }) {
  return (
    <div className="flex min-w-0 flex-col gap-1.5 bg-bg px-5 py-5 md:px-6">
      <dt className="font-mono text-[11px] tracking-[0.06em] text-muted uppercase">{label}</dt>
      <dd className="font-mono text-lg tabular-nums">{value ?? '…'}</dd>
      {note ? <dd className="text-xs text-muted">{note}</dd> : null}
    </div>
  )
}

// Live numbers straight from the desk and the lab, refreshed every 30 s.
export default function DeskPulse() {
  const health = useQuery(healthQuery)
  const summary = useQuery(summaryQuery)
  const lab = useQuery(labStatusQuery)
  const treasury = useQuery(treasuryQuery)
  const s = summary.data

  return (
    <section aria-label="The desk right now" className="cell">
      <dl className="grid grid-cols-2 gap-px bg-line md:grid-cols-4">
        <Cell
          label="live epochs"
          value={health.data?.live_epochs_bucket}
          note={s ? `a new one every ${formatDuration(s.epoch_s)}` : undefined}
        />
        <Cell
          label="shredded"
          value={s ? `${s.epochs_shredded}` : undefined}
          note={s ? `${s.epochs_lost} lost, keys gone ${formatDuration(s.burial_blocks * config.blockSeconds)} after the last tx` : undefined}
        />
        <Cell
          label="exchange log"
          value={lab.data ? `${lab.data.donations + lab.data.decoys}` : undefined}
          note={lab.data ? `${lab.data.donations} to the desk, ${lab.data.decoys} decoys` : undefined}
        />
        <Cell
          label="swept to treasury"
          value={treasury.data ? (SWEPT[treasury.data.swept_bucket] ?? treasury.data.swept_bucket) : undefined}
          note="bucketed on purpose, since boot"
        />
      </dl>
    </section>
  )
}
