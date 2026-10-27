import { useEffect, useRef } from 'react'

const TILE = 192

// Static film grain: a tiny canvas painted once and exported as a data: URL (blob: would break img-src 'self' data:).
export default function Noise() {
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const canvas = document.createElement('canvas')
    canvas.width = canvas.height = TILE
    const ctx = canvas.getContext('2d')
    if (!ctx || !ref.current) return
    const image = ctx.createImageData(TILE, TILE)
    for (let i = 0; i < image.data.length; i += 4) {
      const v = Math.random() * 255
      image.data[i] = image.data[i + 1] = image.data[i + 2] = v
      image.data[i + 3] = 255
    }
    ctx.putImageData(image, 0, 0)
    ref.current.style.backgroundImage = `url("${canvas.toDataURL('image/png')}")`
  }, [])

  return (
    <div
      ref={ref}
      aria-hidden="true"
      className="pointer-events-none fixed inset-0 z-50 bg-repeat opacity-[0.045] mix-blend-soft-light"
    />
  )
}
