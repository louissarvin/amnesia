import { Link } from '@tanstack/react-router'
import Eyebrow from '@/components/Eyebrow'

interface ErrorPageProps {
  error?: Error
  reset?: () => void
}

export default function ErrorPage({ reset }: ErrorPageProps) {
  return (
    <div className="mx-auto flex min-h-screen max-w-xl flex-col justify-center gap-6 px-5">
      <Eyebrow>something broke</Eyebrow>
      <h1 className="display text-5xl">This page failed to load.</h1>
      <p className="text-muted">Nothing was sent or stored. Reload, or go back to the start.</p>
      <div className="flex gap-3">
        {reset ? (
          <button type="button" onClick={reset} className="pill">
            Try again
          </button>
        ) : null}
        <Link to="/" className="pill pill-solid">
          Home
        </Link>
      </div>
    </div>
  )
}
