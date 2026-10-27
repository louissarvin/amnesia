import { useQuery } from '@tanstack/react-query'
import { gsap } from 'gsap'
import { useEffect, useRef } from 'react'
import { BuryDiagram, ClosedDiagram, OpenDiagram, ShredDiagram, SweepDiagram } from '@/components/home/Diagrams'
import Eyebrow from '@/components/Eyebrow'
import Headline from '@/components/Headline'
import { config } from '@/config'
import { infoQuery } from '@/lib/api'
import { cnm } from '@/utils/style'
import { formatDuration } from '@/utils/format'

function BlockCounter({ blocks }: { blocks: number }) {
  const ref = useRef<HTMLSpanElement>(null)

  useEffect(() => {
    const el = ref.current
    if (!el) return
    const mm = gsap.matchMedia()
    mm.add('(prefers-reduced-motion: no-preference)', () => {
      const state = { v: 0 }
      const tween = gsap.to(state, {
        v: blocks,
        duration: 4.8,
        ease: 'power1.inOut',
        repeat: -1,
        repeatDelay: 0.6,
        onUpdate: () => {
          el.textContent = String(Math.round(state.v))
        },
      })
      const card = el.closest('.diagram-card')
      const io = new IntersectionObserver(([e]) => (e.isIntersecting ? tween.play() : tween.pause()))
      if (card) io.observe(card)
      return () => {
        io.disconnect()
        tween.kill()
      }
    })
    return () => mm.revert()
  }, [blocks])

  return (
    <span className="font-mono text-[11px] tracking-[0.06em] uppercase">
      block <span ref={ref}>{blocks}</span> / {blocks}
    </span>
  )
}

export default function Lifecycle() {
  const { data } = useQuery(infoQuery)
  const gridRef = useRef<HTMLDivElement>(null)
  const p = data?.params
  const num = (key: string) => {
    const v = p?.[key]
    return typeof v === 'number' ? v : null
  }
  const epoch = num('epoch_s')
  const grace = num('grace_s')
  const blocks = num('burial_blocks')
  const maxCanonical = num('max_canonical')
  const cap = num('cap')

  useEffect(() => {
    const grid = gridRef.current
    if (!grid) return
    const io = new IntersectionObserver((entries) => {
      for (const e of entries) (e.target as HTMLElement).dataset.live = String(e.isIntersecting)
    })
    grid.querySelectorAll('.diagram').forEach((el) => io.observe(el))
    return () => io.disconnect()
  }, [])

  const cards = [
    {
      name: 'Open',
      span: 'md:col-span-2',
      text: `Takes new donors${epoch ? ` for ${formatDuration(epoch)}` : ''}. Each donor gets one address${cap ? `, up to ${cap} per epoch` : ''}.`,
      diagram: <OpenDiagram />,
    },
    {
      name: 'Closed',
      span: 'md:col-span-2',
      text: `No new addresses. Late payments still land for a ${grace ? formatDuration(grace) : 'fixed'} grace window.`,
      diagram: <ClosedDiagram />,
    },
    {
      name: 'Sweeping',
      span: 'md:col-span-2',
      text: `The pool moves to the treasury in canonical amounts${maxCanonical ? `, at most ${maxCanonical} per sweep` : ''}, so amounts say little about who paid.`,
      diagram: <SweepDiagram />,
    },
    {
      name: 'Burying',
      span: 'md:col-span-3',
      text: `The keys wait ${blocks ? `${blocks} blocks, about ${formatDuration(blocks * config.blockSeconds)},` : 'a fixed number of blocks'} after the last transaction, so a late payment cannot strand funds.`,
      diagram: <BuryDiagram />,
      extra: blocks ? <BlockCounter blocks={blocks} /> : null,
    },
    {
      name: 'Shredded',
      span: 'md:col-span-3',
      text: 'Keys and wallet are overwritten and deleted. The shred log keeps only a bucketed record that the epoch existed.',
      diagram: <ShredDiagram />,
    },
  ]

  return (
    <section id="lifecycle" data-rail="Epoch" className="cell scroll-mt-20">
      <div className="px-6 py-16 md:px-10 md:py-24">
        <Eyebrow>epoch lifecycle</Eyebrow>
        <Headline className="display mt-5 max-w-2xl text-4xl md:text-6xl">Five stages, and the last one is deletion.</Headline>
      </div>

      <div ref={gridRef} className="grid gap-4 px-4 pb-16 md:grid-cols-6 md:px-8 md:pb-24">
        {cards.map((card, i) => (
          <article
            key={card.name}
            className={cnm(
              'diagram-card spot relative flex flex-col overflow-hidden rounded-3xl border border-line bg-bg p-6 md:p-8',
              card.span,
            )}
          >
            <p className="relative font-mono text-[11px] tracking-[0.08em] text-muted">[{String(i + 1).padStart(2, '0')}]</p>
            <div data-live="false" className="diagram relative my-4 flex h-55 items-center justify-center">
              {card.diagram}
            </div>
            <h3 className="relative text-2xl font-medium tracking-[-0.03em]">{card.name}</h3>
            <p className="relative mt-2 max-w-md text-sm text-muted">{card.text}</p>
            {card.extra ? <div className="relative mt-3">{card.extra}</div> : null}
          </article>
        ))}
      </div>
      <p className="px-6 pb-8 font-mono text-[11px] tracking-[0.04em] text-muted uppercase md:px-10">
        Diagrams are illustrations. Timings and limits above are read from /v1/info.
      </p>
    </section>
  )
}
