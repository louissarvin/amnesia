import { Link } from '@tanstack/react-router'
import Headline from '@/components/Headline'

const W = 1200
const H = 160

// Two periods per path so a half-width slide loops seamlessly
const wave = (amp: number, phase: number) => {
  const pts: Array<string> = []
  for (let x = 0; x <= W * 2; x += 20) {
    const y = H / 2 + Math.sin((x / W) * Math.PI * 2 * 2 + phase) * amp
    pts.push(`${x === 0 ? 'M' : 'L'}${x} ${y.toFixed(1)}`)
  }
  return pts.join('')
}

const LINES = [
  { amp: 38, phase: 0, t: '22s', o: 0.9 },
  { amp: 28, phase: 1.4, t: '30s', o: 0.6 },
  { amp: 46, phase: 2.8, t: '38s', o: 0.4 },
  { amp: 20, phase: 4.2, t: '26s', o: 0.5 },
]

const ENDPOINTS = ['/v1/summary', '/v1/shred-log', '/v1/info', '/demo/history']

export default function FooterCta() {
  return (
    <section data-rail="Seize" className="cell overflow-hidden px-5 pt-24 text-center md:pt-36">
      <Headline className="display mx-auto max-w-3xl text-4xl md:text-6xl">
        Do not take our word for it. <span className="text-muted">Seize it.</span>
      </Headline>
      <div className="mt-10 flex flex-wrap justify-center gap-3">
        <Link to="/seize" className="pill pill-solid">
          Open the seizure lab
          <span aria-hidden="true" className="arrow">
            →
          </span>
        </Link>
        <Link to="/donate" className="pill">
          Make a testnet donation
        </Link>
      </div>
      <p className="mt-8 flex flex-wrap justify-center gap-x-5 gap-y-2 font-mono text-[11px] tracking-[0.04em] text-muted uppercase">
        Raw endpoints
        {ENDPOINTS.map((e) => (
          <a key={e} href={e} className="underline underline-offset-4 hover:text-fg">
            {e}
          </a>
        ))}
      </p>
      <div aria-hidden="true" className="relative mt-16 h-40 overflow-hidden">
        {LINES.map((l) => (
          <svg
            key={l.phase}
            viewBox={`0 0 ${W * 2} ${H}`}
            preserveAspectRatio="none"
            className="wave absolute top-0 left-0 h-full w-[200%] fill-none text-fg"
            style={{ ['--t' as string]: l.t, opacity: l.o }}
          >
            <path d={wave(l.amp, l.phase)} stroke="currentColor" strokeWidth={1.5} vectorEffect="non-scaling-stroke" />
          </svg>
        ))}
      </div>
    </section>
  )
}
