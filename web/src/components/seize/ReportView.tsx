import { useQuery } from '@tanstack/react-query'
import type { SeizureSummary } from '@/lib/api'
import type { YourDonation } from '@/components/seize/useYourDonation'
import LetterDrop from '@/components/LetterDrop'
import { config } from '@/config'
import { reportQuery, summaryQuery } from '@/lib/api'
import { shredWindow } from '@/lib/forget'
import { useShredReceipt } from '@/lib/receipt'
import { formatBytes, formatDuration, formatTaz, formatUtc } from '@/utils/format'
import { cnm } from '@/utils/style'

function Fact({ label, value }: { label: string; value: string | number }) {
  return (
    <div>
      <dt className="font-mono text-[11px] tracking-[0.06em] uppercase text-muted">{label}</dt>
      <dd className="mt-1.5 font-mono text-sm">
        <LetterDrop text={String(value)} />
      </dd>
    </div>
  )
}

function YouNote({ you, seizure, found }: { you: YourDonation; seizure: SeizureSummary; found: boolean }) {
  const summary = useQuery(summaryQuery)
  const shredRow = useShredReceipt(you.receipt)
  const amnesia = seizure.target === 'amnesia'
  const w = you.validUntil && summary.data ? shredWindow(you.validUntil, summary.data) : null
  const text =
    seizure.created < you.issuedAt
      ? `This seizure ran before your donation. Hold to seize again and look for ${you.client}.`
      : found
        ? amnesia
          ? `${you.client} is named, because its epoch is still live.${w ? ` Its keys are destroyed by ${formatUtc(w.to)}; seize after that and it is gone.` : ' Once the desk shreds it, the same seizure finds nothing.'}`
          : `${you.client} is named. A normal wallet keeps it for as long as the wallet exists.`
        : amnesia && shredRow && seizure.created >= shredRow.closed_hour
          ? `${you.client} is gone. Its shred receipt is in the public log (shredded ${formatUtc(shredRow.closed_hour)}), and this seizure of the whole desk finds nothing that names it.`
          : amnesia && w && seizure.created >= w.from
          ? `${you.client} is gone. Its epoch has been shredded, so this seizure of the whole desk finds nothing that names it.`
          : `${you.client} is not in this image. If it was not mined yet when the seizure ran, seize again.`
  return (
    <p className="rounded-2xl border border-gold/40 px-4 py-3 text-sm">
      <span className="font-mono text-[11px] tracking-[0.06em] text-gold-ink uppercase">your donation </span>
      <span className="text-muted">{text}</span>
    </p>
  )
}

export default function ReportView({ seizure, you }: { seizure: SeizureSummary; you?: YourDonation | null }) {
  const report = useQuery(reportQuery(seizure.id))
  const summary = useQuery(summaryQuery)
  const r = report.data
  const isAmnesia = seizure.target === 'amnesia'
  const shredAfter = summary.data
    ? formatDuration(summary.data.burial_blocks * config.blockSeconds)
    : 'a fixed delay'

  if (report.isError) {
    return <p className="font-mono text-sm text-muted">This report is no longer available. Seizures expire after the desk shreds the epochs they hold.</p>
  }
  if (!r) return <p className="font-mono text-sm text-muted">Loading report…</p>

  const named = r.linked.length
  const found = Boolean(you && r.linked.some((l) => l.client_id === you.client))

  return (
    <div className="flex flex-col gap-8">
      {you ? <YouNote you={you} seizure={seizure} found={found} /> : null}
      <div>
        <p className="display text-8xl text-gold-ink"><LetterDrop text={String(named)} /></p>
        <p className="mt-2 font-mono text-xs tracking-[0.06em] uppercase">{named === 1 ? 'donor named' : 'donors named'}</p>
      </div>

      {named > 0 ? (
        <ul className="divide-y divide-line border-y border-line font-mono text-sm">
          {r.linked.map((l, i) => (
            <li
              key={`${l.client_id}-${l.height ?? i}-${l.value_zats}`}
              className={cnm(
                'flex items-baseline justify-between gap-4 py-2 text-gold-ink',
                l.client_id === you?.client && '-mx-3 rounded-lg bg-gold/10 px-3',
              )}
            >
              <span className="flex items-baseline gap-2">
                <LetterDrop text={l.client_id} />
                {l.client_id === you?.client ? <span className="chip border-gold/50">you</span> : null}
              </span>
              <span className="text-right">
                <LetterDrop text={formatTaz(l.value_zats)} />
                <span className="text-muted"> · {l.height ?? 'unmined'}</span>
              </span>
            </li>
          ))}
        </ul>
      ) : null}

      {isAmnesia ? (
        <p className="text-sm text-muted">
          These epochs are still live. Their keys exist right now, so a seizure can read them. Each epoch
          is shredded about {shredAfter} after its last transaction, and then its donors cannot be named.
          {seizure.epochs_in_image === 0 ? ' This image held no live epochs.' : ''}
        </p>
      ) : (
        <p className="text-sm text-muted">
          Every receipt sits in the wallet database, tied to the address a donor used. Nothing was ever
          deleted, so the seizure reads it all.
        </p>
      )}

      <dl className="grid grid-cols-2 gap-x-6 gap-y-5">
        <Fact label="withdrawals on record" value={r.records_total} />
        <Fact label="tied to a donor" value={named} />
        <Fact label="left unlinked" value={r.unlinked} />
        <Fact label="keys found" value={seizure.keys_found} />
        <Fact label="live epochs in image" value={seizure.epochs_in_image} />
        <Fact label="image size" value={formatBytes(seizure.bytes)} />
      </dl>

      {r.amount_attack ? (
        <div className="rounded-2xl border border-line p-4">
          <p className="font-mono text-[11px] tracking-[0.06em] uppercase text-muted">amount attack on shredded epochs</p>
          <p className="mt-2 font-mono text-sm">
            {r.amount_attack.sweeps} sweeps, {r.amount_attack.candidates} candidate withdrawals,{' '}
            {r.amount_attack.pinned} pinned, {r.amount_attack.ambiguous} ambiguous
          </p>
          <p className="mt-2 text-sm text-muted">
            With the treasury seed an attacker can see every sweep and try to match donations to it by
            amount. Pinned means a withdrawal fits only one sweep. Ambiguous means it fits several.
          </p>
        </div>
      ) : null}

      <p className="flex flex-wrap gap-x-6 gap-y-3 font-mono text-xs">
        <a href={`/demo/seizures/${encodeURIComponent(seizure.id)}/bundle.tar`} download className="underline underline-offset-4">
          Download the seized bundle
        </a>
        <a href={`/demo/records/${seizure.target}.csv`} className="underline underline-offset-4">
          Exchange records (csv)
        </a>
      </p>
    </div>
  )
}
