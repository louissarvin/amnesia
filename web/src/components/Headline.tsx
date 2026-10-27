import { gsap } from 'gsap'
import { SplitText } from 'gsap/SplitText'
import { useLayoutEffect, useRef } from 'react'
import type { ReactNode } from 'react'

gsap.registerPlugin(SplitText)

interface HeadlineProps {
  as?: 'h1' | 'h2'
  className?: string
  delay?: number
  children: ReactNode
}

// Line-by-line reveal: blur 8px to 0 and y 24px to 0, 60ms stagger, once on first view.
export default function Headline({ as: Tag = 'h2', className, delay = 0, children }: HeadlineProps) {
  const ref = useRef<HTMLHeadingElement>(null)

  useLayoutEffect(() => {
    const el = ref.current
    if (!el) return
    const mm = gsap.matchMedia()
    mm.add('(prefers-reduced-motion: no-preference)', () => {
      let tween: gsap.core.Tween | undefined
      let played = false
      let visible = false
      const play = (lines: Array<Element>) => {
        tween?.kill()
        tween = gsap.fromTo(
          lines,
          { opacity: 0, filter: 'blur(8px)', y: 24 },
          { opacity: 1, filter: 'blur(0px)', y: 0, duration: 0.9, stagger: 0.06, ease: 'expo.out', delay, clearProps: 'filter,transform' },
        )
        played = true
      }
      const split = SplitText.create(el, {
        type: 'lines',
        aria: 'auto',
        autoSplit: true,
        onSplit: (self: SplitText) => {
          // A re-split (fonts loaded, resize) after the reveal makes fresh lines: show them as is.
          if (played) {
            gsap.set(self.lines, { opacity: 1, clearProps: 'filter,transform' })
            return
          }
          gsap.set(self.lines, { opacity: 0 })
          if (visible) play(self.lines)
        },
      })
      const io = new IntersectionObserver(([e]) => {
        if (!e.isIntersecting || visible) return
        visible = true
        io.disconnect()
        if (!played) play(split.lines)
      })
      io.observe(el)
      return () => {
        io.disconnect()
        tween?.kill()
        split.revert()
      }
    })
    return () => mm.revert()
  }, [delay])

  return (
    <Tag ref={ref} className={className}>
      {children}
    </Tag>
  )
}
