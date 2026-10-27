import { useEffect, useState } from 'react'
import { cnm } from '@/utils/style'

type Phase = 'shown' | 'dissolving' | 'gone'

// Shows a one-time address briefly, then removes it from the DOM entirely.
export default function ForgetAddress({ address, issuedAt }: { address: string; issuedAt: number }) {
  const [phase, setPhase] = useState<Phase>(() => (Date.now() / 1000 - issuedAt > 60 ? 'gone' : 'shown'))

  useEffect(() => {
    if (phase !== 'shown') return
    const t = window.setTimeout(() => setPhase('dissolving'), 6000)
    return () => window.clearTimeout(t)
  }, [phase])

  if (phase === 'gone') {
    return <p className="font-mono text-sm text-muted">address forgotten</p>
  }

  return (
    <p
      onAnimationEnd={() => setPhase('gone')}
      className={cnm('break-all font-mono text-sm leading-6', phase === 'dissolving' && 'forget')}
    >
      {address}
    </p>
  )
}
