import { useQuery } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import Eyebrow from '@/components/Eyebrow'
import { healthQuery } from '@/lib/api'

export default function SiteFooter() {
  const { data, isError } = useQuery(healthQuery)

  const status = data
    ? data.intake === 'open'
      ? 'live / testnet / synced / intake open'
      : 'testnet / desk catching up with the chain'
    : isError
      ? 'testnet / desk unavailable'
      : 'testnet / checking desk'

  return (
    <footer className="cell">
      <div className="flex flex-col gap-4 px-5 py-8 md:flex-row md:items-center md:justify-between md:px-10">
        <Eyebrow live={data?.intake === 'open'}>{status}</Eyebrow>
        <p className="font-mono text-[11px] tracking-[0.04em] text-muted uppercase">
          Zcash testnet only. No analytics, no third-party requests.{' '}
          <Link to="/verify" className="underline underline-offset-4 hover:text-fg">
            Check for yourself
          </Link>
          {' · '}
          <Link to="/operate" className="underline underline-offset-4 hover:text-fg">
            Run your own
          </Link>
        </p>
      </div>
    </footer>
  )
}
