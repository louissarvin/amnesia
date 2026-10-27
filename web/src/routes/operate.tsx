import { createFileRoute, Link } from '@tanstack/react-router'
import { useState } from 'react'
import PageHeader from '@/components/PageHeader'
import Eyebrow from '@/components/Eyebrow'
import { config } from '@/config'

export const Route = createFileRoute('/operate')({
  component: OperatePage,
})

const FOR = [
  ['Newsrooms', 'Readers who fund investigations should not end up in a subpoenaed spreadsheet.'],
  ['Legal defense funds', 'Supporters of a defendant are exactly who an opponent wants to name.'],
  ['Activists and NGOs', 'A seized laptop or server should not become a membership list.'],
] as const

// Mirrors infra/README.md. The lab is left out: a real deployment has no attacker built in.
const STEPS = [
  {
    title: 'Get a Linux host',
    body: 'Debian 13 with Docker. The reference is a 4 GB VPS. infra/vps/bootstrap.sh turns swap off, disables core dumps and hibernation, and locks ptrace.',
    code: 'git clone https://github.com/louissarvin/amnesia.git && cd amnesia\nsudo ./infra/vps/bootstrap.sh && sudo reboot',
  },
  {
    title: 'Point it at your treasury',
    body: 'Sweeps go to a unified address you control. Keep its spending key off this host: the desk never needs it.',
    code: 'cp infra/env/desk.env.example infra/env/desk.env\nchmod 600 infra/env/desk.env\n# set TREASURY_UA=<your unified address>',
  },
  {
    title: 'Pick the profile',
    body: 'In infra/compose.yaml set AMNESIA_PROFILE to prod: 12 h epochs, 12 h grace, up to 500 addresses each. The public demo runs demo, with 2 h epochs, so judges can watch a shred.',
    code: 'AMNESIA_PROFILE: prod',
  },
  {
    title: 'Start the desk and its onion',
    body: 'Only two services. Nothing is published to the internet; donors reach the desk through Tor alone.',
    code: 'docker compose -f infra/compose.yaml up -d --build desk tor\ndocker compose -f infra/compose.yaml exec tor cat /var/lib/tor/desk/hostname',
  },
]

function Copy({ text, label = 'Copy' }: { text: string; label?: string }) {
  const [state, setState] = useState<'idle' | 'copied' | 'failed'>('idle')
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text)
      setState('copied')
    } catch {
      setState('failed')
    }
    window.setTimeout(() => setState('idle'), 1600)
  }
  return (
    <button type="button" onClick={copy} className="pill shrink-0 px-3 py-2 text-[11px]">
      {state === 'copied' ? 'Copied' : state === 'failed' ? 'Copy failed' : label}
    </button>
  )
}

function Code({ children }: { children: string }) {
  return (
    <div className="flex items-start gap-3 rounded-2xl border border-line p-3 pl-4">
      <pre className="min-w-0 flex-1 overflow-x-auto font-mono text-xs leading-relaxed whitespace-pre">{children}</pre>
      <Copy text={children} />
    </div>
  )
}

function OperatePage() {
  const snippet = `<a href="${config.onionUrl}" rel="noreferrer">Donate privately over Tor</a>\n<!-- Open in Tor Browser. The desk forgets who paid. -->`

  return (
    <>
      <PageHeader eyebrow="for organisations" title={<>Run your own desk. <span className="text-muted">Keep no list.</span></>}>
        Amnesia is built to be run by the people who receive donations, on a host they control. This is the real
        setup, the same code that runs this demo, minus the lab that attacks it.
      </PageHeader>

      <section className="cell grid grid-cols-1 gap-px bg-line md:grid-cols-3">
        {FOR.map(([who, why]) => (
          <div key={who} className="flex min-w-0 flex-col gap-2 bg-bg px-5 py-8 md:px-8">
            <p className="text-lg font-medium tracking-[-0.02em]">{who}</p>
            <p className="text-sm text-muted">{why}</p>
          </div>
        ))}
      </section>

      <section className="cell px-5 py-16 md:px-10 md:py-20">
        <Eyebrow>setup, four steps</Eyebrow>
        <ol className="mt-8 border-t border-line">
          {STEPS.map((s, i) => (
            <li key={s.title} className="grid grid-cols-1 gap-4 border-b border-line py-8 md:grid-cols-[3.5rem_minmax(0,1fr)_minmax(0,1.3fr)] md:gap-8">
              <span className="font-mono text-xs text-muted tabular-nums">{String(i + 1).padStart(2, '0')}</span>
              <div className="min-w-0">
                <p className="text-lg font-medium tracking-[-0.02em]">{s.title}</p>
                <p className="mt-2 text-sm text-muted">{s.body}</p>
              </div>
              <div className="min-w-0">
                <Code>{s.code}</Code>
              </div>
            </li>
          ))}
        </ol>
        <p className="mt-6 max-w-2xl text-xs text-muted">
          Full details, hardening table and caveats: infra/README.md and SECURITY.md in the repository.
        </p>
      </section>

      <section className="cell grid grid-cols-1 gap-10 px-5 py-16 md:grid-cols-2 md:px-10 md:py-20">
        <div className="min-w-0">
          <Eyebrow>put it on your site</Eyebrow>
          <h2 className="mt-5 text-2xl font-medium tracking-[-0.02em]">One link. No script, no tracker.</h2>
          <p className="mt-3 max-w-md text-muted">
            Your site only links to the onion. Nothing on your page talks to the desk, so your web host never sees a
            donor. This is the link for this demo desk; yours comes from step 4.
          </p>
        </div>
        <div className="min-w-0">
          <Code>{snippet}</Code>
        </div>
      </section>

      <section className="cell px-5 py-16 md:px-10 md:py-20">
        <Eyebrow>before you rely on it</Eyebrow>
        <ul className="mt-6 grid max-w-3xl gap-3 text-sm text-muted">
          <li>
            This demo runs on a Mac with launchd (infra/mac). The Linux setup above is the hardened design and has been
            tested in containers, not yet run as a public desk.
          </li>
          <li>Testnet only today. Mainnet needs the fee and expiry rules updated for NU7 (ZIP 2009, ZIP 203).</li>
          <li>One host holds every live epoch. A compromise while an epoch is live exposes that epoch.</li>
          <li>The shred log is self-reported. There is no hardware attestation yet.</li>
          <li>
            Each payment costs network fees to forward, merge and sweep. See the live numbers on{' '}
            <Link to="/verify" className="underline underline-offset-4">
              /verify
            </Link>
            .
          </li>
        </ul>
      </section>
    </>
  )
}
