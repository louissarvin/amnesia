const S = 14
const ACCENT = 'stroke-gold-ink fill-gold-ink/10 [fill-opacity:1]'
const iso = (x: number, y: number, z: number) =>
  `${((x - y) * 0.866 * S).toFixed(1)},${((x + y) * 0.5 * S - z * S).toFixed(1)}`
const poly = (pts: Array<[number, number, number]>) => `M${pts.map((p) => iso(...p)).join('L')}Z`

function Box({ x, y, z, w, d, h, delay = 0 }: { x: number; y: number; z: number; w: number; d: number; h: number; delay?: number }) {
  const faces = [
    poly([[x, y, z + h], [x + w, y, z + h], [x + w, y + d, z + h], [x, y + d, z + h]]),
    poly([[x, y + d, z], [x + w, y + d, z], [x + w, y + d, z + h], [x, y + d, z + h]]),
    poly([[x + w, y, z], [x + w, y + d, z], [x + w, y + d, z + h], [x + w, y, z + h]]),
  ]
  return (
    <>
      {faces.map((f, i) => (
        <path key={i} d={f} pathLength={1} className="draw" style={{ ['--d' as string]: `${delay + i * 0.15}s` }} />
      ))}
    </>
  )
}

// Floor plate with a faint grid, so every scene sits on something
function Plate({ z = -1.6 }: { z?: number }) {
  const lines = [-2, 0, 2].flatMap((k) => [
    `M${iso(k, -4, z)}L${iso(k, 4, z)}`,
    `M${iso(-4, k, z)}L${iso(4, k, z)}`,
  ])
  return (
    <g strokeOpacity={0.45}>
      <path d={poly([[-4, -4, z], [4, -4, z], [4, 4, z], [-4, 4, z]])} pathLength={1} className="draw" />
      {lines.map((d, i) => (
        <path key={i} d={d} pathLength={1} className="draw" strokeDasharray="1" style={{ ['--d' as string]: `${0.3 + i * 0.1}s` }} />
      ))}
    </g>
  )
}

function Frame({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <svg
      viewBox="0 0 240 150"
      role="img"
      aria-label={`Illustration: ${label}`}
      className="h-full w-full text-fg/64"
      fill="currentColor"
      fillOpacity={0.06}
      stroke="currentColor"
      strokeWidth={1.3}
      strokeLinejoin="round"
      strokeLinecap="round"
    >
      <g transform="translate(120 74)">{children}</g>
    </svg>
  )
}

export function OpenDiagram() {
  return (
    <Frame label="notes dropping into an open pool">
      <Plate />
      <Box x={-3} y={-3} z={-1.6} w={6} d={6} h={1.6} />
      {[[-1.5, -1, 3], [0.5, -2, 4], [1.2, 0.8, 3.5], [-2, 1.2, 4.5]].map(([x, y, z], i) => (
        <path
          key={i}
          d={poly([[x, y, z], [x + 1, y, z], [x + 1, y + 1, z], [x, y + 1, z]])}
          className={`d-drop ${ACCENT}`}
          style={{ ['--d' as string]: `${i * 0.6}s` }}
        />
      ))}
    </Frame>
  )
}

export function ClosedDiagram() {
  return (
    <Frame label="a gate closing over a pool">
      <Plate />
      <Box x={-3} y={-3} z={-1.6} w={6} d={6} h={1.6} />
      <path d={poly([[-3, 3, 0.2], [3, 3, 0.2], [3, 3, 3.4], [-3, 3, 3.4]])} className={`d-lid ${ACCENT}`} />
    </Frame>
  )
}

function Coin({ cx, cy }: { cx: number; cy: number }) {
  return <ellipse cx={cx} cy={cy} rx={11} ry={6} />
}

export function SweepDiagram() {
  const stacks = [
    { cx: 10, n: 1 },
    { cx: 50, n: 2 },
    { cx: 90, n: 5 },
  ]
  return (
    <Frame label="coins split into stacks of one, two and five">
      <path d="M-100 28C-70 28 -50 28 -22 28" pathLength={1} className="d-flow" />
      {stacks.map(({ cx, n }) => (
        <g key={cx}>
          {Array.from({ length: n }, (_, i) => (
            <g key={i} className={i === n - 1 ? 'd-bob' : undefined} style={{ ['--d' as string]: `${cx / 100}s` }}>
              <Coin cx={cx - 20} cy={30 - i * 7} />
            </g>
          ))}
        </g>
      ))}
      {[0, 1, 2].map((i) => (
        <Coin key={i} cx={-60 + i * 9} cy={32 - i * 4} />
      ))}
    </Frame>
  )
}

export function BuryDiagram() {
  return (
    <Frame label="a box buried under a growing stack of blocks">
      <Plate z={0} />
      <Box x={-2} y={-2} z={-2.5} w={4} d={4} h={2.5} delay={0.2} />
      {[0, 1, 2].map((i) => (
        <g key={i} className={`d-layer ${ACCENT}`} style={{ ['--d' as string]: `${i * 0.5}s` }}>
          <Box x={-2.5} y={-2.5} z={0.4 + i * 0.7} w={5} d={5} h={0.35} />
        </g>
      ))}
    </Frame>
  )
}

export function ShredDiagram() {
  const strips = [0, 1, 2, 3, 4]
  return (
    <Frame label="a box breaking into strips">
      <Plate z={0.6} />
      {strips.map((i) => (
        <g
          key={i}
          className="d-strip"
          style={{ ['--x' as string]: `${(i - 2) * 8}px`, ['--y' as string]: `${14 + (i % 2) * 8}px`, ['--r' as string]: `${(i - 2) * 5}deg`, ['--d' as string]: `${i * 0.08}s` }}
        >
          <Box x={-3 + i * 1.2} y={-3} z={0} w={1.2} d={6} h={2.4} delay={i * 0.1} />
        </g>
      ))}
    </Frame>
  )
}
