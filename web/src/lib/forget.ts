import type { Summary } from '@/lib/api'
import { config } from '@/config'

/**
 * When the desk should have destroyed the keys for an address valid until `validUntil`.
 * Mirrors desk phases: sweep wait grace/4..grace (give-up at +grace), then burial_blocks.
 * The deadline is floored to the hour, hence the extra hour on the late side.
 */
export function shredWindow(validUntil: number, s: Pick<Summary, 'grace_s' | 'burial_blocks'>) {
  const burial = s.burial_blocks * config.blockSeconds
  return { from: validUntil + s.grace_s / 4 + burial, to: validUntil + 3600 + 2 * s.grace_s + burial }
}
