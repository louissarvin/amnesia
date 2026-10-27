import { useQuery } from '@tanstack/react-query'
import CountUp from '@/components/reactbits/CountUp'
import { summaryQuery } from '@/lib/api'

function Stat({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="spot relative flex flex-col gap-4 bg-bg p-6 md:p-8">
      <dt className="font-mono text-[11px] tracking-[0.08em] text-muted uppercase">{label}</dt>
      <dd className="display text-5xl md:text-6xl">{children}</dd>
    </div>
  )
}

export default function Stats() {
  const { data: s, isError } = useQuery(summaryQuery)
  const pending = <span className="font-mono text-sm tracking-normal text-muted">{isError ? 'unavailable' : 'loading'}</span>

  return (
    <section className="cell">
      <dl className="relative grid grid-cols-2 gap-px border-b border-line bg-line md:grid-cols-4">
        <Stat label="epochs forgotten">{s ? <CountUp to={s.epochs_shredded} /> : pending}</Stat>
        <Stat label={s ? `shredded on the ${s.profile} profile` : 'shredded on this profile'}>
          {s ? <CountUp to={s.epochs_shredded_this_profile} /> : pending}
        </Stat>
        <Stat label="live epochs">{s ? s.live_epochs_bucket : pending}</Stat>
        <Stat label="intake">{s ? s.intake : pending}</Stat>
      </dl>
      <p className="relative px-6 py-4 font-mono text-[11px] tracking-[0.04em] text-muted uppercase md:px-8">
        Live from{' '}
        <a href="/v1/summary" className="underline underline-offset-4">
          /v1/summary
        </a>
        . Live epochs are bucketed on purpose.
      </p>
    </section>
  )
}
