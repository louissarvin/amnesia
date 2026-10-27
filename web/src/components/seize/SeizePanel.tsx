import type { Run, SeizeTarget, SeizureSummary } from '@/lib/api'
import type { StatusMarkStatus } from '@/components/reactbits/StatusMark'
import HoldButton from '@/components/reactbits/HoldButton'
import StatusMark from '@/components/reactbits/StatusMark'
import type { YourDonation } from '@/components/seize/useYourDonation'
import ReportView from '@/components/seize/ReportView'
import Eyebrow from '@/components/Eyebrow'

const STEPS = [
  { kind: 'bundle_ready', label: 'Copy everything the host holds' },
  { kind: 'scanning', label: 'Scan for keys and receipts' },
  { kind: 'report', label: 'Link receipts to donors' },
] as const

function stepStatus(run: Run, index: number): StatusMarkStatus {
  const seen = (i: number) => run.events.some((e) => e.kind === STEPS[i].kind)
  if (seen(index)) return 'done'
  const firstOpen = STEPS.findIndex((_, i) => !seen(i))
  if (index !== firstOpen) return 'pending'
  return run.error ? 'failed' : 'running'
}

interface SeizePanelProps {
  you?: YourDonation | null
  target: SeizeTarget
  title: string
  blurb: string
  latest?: SeizureSummary
  run?: Run
  starting: boolean
  busy: boolean
  error?: string
  onSeize: () => void
}

export default function SeizePanel({ you, target, title, blurb, latest, run, starting, busy, error, onSeize }: SeizePanelProps) {
  const running = starting || Boolean(run)

  return (
    <section aria-labelledby={`${target}-title`} className="card relative flex flex-col gap-8 overflow-hidden p-6 md:p-10">
      {running ? <div aria-hidden="true" className="beam" /> : null}
      <div>
        <Eyebrow>{target === 'baseline' ? 'ordinary wallet' : 'amnesia desk'}</Eyebrow>
        <h2 id={`${target}-title`} className="display mt-3 text-4xl md:text-5xl">
          {title}
        </h2>
        <p className="mt-3 max-w-sm text-sm text-muted">{blurb}</p>
      </div>

      <div>
        <HoldButton
          size="md"
          radius={999}
          holdTime={1200}
          wave={false}
          glow={false}
          disabled={busy}
          backgroundColor="var(--surface)"
          fillColor="var(--fg)"
          textColor="var(--fg)"
          fillTextColor="var(--bg)"
          doneLabel="Seizing"
          className="w-full border border-line-strong font-mono tracking-[0.04em] uppercase"
          onHold={onSeize}
        >
          Hold to seize
        </HoldButton>
        <p role="alert" className="mt-3 text-sm text-muted empty:hidden">
          {error}
        </p>
      </div>

      {running ? (
        <ol aria-live="polite" className="flex flex-col gap-3">
          {STEPS.map((step, i) => (
            <li key={step.kind}>
              <StatusMark
                size={18}
                fontSize={14}
                label={step.label}
                status={run ? stepStatus(run, i) : i === 0 ? 'running' : 'pending'}
              />
            </li>
          ))}
          {run?.error ? <p className="text-sm text-muted">The seizure failed on the lab side. Try again.</p> : null}
        </ol>
      ) : latest ? (
        <ReportView seizure={latest} you={you} />
      ) : (
        <p className="font-mono text-sm text-muted">No seizure yet. Hold the button to run one.</p>
      )}
    </section>
  )
}
