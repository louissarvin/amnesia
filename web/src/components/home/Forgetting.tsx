import { gsap } from 'gsap'
import { ScrollTrigger } from 'gsap/ScrollTrigger'
import { SplitText } from 'gsap/SplitText'
import { useEffect, useRef } from 'react'
import Eyebrow from '@/components/Eyebrow'

gsap.registerPlugin(ScrollTrigger, SplitText)

export default function Forgetting() {
  const ref = useRef<HTMLParagraphElement>(null)

  useEffect(() => {
    const el = ref.current
    if (!el) return
    const mm = gsap.matchMedia()
    mm.add('(prefers-reduced-motion: no-preference)', () => {
      const split = SplitText.create(el, { type: 'words', aria: 'auto' })
      const tween = gsap.fromTo(
        split.words,
        { opacity: 0.2 },
        {
          opacity: 1,
          stagger: 0.1,
          ease: 'none',
          scrollTrigger: { trigger: el, start: 'top 80%', end: 'bottom 50%', scrub: 0.6 },
        },
      )
      return () => {
        tween.scrollTrigger?.kill()
        tween.kill()
        split.revert()
      }
    })
    return () => mm.revert()
  }, [])

  return (
    <section data-rail="Shred" className="cell px-5 py-24 md:px-10 md:py-32">
      <Eyebrow>what a seizure finds</Eyebrow>
      <p ref={ref} className="display mt-6 max-w-5xl text-3xl sm:text-5xl md:text-6xl">
        What the desk never keeps, no warrant can take. Every address is used once, and every epoch is deleted.
</p>
    </section>
  )
}
