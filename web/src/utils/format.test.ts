import { describe, expect, it } from 'vitest'
import { formatDuration, formatTaz, shortId } from './format'

describe('format', () => {
  it('formats zats as TAZ without trailing zeros', () => {
    expect(formatTaz(400000)).toBe('0.004 TAZ')
    expect(formatTaz('5000000')).toBe('0.05 TAZ')
  })
  it('formats durations', () => {
    expect(formatDuration(7200)).toBe('2 h')
    expect(formatDuration(15000)).toBe('4.2 h')
    expect(formatDuration(900)).toBe('15 min')
  })
  it('shortens ids', () => {
    expect(shortId('0123456789abcdef0123')).toBe('01234567…0123')
  })
})
