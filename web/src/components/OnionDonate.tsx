import { useMemo, useState } from 'react'
import { encode } from 'uqr'
import { config } from '@/config'

// Drawn as React rects from the encoded matrix: no injected SVG markup.
function Qr({ text }: { text: string }) {
  const { data, size } = useMemo(() => encode(text, { ecc: 'M', border: 2 }), [text])
  const cells = useMemo(() => {
    const out: Array<string> = []
    data.forEach((row, y) => row.forEach((on, x) => on && out.push(`M${x} ${y}h1v1h-1z`)))
    return out.join('')
  }, [data])
  return (
    <svg
      viewBox={`0 0 ${size} ${size}`}
      role="img"
      aria-label="QR code of the onion address"
      className="size-32 shrink-0 rounded-xl bg-[#f3f1ef] md:size-36"
      shapeRendering="crispEdges"
    >
      <path d={cells} fill="#07080a" />
    </svg>
  )
}

const STEPS = [
  'Open the address in Tor Browser.',
  'Solve the short proof of work, or wait a minute with JavaScript off, to get a one-time shielded address.',
  'Pay it from your own testnet wallet, with no memo, before its deadline.',
]

export default function OnionDonate() {
  const [copied, setCopied] = useState<'idle' | 'copied' | 'failed'>('idle')
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(config.onionUrl)
      setCopied('copied')
    } catch {
      setCopied('failed')
    }
    window.setTimeout(() => setCopied('idle'), 1600)
  }

  return (
    <section aria-labelledby="onion-title" className="card flex flex-col gap-6 p-5 md:p-6">
      <div>
        <p className="font-mono text-[11px] tracking-[0.06em] text-muted uppercase">your own wallet</p>
        <h2 id="onion-title" className="mt-2 text-xl font-medium tracking-[-0.02em]">
          Donate for real, over Tor
        </h2>
        <p className="mt-2 text-sm text-muted">
          No wallet connection, no account. The desk only ever sees a Tor circuit and a payment to an
          address it will later forget.
        </p>
      </div>
      <div className="flex flex-col gap-5 sm:flex-row sm:items-start">
        <Qr text={config.onionUrl} />
        <ol className="flex min-w-0 flex-col gap-3 text-sm">
          {STEPS.map((s, i) => (
            <li key={s} className="grid grid-cols-[1.5rem_1fr] gap-2">
              <span className="font-mono text-xs text-muted tabular-nums">{String(i + 1).padStart(2, '0')}</span>
              <span>{s}</span>
            </li>
          ))}
        </ol>
      </div>
      <div className="flex items-center gap-3 rounded-2xl border border-line py-2 pr-2 pl-4">
        <p className="min-w-0 flex-1 truncate font-mono text-xs" title={config.onionUrl}>
          {config.onionUrl.replace('http://', '')}
        </p>
        <button type="button" onClick={copy} className="pill shrink-0 px-3 py-2 text-[11px]">
          {copied === 'copied' ? 'Copied' : copied === 'failed' ? 'Copy failed' : 'Copy'}
        </button>
      </div>
    </section>
  )
}
