const ZATS_PER_TAZ = 100_000_000

const trim = (n: number, digits: number) => n.toFixed(digits).replace(/\.?0+$/, '')

export const formatTaz = (zats: number | string): string => {
  const taz = Number(zats) / ZATS_PER_TAZ
  return Number.isFinite(taz) ? `${trim(taz, 8)} TAZ` : '-'
}

const utc = new Intl.DateTimeFormat('en-GB', {
  dateStyle: 'medium',
  timeStyle: 'short',
  timeZone: 'UTC',
})
const utcDay = new Intl.DateTimeFormat('en-GB', { dateStyle: 'medium', timeZone: 'UTC' })

export const formatUtc = (unixSeconds: number): string => `${utc.format(unixSeconds * 1000)} UTC`
export const formatUtcDay = (unixSeconds: number): string => utcDay.format(unixSeconds * 1000)

export const formatDuration = (seconds: number): string => {
  const hours = seconds / 3600
  return hours >= 1 ? `${trim(hours, 1)} h` : `${Math.round(seconds / 60)} min`
}

export const shortId = (id: string, head = 8, tail = 4): string =>
  id.length <= head + tail + 1 ? id : `${id.slice(0, head)}…${id.slice(-tail)}`

export const formatBytes = (bytes: number): string => {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(1)} MB`
  if (bytes >= 1e3) return `${(bytes / 1e3).toFixed(0)} KB`
  return `${bytes} B`
}

const utcTime = new Intl.DateTimeFormat('en-GB', { timeStyle: 'short', timeZone: 'UTC' })
export const formatUtcTime = (unixSeconds: number): string => `${utcTime.format(unixSeconds * 1000)} UTC`
