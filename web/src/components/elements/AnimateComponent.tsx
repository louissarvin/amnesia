import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { gsap } from 'gsap'
import { cnm } from '@/utils/style'

type EntryAnimation =
  | 'fadeIn'
  | 'fadeInUp'
  | 'fadeInDown'
  | 'fadeInLeft'
  | 'fadeInRight'
  | 'scaleIn'
  | 'slideUp'

interface AnimateComponentProps {
  /** Entry animation style */
  entry?: EntryAnimation
  /** GSAP easing function */
  ease?: string
  /** Animation duration in milliseconds */
  duration?: number
  /** Animation delay in milliseconds */
  delay?: number
  /** Additional CSS classes */
  className?: string
  /** Child elements to animate */
  children: React.ReactNode
  /** Whether to trigger animation on scroll */
  onScroll?: boolean
  /** Intersection observer threshold (0 to 1) */
  threshold?: number
  /** Intersection observer root margin */
  rootMargin?: string
  /** Whether to reset animation when element leaves viewport */
  resetOnLeave?: boolean
  /** Whether to stagger children animations */
  stagger?: boolean
  /** Stagger delay between children */
  staggerDelay?: number
}

/**
 * AnimateComponent - GSAP-powered entrance and exit animations
 *
 * @example
 * // Basic usage
 * <AnimateComponent>
 *   <h1>Hello World</h1>
 * </AnimateComponent>
 *
 * @example
 * // Scroll-triggered animation
 * <AnimateComponent onScroll entry="fadeInUp" delay={200}>
 *   <Card />
 * </AnimateComponent>
 */
const reducedMotion = () => window.matchMedia('(prefers-reduced-motion: reduce)').matches

export default function AnimateComponent({
  entry = 'fadeInUp',
  ease = 'expo.out',
  duration = 800,
  delay = 0,
  className,
  children,
  onScroll = false,
  threshold = 0.2,
  rootMargin = '-10%',
  resetOnLeave = false,
}: AnimateComponentProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const [isInView, setIsInView] = useState(!onScroll)
  const hasAnimated = useRef(false)

  // Convert ms to s for GSAP
  const durationInSec = duration / 1000
  const delayInSec = delay / 1000

  useLayoutEffect(() => {
    const el = containerRef.current
    if (!el || reducedMotion()) return

    gsap.set(el, { autoAlpha: 0 })

    if (onScroll) {
      const observer = new IntersectionObserver(
        (entries) => {
          entries.forEach((item) => {
            if (item.isIntersecting && !hasAnimated.current) {
              setIsInView(true)
              if (!resetOnLeave) {
                hasAnimated.current = true
              }
            } else if (!item.isIntersecting && resetOnLeave) {
              setIsInView(false)
              hasAnimated.current = false
            }
          })
        },
        {
          threshold,
          rootMargin,
        }
      )

      observer.observe(el)
      return () => observer.disconnect()
    }

    return undefined
  }, [onScroll, threshold, rootMargin, resetOnLeave])

  useEffect(() => {
    const el = containerRef.current
    if (!el || !isInView || reducedMotion()) return

    // Animate entry
    const [fromVars, toVars] = getEntryGSAPVars(entry)
    const tl = gsap.timeline()

    tl.fromTo(
      el,
      { ...fromVars, autoAlpha: 0 },
      {
        ...toVars,
        autoAlpha: 1,
        ease,
        duration: durationInSec,
        delay: delayInSec,
      }
    )

    return () => {
      tl.kill()
    }
  }, [
    isInView,
    entry,
    ease,
    durationInSec,
    delayInSec,
    onScroll,
    resetOnLeave,
  ])

  return (
    <div ref={containerRef} className={cnm(className)}>
      {children}
    </div>
  )
}

function getEntryGSAPVars(
  animation: EntryAnimation
): [gsap.TweenVars, gsap.TweenVars] {
  const animations: Record<EntryAnimation, [gsap.TweenVars, gsap.TweenVars]> = {
    fadeIn: [{ opacity: 0 }, { opacity: 1 }],
    fadeInUp: [{ y: 40, opacity: 0 }, { y: 0, opacity: 1 }],
    fadeInDown: [{ y: -40, opacity: 0 }, { y: 0, opacity: 1 }],
    fadeInLeft: [{ x: -40, opacity: 0 }, { x: 0, opacity: 1 }],
    fadeInRight: [{ x: 40, opacity: 0 }, { x: 0, opacity: 1 }],
    scaleIn: [{ scale: 0.9, opacity: 0 }, { scale: 1, opacity: 1 }],
    slideUp: [{ y: 60, opacity: 0 }, { y: 0, opacity: 1 }],
  }

  return animations[animation]
}
