import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { useState } from 'react'
import PageHeader from '@/components/PageHeader'
import Eyebrow from '@/components/Eyebrow'
import { infoQuery } from '@/lib/api'
import { formatDuration } from '@/utils/format'
import { cnm } from '@/utils/style'

export const Route = createFileRoute('/verify')({
  component: VerifyPage,
})

const ENDPOINTS = [
  '/v1/health',
  '/v1/summary',
  '/v1/shred-log?limit=20',
  '/v1/info',
  '/v1/audit/weeks',
  '/v1/treasury',
  '/demo/status',
  '/demo/history',
  '/demo/seizures',
]

const SECONDS = new Set(['epoch_s', 'grace_s'])

const readable = (key: string, value: number | string) => {
  if (typeof value === 'number' && SECONDS.has(key)) return `${value} (${formatDuration(value)})`
  return String(value)
}

function HashBlock({ label, value, wide }: { label: string; value: string; wide?: boolean }) {
  const [state, setState] = useState<'idle' | 'copied' | 'failed'>('idle')
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(value)
      setState('copied')
    } catch {
      setState('failed')
    }
    window.setTimeout(() => setState('idle'), 1600)
  }
  return (
    <div className={cnm('card flex flex-col gap-4 p-5 md:p-6', wide && 'md:col-span-2')}>
      <div className="flex items-center justify-between gap-3">
        <p className="font-mono text-[11px] tracking-[0.08em] text-muted uppercase">{label}</p>
        <button type="button" onClick={copy} className="pill px-3 py-2 text-[11px]">
          {state === 'copied' ? 'Copied' : state === 'failed' ? 'Copy failed' : 'Copy'}
        </button>
      </div>
      <p className="font-mono text-sm break-all">{value}</p>
    </div>
  )
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid items-baseline gap-1.5 border-t border-line py-4 md:grid-cols-[14rem_1fr] md:gap-6">
      <dt className="font-mono text-[11px] tracking-[0.06em] text-muted uppercase">{label}</dt>
      <dd className="min-w-0 break-all font-mono text-sm">{children}</dd>
    </div>
  )
}

function VerifyPage() {
  const { data, isError } = useQuery(infoQuery)

  return (
    <>
      <PageHeader eyebrow="build provenance" title={<>Check the claims. <span className="text-muted">Do not trust them.</span></>}>
        Everything on this site comes from the endpoints below. Fetch them yourself, compare the binary hash,
        and rebuild it.
      </PageHeader>

      <section className="cell px-5 py-16 md:px-10 md:py-20">
        <Eyebrow>the running desk</Eyebrow>
        {isError ? (
          <p className="mt-6 font-mono text-sm text-muted">Build info is unavailable right now.</p>
        ) : !data ? (
          // Same shape as the loaded content, so the sections below do not jump (CLS).
          <div aria-busy="true" className="mt-6 grid gap-4 md:grid-cols-2">
            {['binary sha256', 'Cargo.lock sha256', 'git commit'].map((label, i) => (
              <div key={label} className={cnm('card flex min-h-36 flex-col gap-4 p-5 md:min-h-28 md:p-6', i === 0 && 'md:col-span-2')}>
                <p className="font-mono text-[11px] tracking-[0.08em] text-muted uppercase">{label}</p>
                <p className="font-mono text-sm text-muted">Loading…</p>
              </div>
            ))}
            <div className="min-h-[36rem] md:col-span-2 md:min-h-[24rem]" />
          </div>
        ) : (
          <>
            <div className="mt-6 grid gap-4 md:grid-cols-2">
              <HashBlock wide label="binary sha256" value={data.binary_sha256} />
              <HashBlock label="Cargo.lock sha256" value={data.build.cargo_lock_sha256} />
              <HashBlock label="git commit" value={data.git_commit} />
            </div>
            <dl className="mt-10">
              <Row label="toolchain">
                <pre className="font-mono whitespace-pre-wrap">{data.build.toolchain.trim()}</pre>
              </Row>
              <Row label="how to reproduce">
                <span className="font-sans text-sm break-normal">{data.build.reproduce}</span>
              </Row>
              <Row label="version">{data.version}</Row>
              <Row label="network">{data.network}</Row>
              <Row label="profile">{data.profile}</Row>
              <Row label="tee">{data.tee}</Row>
            </dl>
          </>
        )}
      </section>

      <section className="cell px-5 py-16 md:px-10 md:py-20">
        <Eyebrow>live parameters</Eyebrow>
        {data ? (
          <div className="mt-6 overflow-x-auto">
            <table className="w-full min-w-96 border-collapse text-left font-mono text-sm">
              <tbody>
                {Object.entries(data.params).map(([key, value]) => (
                  <tr key={key} className="border-t border-line">
                    <th scope="row" className="w-40 py-3 pr-6 font-normal text-muted md:w-[15.5rem]">
                      {key}
                    </th>
                    <td className="py-3">{readable(key, value)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : null}
      </section>

      <section className="cell px-5 py-16 md:px-10 md:py-20">
        <Eyebrow>raw endpoints</Eyebrow>
        <ul className="mt-6 grid gap-x-8 gap-y-2 font-mono text-sm sm:grid-cols-2">
          {ENDPOINTS.map((path) => (
            <li key={path}>
              <a href={path} target="_blank" rel="noopener noreferrer" className="underline underline-offset-4">
                {path}
              </a>
            </li>
          ))}
        </ul>
        <p className="mt-6 max-w-2xl text-sm text-muted">
          Compare the running hash from a shell: <span className="font-mono">curl -s /v1/info</span> on this
          origin, then hash the binary you built with <span className="font-mono">sha256sum</span>.
        </p>
      </section>
    </>
  )
}
