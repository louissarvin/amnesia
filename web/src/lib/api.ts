import { queryOptions } from '@tanstack/react-query'
import { z } from 'zod'

export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
  ) {
    super(code)
  }
}

const num = z.number()
const str = z.string()

const health = z.object({
  fresh: z.boolean(),
  synced: z.boolean(),
  intake: str,
  live_epochs_bucket: str,
  version: str,
})

const summary = z.object({
  burial_blocks: num,
  epoch_s: num,
  grace_s: num,
  epochs_lost: num,
  epochs_shredded: num,
  epochs_shredded_this_profile: num,
  intake: str,
  live_epochs_bucket: str,
  profile: str,
  swept_bucket: str,
})

const labStatus = z.object({
  donations: num,
  decoys: num,
  exchange_spendable_zats: num.nullable(),
  exchange_notes: num.nullable(),
  fee: num,
})

const treasury = z.object({ swept_bucket: str })

const shredRow = z.object({
  epoch_id: str,
  opened_hour: num,
  closed_hour: num,
  closed_kind: z.enum(['shredded', 'lost']),
  issued_bucket: str,
  profile: str,
})

const shredLog = z.object({ rows: z.array(shredRow), total: num, offset: num })

const info = z.object({
  binary_sha256: str,
  git_commit: str,
  network: str,
  profile: str,
  tee: str,
  version: str,
  build: z.object({ cargo_lock_sha256: str, reproduce: str, toolchain: str }),
  params: z.record(str, z.union([num, str])),
})

const auditWeeks = z.object({
  weeks: z.array(z.object({ epochs_forgotten: num, total_2sf_zats: str, week_start: num })),
})

const seizureSummary = z.object({
  id: str,
  target: z.enum(['amnesia', 'baseline']),
  created: num,
  bytes: num,
  files: num,
  epochs_in_image: num,
  keys_found: num,
  raw_addresses: num,
  receipts: num,
  records_total: num,
  linked: num,
  amount_sweeps: num,
  amount_pinned: num,
  amount_ambiguous: num,
})

const seizureList = z.object({ seizures: z.array(seizureSummary) })

const report = z.object({
  linked: z.array(
    z.object({ client_id: str, value_zats: num, height: num.nullable(), how: str }),
  ),
  unlinked: num,
  records_total: num,
  keyring_keys: num,
  amount_attack: z
    .object({ sweeps: num, candidates: num, pinned: num, excluded: num, ambiguous: num })
    .nullable(),
})

const empty = z.object({})
const runEvent = z.discriminatedUnion('kind', [
  z.object({
    kind: z.literal('issued'),
    t: num,
    detail: z.object({
      client: str,
      value_zats: num,
      amnesia_address: str,
      valid_until: num.optional(),
      receipt: str.regex(/^[0-9a-f]{32}$/).nullable().optional(),
    }),
  }),
  z.object({
    kind: z.literal('broadcast'),
    t: num,
    detail: z.object({ amnesia_txid: str, baseline_txid: str }),
  }),
  z.object({
    kind: z.literal('mined'),
    t: num,
    detail: z.object({ amnesia_height: num, baseline_height: num }),
  }),
  z.object({ kind: z.literal('float_wait'), t: num, detail: empty }),
  z.object({ kind: z.literal('bundle_ready'), t: num, detail: empty }),
  z.object({ kind: z.literal('scanning'), t: num, detail: empty }),
  z.object({ kind: z.literal('report'), t: num, detail: seizureSummary }),
])
const run = z.object({
  id: str,
  kind: z.enum(['donate', 'seize']),
  created: num.optional(),
  done: z.boolean(),
  error: str.nullable(),
  events: z.array(runEvent),
})

const runStart = z.object({ run_id: str.regex(/^[0-9a-f]{16}$/) })

export type Summary = z.infer<typeof summary>
export type LabStatus = z.infer<typeof labStatus>
export type ShredRow = z.infer<typeof shredRow>
export type SeizureSummary = z.infer<typeof seizureSummary>
export type Report = z.infer<typeof report>
export type Run = z.infer<typeof run>
export type RunEvent = z.infer<typeof runEvent>
export type SeizeTarget = SeizureSummary['target']

async function readError(res: Response): Promise<ApiError> {
  let code = 'unavailable'
  try {
    const body: unknown = await res.json()
    const parsed = z.object({ error: str }).safeParse(body)
    if (parsed.success) code = parsed.data.error
  } catch {
    // non-JSON error body, keep the generic code
  }
  return new ApiError(res.status, code)
}

async function get<TSchema extends z.ZodType>(path: string, schema: TSchema): Promise<z.infer<TSchema>> {
  const res = await fetch(path, { headers: { accept: 'application/json' } })
  if (!res.ok) throw await readError(res)
  return schema.parse(await res.json())
}

