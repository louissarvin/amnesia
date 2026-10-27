import { useEffect } from 'react'

const REDUCED = '(prefers-reduced-motion: reduce)'
const CHARS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789/<>_'
const SCRAMBLE_MS = 300

function firstText(el: Element): Text | null {
  const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT)
  for (let n = walker.nextNode(); n; n = walker.nextNode()) if (n.nodeValue?.trim()) return n as Text
  return null
}

// Sui-style scramble on one text node; a gold block (CSS Custom Highlight) marks the resolving character.
function scramble(node: Text) {
  const original = node.data
  const len = original.length
  const range = new Range()
  const start = performance.now()
  let last = original
  let raf = 0
  const frame = (now: number) => {
    // rAF timestamps mark the frame start, which can precede `start`: never go negative.
    const t = Math.min(1, Math.max(0, (now - start) / SCRAMBLE_MS))
    const done = Math.floor(t * len)
    if (t >= 1) {
      CSS.highlights.delete('scramble-cursor')
      if (node.data === last) node.data = original
      return
    }
    let out = ''
    for (let i = 0; i < len; i++) {
      const c = original[i]
      out += i < done || c === ' ' ? c : CHARS[Math.floor(Math.random() * CHARS.length)]
    }
    last = out
    node.data = out
    range.setStart(node, Math.min(done, len - 1))
    range.setEnd(node, Math.min(done + 1, len))
    CSS.highlights.set('scramble-cursor', new Highlight(range))
    raf = requestAnimationFrame(frame)
  }
  raf = requestAnimationFrame(frame)
  return () => {
    cancelAnimationFrame(raf)
    CSS.highlights.delete('scramble-cursor')
    if (node.data === last) node.data = original
  }
}

// Global, delegated motion: cell draw-in, spotlight borders, mono scramble. No React renders on pointer moves.
export default function MotionRuntime() {
  useEffect(() => {
    const mq = window.matchMedia(REDUCED)

    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (!e.isIntersecting) continue
          ;(e.target as HTMLElement).dataset.draw = 'in'
          io.unobserve(e.target)
        }
      },
      { rootMargin: '0px 0px -8% 0px' },
    )
    const arm = (root: ParentNode) => {
      if (mq.matches) return
      root.querySelectorAll<HTMLElement>('.cell:not([data-draw])').forEach((el) => {
        Array.from(el.children).forEach((c, i) => (c as HTMLElement).style.setProperty('--i', String(Math.min(i, 5))))
        el.dataset.draw = 'pending'
        io.observe(el)
      })
    }
    arm(document)
    let queued = 0
    const mo = new MutationObserver(() => {
      if (queued) return
      queued = requestAnimationFrame(() => {
        queued = 0
        arm(document)
      })
    })
    mo.observe(document.body, { childList: true, subtree: true })

    const onMove = (e: PointerEvent) => {
      const el = (e.target as Element | null)?.closest<HTMLElement>('.spot, .card')
      if (!el) return
      const r = el.getBoundingClientRect()
      el.style.setProperty('--x', `${e.clientX - r.left}px`)
      el.style.setProperty('--y', `${e.clientY - r.top}px`)
    }
    document.addEventListener('pointermove', onMove, { passive: true })

    const running = new WeakMap<Element, () => void>()
    const canScramble = typeof Highlight !== 'undefined' && 'highlights' in CSS
    const onOver = (e: PointerEvent) => {
      if (!canScramble || mq.matches || e.pointerType !== 'mouse') return
      const el = (e.target as Element | null)?.closest('[data-scramble], .pill, .chip')
      if (!el || running.has(el) || (e.relatedTarget instanceof Node && el.contains(e.relatedTarget))) return
      const node = firstText(el)
      if (!node) return
      const stop = scramble(node)
      running.set(el, stop)
      window.setTimeout(() => running.delete(el), SCRAMBLE_MS + 50)
    }
    document.addEventListener('pointerover', onOver)

    return () => {
      io.disconnect()
      mo.disconnect()
      cancelAnimationFrame(queued)
      document.removeEventListener('pointermove', onMove)
      document.removeEventListener('pointerover', onOver)
    }
  }, [])

  return null
}
