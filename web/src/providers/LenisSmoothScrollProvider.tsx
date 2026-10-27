import Lenis from 'lenis'
import { useEffect } from 'react'
import { gsap } from 'gsap'
import { ScrollTrigger } from 'gsap/ScrollTrigger'

gsap.registerPlugin(ScrollTrigger)

export default function LenisSmoothScrollProvider() {
  useEffect(() => {
    const mm = gsap.matchMedia()

    mm.add('(prefers-reduced-motion: no-preference)', () => {
      const lenis = new Lenis({ autoRaf: false, lerp: 0.1, smoothWheel: true })
      const raf = (time: number) => lenis.raf(time * 1000)

      lenis.on('scroll', ScrollTrigger.update)
      gsap.ticker.add(raf)
      gsap.ticker.lagSmoothing(0)

      return () => {
        gsap.ticker.remove(raf)
        lenis.destroy()
      }
    })

    return () => mm.revert()
  }, [])

  return null
}
