import { animate, useInView, useReducedMotion } from 'motion/react'
import { useEffect, useRef } from 'react'
import { EASE_OUT_EXPO } from '@/config/animation'

interface CountUpProps {
  to: number
  duration?: number
  className?: string
}

// Counts from the last shown value to `to` once visible. Tabular digits and a hidden sizer hold the final width, so nothing shifts.
export default function CountUp({ to, duration = 1.6, className }: CountUpProps) {
  const ref = useRef<HTMLSpanElement>(null)
  const shown = useRef(0)
  const inView = useInView(ref, { once: true })
  const reduce = useReducedMotion()
  const final = to.toLocaleString('en-US')

  useEffect(() => {
    const el = ref.current
    if (!el || !inView) return
    const write = (v: number) => {
      shown.current = v
      el.textContent = Math.round(v).toLocaleString('en-US')
    }
    if (reduce) {
      write(to)
      return
    }
    const controls = animate(shown.current, to, { duration, ease: EASE_OUT_EXPO, onUpdate: write })
    return () => controls.stop()
  }, [to, inView, reduce, duration])

  return (
    <span className={`inline-grid tabular-nums ${className ?? ''}`}>
      <span aria-hidden="true" className="invisible col-start-1 row-start-1">
        {final}
      </span>
      <span ref={ref} aria-hidden="true" className="col-start-1 row-start-1">
        0
      </span>
      <span className="sr-only">{final}</span>
    </span>
  )
}
