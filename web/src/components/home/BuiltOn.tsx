import Eyebrow from '@/components/Eyebrow'

const STACK = [
  { name: 'Zcash', style: 'text-2xl font-semibold tracking-[-0.04em]' },
  { name: 'Orchard / Ironwood', style: 'text-xl font-medium tracking-[-0.03em]' },
  { name: 'Tor', style: 'text-2xl font-bold tracking-[-0.05em]' },
  { name: 'librustzcash', style: 'font-mono text-base' },
  { name: 'lightwalletd (zec.rocks)', style: 'font-mono text-base' },
  { name: 'arti', style: 'font-mono text-lg font-semibold' },
]

export default function BuiltOn() {
  return (
    <section className="cell px-5 py-10 md:px-10 md:py-12">
      <Eyebrow>built on</Eyebrow>
      <ul className="mt-6 flex flex-wrap items-center gap-x-10 gap-y-5">
        {STACK.map((item) => (
          <li key={item.name} className={`text-muted transition-colors duration-200 hover:text-fg ${item.style}`}>
            {item.name}
          </li>
        ))}
      </ul>
    </section>
  )
}