async function post(path: string, body?: object): Promise<z.infer<typeof runStart>> {
  const res = await fetch(path, {
    method: 'POST',
    headers: {
      'X-Amnesia-Demo': '1',
      ...(body ? { 'content-type': 'application/json' } : {}),
    },
    body: body ? JSON.stringify(body) : undefined,
  })
  if (!res.ok) throw await readError(res)
  return runStart.parse(await res.json())
}

export const startDonation = () => post('/demo/donate')
export const startSeizure = (target: SeizeTarget) => post('/demo/seize', { target })

const COPY: Record<string, string> = {
  cooldown: 'The lab paces runs so the demo stays fair. Give it a few seconds and try again.',
  seizure_in_progress: 'Another seizure is running right now. Wait for it to finish, then try again.',
  donation_cap_reached:
    'The lab has reached its donation limit for this hour, to protect the testnet float. Seizures still work.',
  downloads_busy: 'Too many bundle downloads are running. Try again shortly.',
}

export const errorCopy = (error: unknown): string => {
  if (error instanceof ApiError) {
    return COPY[error.code] ?? 'The lab could not start this run. Try again in a moment.'
  }
  return 'The lab is unreachable right now. Try again in a moment.'
}

export const healthQuery = queryOptions({
  queryKey: ['desk', 'health'],
  queryFn: () => get('/v1/health', health),
  refetchInterval: 30_000,
})

export const summaryQuery = queryOptions({
  queryKey: ['desk', 'summary'],
  queryFn: () => get('/v1/summary', summary),
  refetchInterval: 30_000,
})

export const labStatusQuery = queryOptions({
  queryKey: ['lab', 'status'],
  queryFn: () => get('/demo/status', labStatus),
  refetchInterval: 30_000,
})

export const treasuryQuery = queryOptions({
  queryKey: ['desk', 'treasury'],
  queryFn: () => get('/v1/treasury', treasury),
  refetchInterval: 60_000,
})

export const infoQuery = queryOptions({
  queryKey: ['desk', 'info'],
  queryFn: () => get('/v1/info', info),
  staleTime: 5 * 60_000,
})

export const auditWeeksQuery = queryOptions({
  queryKey: ['desk', 'audit-weeks'],
  queryFn: () => get('/v1/audit/weeks', auditWeeks),
})

export const shredLogQuery = (limit: number, offset: number) =>
  queryOptions({
    queryKey: ['desk', 'shred-log', limit, offset],
    queryFn: () =>
      get(`/v1/shred-log?${new URLSearchParams({ limit: String(limit), offset: String(offset) })}`, shredLog),
  })

export const historyQuery = queryOptions({
  queryKey: ['lab', 'history'],
  queryFn: () => get('/demo/history', seizureList),
  refetchInterval: 30_000,
})

export const seizuresQuery = queryOptions({
  queryKey: ['lab', 'seizures'],
  queryFn: () => get('/demo/seizures', seizureList),
})

export const reportQuery = (id: string) =>
  queryOptions({
    queryKey: ['lab', 'report', id],
    queryFn: () => get(`/demo/seizures/${encodeURIComponent(id)}/report.json`, report),
    staleTime: Infinity,
  })

export const runQuery = (id: string) =>
  queryOptions({
    queryKey: ['lab', 'run', id],
    queryFn: () => get(`/demo/runs/${encodeURIComponent(id)}`, run),
    refetchInterval: (query) => (query.state.data?.done ? false : 2000),
    retry: false,
  })

const RUN_KEY = 'amnesia:donate-run'
const YOURS_KEY = 'amnesia:your-donation'

const yours = z.object({
  client: str.regex(/^you-[0-9a-f]{4}$/),
  issuedAt: num,
  validUntil: num.optional(),
  receipt: str.regex(/^[0-9a-f]{32}$/).optional(),
})
export type YourDonation = z.infer<typeof yours>

// Kept on this device only, so a judge who returns after the shred can see the donation is gone.
export const storedDonation = (): YourDonation | null => {
  try {
    const parsed = yours.safeParse(JSON.parse(localStorage.getItem(YOURS_KEY) ?? 'null'))
    return parsed.success ? parsed.data : null
  } catch {
    return null
  }
}

export const rememberDonation = (d: YourDonation) => {
  try {
    localStorage.setItem(YOURS_KEY, JSON.stringify(d))
  } catch {
    // storage unavailable, /seize falls back to this tab's run
  }
}

export const lastDonateRun = (): string | null => {
  try {
    const id = sessionStorage.getItem(RUN_KEY)
    return id && /^[0-9a-f]{16}$/.test(id) ? id : null
  } catch {
    return null
  }
}

export const rememberDonateRun = (id: string | null) => {
  try {
    if (id) sessionStorage.setItem(RUN_KEY, id)
    else sessionStorage.removeItem(RUN_KEY)
  } catch {
    // storage unavailable, the run just will not survive a reload
  }
}
