import { useMutation, useQuery } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import { useEffect, useState } from 'react'
import type { RunEvent } from '@/lib/api'
import type { StatusMarkStatus } from '@/components/reactbits/StatusMark'
import PageHeader from '@/components/PageHeader'
import DeskPulse from '@/components/DeskPulse'
import ForgetAddress from '@/components/ForgetAddress'
import OnionDonate from '@/components/OnionDonate'
import StatusMark from '@/components/reactbits/StatusMark'
import { config } from '@/config'
import { ApiError, errorCopy, healthQuery, lastDonateRun, rememberDonateRun, rememberDonation, runQuery, startDonation, summaryQuery } from '@/lib/api'
import { shredWindow } from '@/lib/forget'
import { formatTaz, formatUtc, shortId } from '@/utils/format'

export const Route = createFileRoute('/donate')({
  component: DonatePage,
})

const TXID = /^[0-9a-f]{64}$/

// Explorers only index mined transactions, so the link appears once the block lands.
function TxLink({ label, txid, mined }: { label: string; txid: string; mined: boolean }) {
  const id = shortId(txid, 10, 6)
  return (
    <p className="font-mono text-sm">
      <span className="text-muted">{label} </span>
      {mined && TXID.test(txid) ? (
        <a
          href={`${config.explorerTxUrl}${txid}`}
          target="_blank"
          rel="noopener noreferrer"
          className="underline underline-offset-4"
        >
          {id}
        </a>
      ) : (
        <span title={txid}>{id}</span>
      )}
    </p>
  )
}

const clock = (seconds: number) => {
  const s = Math.max(0, Math.round(seconds))
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
}

// Ticks once a second while `on`, for the live elapsed counter.
function useNow(on: boolean) {
  const [now, setNow] = useState(() => Date.now() / 1000)
  useEffect(() => {
    if (!on) return
    const id = window.setInterval(() => setNow(Date.now() / 1000), 1000)
    return () => window.clearInterval(id)
  }, [on])
  return now
}

function Step({
  status,
  label,
  children,
}: {
  status: StatusMarkStatus
  label: string
  children?: React.ReactNode
}) {
  return (
    <li className="flex flex-col gap-3 border-b border-line py-5 last:border-b-0">
      <StatusMark size={20} fontSize={16} label={label} status={status} />
      {children ? <div className="pl-[31px]">{children}</div> : null}
    </li>
  )
}

