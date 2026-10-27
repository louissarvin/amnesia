import { useQuery } from '@tanstack/react-query'
import { shredLogQuery } from '@/lib/api'

/**
 * Looks the receipt up in the public shred log, entirely in the browser: the desk never learns
 * which epoch anyone is checking. Returns the row once shredded, null before, undefined while loading.
 */
export function useShredReceipt(receipt: string | undefined) {
  const log = useQuery({ ...shredLogQuery(1000, 0), enabled: Boolean(receipt), refetchInterval: 60_000 })
  if (!receipt || !log.data) return undefined
  return log.data.rows.find((r) => r.epoch_id === receipt) ?? null
}
