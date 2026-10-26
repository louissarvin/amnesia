import { describe, expect, it } from 'vitest'
import { ApiError, errorCopy } from './api'

describe('errorCopy', () => {
  it.each(['cooldown', 'seizure_in_progress', 'donation_cap_reached', 'downloads_busy'])('maps %s', (code) => {
    expect(errorCopy(new ApiError(429, code))).not.toMatch(/could not start/)
  })
  it('falls back for unknown codes and network errors', () => {
    expect(errorCopy(new ApiError(500, 'x'))).toMatch(/could not start/)
    expect(errorCopy(new TypeError('fetch'))).toMatch(/unreachable/)
  })
})