function DonatePage() {
  const [runId, setRunId] = useState(lastDonateRun)
  const [message, setMessage] = useState<string | null>(null)

  const start = useMutation({
    mutationFn: startDonation,
    onMutate: () => setMessage(null),
    onSuccess: ({ run_id }) => {
      rememberDonateRun(run_id)
      setRunId(run_id)
    },
    onError: (e) => setMessage(errorCopy(e)),
  })

  const run = useQuery({ ...runQuery(runId ?? ''), enabled: runId !== null })

  useEffect(() => {
    if (run.error instanceof ApiError && run.error.status === 404) {
      rememberDonateRun(null)
      setRunId(null)
    }
  }, [run.error])

  const events = run.data?.events ?? []
  const find = <TKind extends RunEvent['kind']>(kind: TKind) =>
    events.find((e): e is Extract<RunEvent, { kind: TKind }> => e.kind === kind)
  const issued = find('issued')
  const floatWait = find('float_wait')
  const broadcast = find('broadcast')
  const mined = find('mined')
  const failed = Boolean(run.data?.error)
  const live = (done: boolean, prev: boolean): StatusMarkStatus =>
    done ? 'done' : prev ? (failed ? 'failed' : 'running') : 'pending'
  const started = runId !== null
  const waiting = start.isPending || (started && !run.data)
  const now = useNow(Boolean(broadcast) && !mined && !failed)
  const health = useQuery(healthQuery)
  const deskBehind = health.data ? health.data.intake !== 'open' : false
  const summary = useQuery(summaryQuery)
  const validUntil = issued?.detail.valid_until
  const forget = validUntil && summary.data ? shredWindow(validUntil, summary.data) : null

  const issuedClient = issued?.detail.client
  const issuedAt = issued?.t
  const receipt = issued?.detail.receipt ?? undefined
  useEffect(() => {
    if (issuedClient && issuedAt) rememberDonation({ client: issuedClient, issuedAt, validUntil, receipt })
  }, [issuedClient, issuedAt, validUntil, receipt])

  return (
    <>
      <PageHeader eyebrow="testnet donation" title={<>Give once. <span className="text-muted">Be forgotten.</span></>} />
      <DeskPulse />

      <div className="cell grid grid-cols-1 gap-10 px-5 py-12 md:grid-cols-[1fr_1.2fr] md:gap-16 md:px-10 md:py-20">
        <div className="flex min-w-0 flex-col gap-6">
          <div>
            <p className="font-mono text-[11px] tracking-[0.06em] text-muted uppercase">no wallet needed</p>
            <h2 className="mt-2 text-xl font-medium tracking-[-0.02em]">Let the lab donate for you</h2>
            <p className="mt-2 text-muted">
              The lab runs a simulated exchange with real testnet funds. One click pays a fresh Amnesia address
              and an address in a normal wallet, same amount, so you can seize both and compare.
            </p>
          </div>
          <div>
            <button
              type="button"
              disabled={deskBehind || start.isPending || (started && !run.data?.done && !failed)}
              onClick={() => start.mutate()}
              className="pill pill-solid"
            >
              {started && run.data?.done ? 'Make another donation' : 'Make a judge donation'}
            </button>
            <p role="alert" className="mt-3 text-sm text-muted empty:hidden">
              {deskBehind && !message
                ? 'The desk is catching up with the chain, usually after the host was busy or asleep. It stops handing out addresses until it is current, so donations reopen by themselves in a few minutes. Seizures still work.'
                : message}
            </p>
          </div>
        </div>

        <div className="card min-w-0 self-start overflow-hidden md:col-start-2 md:row-span-2 md:row-start-1">
          <div className="flex items-center gap-1.5 border-b border-line px-5 py-3">
            <span className="size-2 rounded-full bg-line-strong" />
            <span className="size-2 rounded-full bg-line-strong" />
            <span className="size-2 rounded-full bg-line-strong" />
            <span className="ml-3 font-mono text-[11px] tracking-[0.06em] text-muted uppercase">donation run</span>
          </div>
          {mined && broadcast ? (
            <div className="grid grid-cols-2 gap-px border-b border-line bg-line">
              <div className="bg-bg px-5 py-5 md:px-8">
                <p className="font-mono text-[11px] tracking-[0.06em] text-muted uppercase">mined in</p>
                <p className="display mt-2 text-4xl tabular-nums">{clock(mined.t - broadcast.t)}</p>
              </div>
              <div className="bg-bg px-5 py-5 md:px-8">
                <p className="font-mono text-[11px] tracking-[0.06em] text-muted uppercase">keys gone by</p>
                <p className="display mt-2 text-4xl tabular-nums">{forget ? formatUtc(forget.to).replace(' UTC', '') : '…'}</p>
                <p className="mt-1 font-mono text-[11px] text-muted">UTC, at the latest</p>
              </div>
            </div>
          ) : null}
          <div className="px-5 pb-2 md:px-8">
          {started || start.isPending ? (
            <ol aria-live="polite">
              <Step status={live(Boolean(issued), true)} label="Address issued">
                {issued ? (
                  <>
                    <ForgetAddress address={issued.detail.amnesia_address} issuedAt={issued.t} />
                    <p className="mt-3 text-sm text-muted">
                      {formatTaz(issued.detail.value_zats)} from client{' '}
                      <span className="font-mono">{issued.detail.client}</span>. Look for that id in the
                      seizure reports.
                    </p>
                  </>
                ) : floatWait && !failed ? (
                  <p className="text-sm text-muted">
                    The lab exchange&apos;s coins from earlier donations are still confirming. Your donation starts
                    as soon as they are spendable, usually within a few minutes.
                  </p>
                ) : waiting || (started && !failed) ? (
                  <p className="text-sm text-muted">Asking the desk for a one-time address.</p>
                ) : null}
              </Step>
              <Step status={live(Boolean(broadcast), Boolean(issued))} label="Broadcast">
                {broadcast ? (
                  <>
                    <TxLink label="desk" txid={broadcast.detail.amnesia_txid} mined={Boolean(mined)} />
                    <TxLink label="baseline" txid={broadcast.detail.baseline_txid} mined={Boolean(mined)} />
                    <p className="mt-3 text-sm text-muted">
                      The lab&apos;s exchange sent both payments at once: same amount, same moment, one to the
                      desk and one to a normal wallet.
                    </p>
                  </>
                ) : null}
              </Step>
              <Step status={live(Boolean(mined), Boolean(broadcast))} label="Mined">
                {mined && broadcast ? (
                  <>
                    <p className="font-mono text-sm">
                      <span className="text-muted">heights </span>
                      {mined.detail.amnesia_height} and {mined.detail.baseline_height}
                      <span className="text-muted"> · {clock(mined.t - broadcast.t)} after broadcast</span>
                    </p>
                    <p className="mt-3 text-sm text-muted">
                      The txids above now open in a third-party explorer (it shows ads). Amounts read 0 there:
                      both payments are shielded.
                    </p>
                  </>
                ) : broadcast && !failed ? (
                  <p className="text-sm text-muted">
                    <span className="font-mono text-fg tabular-nums">{clock(now - broadcast.t)}</span> waiting for a
                    block. Testnet aims for one every {config.blockSeconds} s, and the lab checks every 15 s, so
                    this usually takes a few minutes.
                  </p>
                ) : null}
              </Step>
              <Step status={mined ? 'running' : 'pending'} label="Forgotten">
                {mined && issued && forget && validUntil ? (
                  <p className="text-sm text-muted">
                    The address {Date.now() / 1000 < validUntil ? 'takes' : 'took'} payments until {formatUtc(validUntil)}. Then the desk sweeps the epoch to its
                    treasury and waits {summary.data?.burial_blocks} blocks. The keys are destroyed between{' '}
                    <span className="text-fg">{formatUtc(forget.from)}</span> and{' '}
                    <span className="text-fg">{formatUtc(forget.to)}</span>. Seize now and{' '}
                    <span className="font-mono text-gold-ink">{issued.detail.client}</span> is named. Seize after
                    that and it is gone. This device remembers the id, so /seize can tell you.
                  </p>
                ) : null}
                {mined && receipt ? (
                  <p className="mt-3 font-mono text-sm">
                    <span className="text-muted">shred receipt </span>
                    <span title={receipt}>{shortId(receipt, 8, 4)}</span>
                    <span className="block font-sans text-xs text-muted">
                      Your epoch&apos;s id. It shows up in the public shred log only once the keys are destroyed, and it
                      names the epoch, never you.
                    </span>
                  </p>
                ) : null}
              </Step>
              {failed ? (
                <li className="border-b border-line py-5 text-sm text-muted">
                  The donation did not complete on the lab side. Try again in a moment.
                </li>
              ) : null}
              {mined ? (
                <li className="border-b border-line py-5">
                  <Link to="/seize" className="pill pill-solid">
                    Now seize both wallets
                  </Link>
                </li>
              ) : null}
            </ol>
          ) : (
            <ol aria-label="Donation steps, not started">
              <Step status="pending" label="Address issued" />
              <Step status="pending" label="Broadcast" />
              <Step status="pending" label="Mined" />
              <Step status="pending" label="Forgotten" />
            </ol>
          )}
          </div>
        </div>
        <div className="min-w-0 md:col-start-1">
          <OnionDonate />
        </div>
      </div>
    </>
  )
}
