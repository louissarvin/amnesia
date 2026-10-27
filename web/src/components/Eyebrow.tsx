import { cnm } from '@/utils/style'

interface EyebrowProps {
  children: React.ReactNode
  live?: boolean
  className?: string
}

export default function Eyebrow({ children, live, className }: EyebrowProps) {
  return (
    <p data-scramble className={cnm('flex w-fit items-center gap-2 font-mono text-[11px] uppercase tracking-[0.08em] text-muted', className)}>
      {live ? <span aria-hidden="true" className="live-dot size-1.5 rounded-full bg-gold" /> : null}
      {children}
    </p>
  )
}
