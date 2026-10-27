import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import type { YourDonation } from '@/lib/api'
import { lastDonateRun, runQuery, storedDonation } from '@/lib/api'

export type { YourDonation }

// The judge's own donation: this device's record first, else this tab's live run.
export function useYourDonation(): YourDonation | null {
  const [stored] = useState(storedDonation)
  const [runId] = useState(lastDonateRun)
  const run = useQuery({ ...runQuery(runId ?? ''), enabled: !stored && runId !== null })
  if (stored) return stored
  const issued = run.data?.events.find((e) => e.kind === 'issued')
  return issued?.kind === 'issued'
    ? { client: issued.detail.client, issuedAt: issued.t, validUntil: issued.detail.valid_until }
    : null
}
