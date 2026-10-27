import { useQuery } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { Suspense, lazy, useCallback, useEffect, useRef, useState } from 'react'
import Headline from '@/components/Headline'
import CountUp from '@/components/reactbits/CountUp'
import { summaryQuery } from '@/lib/api'
import { cnm } from '@/utils/style'

// three.js stays out of the main bundle: it loads only here, only when motion is allowed
const KeyScene = lazy(() => import('@/components/home/KeyScene'))

const REDUCED = '(prefers-reduced-motion: reduce)'

function LiveSentence() {
  const { data, isError } = useQuery(summaryQuery)
  if (isError) return <>The desk is unreachable right now, so there is no live count.</>
  return (
    <>
      Already forgot{' '}
      <span className="inline-flex justify-center rounded-full border border-gold/50 bg-gold/10 px-2.5 py-0.5 font-mono text-[0.85em] text-gold-ink">
        {data ? <CountUp to={data.epochs_shredded} /> : '…'}
      </span>{' '}
      epochs. Their donors can no longer be named.
    </>
  )
}

export default function Hero() {
  const wrapRef = useRef<HTMLElement>(null)
  const [still, setStill] = useState(() => window.matchMedia(REDUCED).matches)
  const [ready, setReady] = useState(false)
  const onReady = useCallback(() => setReady(true), [])
  const onFail = useCallback(() => {
    setStill(true)
    setReady(false)
  }, [])

  useEffect(() => {
    const mq = window.matchMedia(REDUCED)
    const sync = () => setStill(mq.matches)
    mq.addEventListener('change', sync)
    return () => mq.removeEventListener('change', sync)
  }, [])

  return (
    <section ref={wrapRef} id="top" data-rail="Donate" className={cnm('relative', still ? 'h-svh min-h-160' : 'h-[150svh]')}>
      {still ? null : (
        <Suspense fallback={null}>
          <KeyScene trigger={wrapRef} onReady={onReady} onFail={onFail} />
        </Suspense>
      )}
      <div className="sticky top-0 h-svh min-h-160 overflow-hidden">
        <img
          src="/key-still.png"
          alt=""
          width={960}
          height={520}
          fetchPriority="high"
          className={cnm(
            'pointer-events-none absolute top-[31%] left-1/2 w-[min(96vw,730px)] -translate-x-1/2 -translate-y-1/2 transition-opacity duration-700',
            ready && !still ? 'opacity-0' : 'opacity-100',
          )}
        />

        <div className="absolute inset-x-0 bottom-0 flex flex-col items-center px-5 pb-8 text-center sm:pb-12">
          <Headline as="h1" delay={0.15} className="display max-w-4xl text-[2.75rem] sm:text-7xl md:text-8xl">
            A donation desk that forgets.
          </Headline>
          <p className="mt-5 text-base text-muted sm:text-lg">
            <LiveSentence />
          </p>
          <div className="mt-7 flex flex-wrap justify-center gap-3">
            <Link to="/seize" className="pill pill-solid">
              Seize it yourself
              <span aria-hidden="true" className="arrow">
                →
              </span>
            </Link>
            <Link to="/donate" className="pill">
              Make a donation
            </Link>
          </div>
          <p className={cnm('mt-6 font-mono text-[11px] tracking-[0.08em] text-muted uppercase', still && 'invisible')}>
            Scroll to shred the key
          </p>
        </div>
      </div>
    </section>
  )
}
