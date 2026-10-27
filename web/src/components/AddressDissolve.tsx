import { useEffect, useRef, useState } from 'react'
import { gsap } from 'gsap'
import { ScrambleTextPlugin } from 'gsap/ScrambleTextPlugin'
import DecryptedText from '@/components/reactbits/DecryptedText'

gsap.registerPlugin(ScrambleTextPlugin)

type Phase = 'resolve' | 'redact' | 'gone'

const RESOLVE_MS = 3600
const GONE_MS = 2000

// Resolve, scramble into redaction glyphs, blur out, repeat while visible. Reduced motion shows the end state.
export default function AddressDissolve({ address, label }: { address: string; label: string }) {
  const rootRef = useRef<HTMLDivElement>(null)
  const redactRef = useRef<HTMLSpanElement>(null)
  const [visible, setVisible] = useState(false)
  const [phase, setPhase] = useState<Phase>('resolve')
  const [cycle, setCycle] = useState(0)
  const [still, setStill] = useState(false)
  const glyphs = '█'.repeat(address.length)

  useEffect(() => {
    const root = rootRef.current
    if (!root) return
    const io = new IntersectionObserver(([entry]) => setVisible(entry.isIntersecting))
    io.observe(root)
    return () => io.disconnect()
  }, [])

  useEffect(() => {
    const mm = gsap.matchMedia()
    mm.add('(prefers-reduced-motion: reduce)', () => setStill(true))
    mm.add('(prefers-reduced-motion: no-preference)', () => setStill(false))
    return () => mm.revert()
  }, [])

  useEffect(() => {
    if (!visible || still) return
    if (phase === 'resolve') {
      const t = window.setTimeout(() => setPhase('redact'), RESOLVE_MS)
      return () => window.clearTimeout(t)
    }
    if (phase === 'redact') {
      const el = redactRef.current
      if (!el) return
      const tl = gsap.timeline({ onComplete: () => setPhase('gone') })
      tl.to(el, {
        duration: 1.1,
        ease: 'none',
        scrambleText: { text: glyphs, chars: '█▓▒░', speed: 0.6, revealDelay: 0.15 },
      }).to(el, { opacity: 0, filter: 'blur(8px)', duration: 0.9, ease: 'expo.out' }, '+=0.25')
      return () => {
        tl.kill()
      }
    }
    const t = window.setTimeout(() => {
      setCycle((c) => c + 1)
      setPhase('resolve')
    }, GONE_MS)
    return () => window.clearTimeout(t)
  }, [visible, still, phase, glyphs])

  const spoken = `${label}. A donor address that resolves, is redacted, then forgotten.`

  return (
    <div ref={rootRef} role="img" aria-label={spoken} className="min-h-24 sm:min-h-16">
      <p className="mb-3 font-mono text-xs lowercase text-muted">{label}</p>
      <div aria-hidden="true" className="break-all font-mono text-sm leading-6 text-fg sm:text-base">
        {still ? (
          <span className="opacity-40">{glyphs}</span>
        ) : phase === 'resolve' ? (
          <DecryptedText
            key={cycle}
            text={address}
            animateOn="view"
            sequential
            speed={22}
            characters="abcdefghijklmnopqrstuvwxyz0123456789"
            encryptedClassName="text-muted"
          />
        ) : phase === 'redact' ? (
          <span key="redact" ref={redactRef}>{address}</span>
        ) : (
          <span key="gone" className="text-muted">
            address forgotten
          </span>
        )}
      </div>
    </div>
  )
}
