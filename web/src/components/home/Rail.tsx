import { useEffect, useState } from 'react'
import { cnm } from '@/utils/style'

const STEPS = ['Donate', 'Address', 'Epoch', 'Shred', 'Seize'] as const

// Fixed dotted progress on the right edge; the active step follows the section that holds the viewport centre.
export default function Rail() {
  const [active, setActive] = useState<string>(STEPS[0])

  useEffect(() => {
    const sections = document.querySelectorAll<HTMLElement>('[data-rail]')
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) if (e.isIntersecting) setActive((e.target as HTMLElement).dataset.rail ?? STEPS[0])
      },
      { rootMargin: '-50% 0px -50% 0px' },
    )
    sections.forEach((s) => io.observe(s))
    return () => io.disconnect()
  }, [])

  return (
    <nav aria-label="Page progress" className="fixed top-1/2 right-5 z-30 hidden -translate-y-1/2 xl:block">
      <ol className="flex flex-col items-end gap-4">
        {STEPS.map((label) => {
          const on = label === active
          return (
            <li key={label} className="flex items-center gap-3">
              <span
                className={cnm(
                  'font-mono text-[10px] tracking-[0.08em] uppercase transition-opacity duration-300',
                  on ? 'text-fg opacity-100' : 'opacity-0',
                )}
              >
                {label}
              </span>
              <span
                aria-current={on ? 'step' : undefined}
                className={cnm('block rounded-full transition-all duration-300', on ? 'size-2 bg-gold' : 'size-1.5 bg-line-strong')}
              >
                <span className="sr-only">{label}</span>
              </span>
            </li>
          )
        })}
      </ol>
    </nav>
  )
}
