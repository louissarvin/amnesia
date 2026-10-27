import { Link } from '@tanstack/react-router'
import { Moon, Sun } from 'lucide-react'
import { config } from '@/config'
import { useTheme } from '@/providers/ThemeProvider'

export default function SiteHeader() {
  const { theme, toggleTheme } = useTheme()

  return (
    <header className="site-nav pointer-events-none fixed inset-x-0 top-3 z-40 flex justify-center px-3 sm:top-5">
      <div className="pointer-events-auto flex h-12 items-center gap-3 rounded-full border border-line-strong bg-glass py-1 pr-1.5 pl-4 backdrop-blur-xl sm:gap-6 sm:pl-5">
        <Link to="/" className="text-[15px] font-semibold tracking-[-0.04em]">
          amnesia
        </Link>
        <nav aria-label="Primary" className="flex items-center gap-3 font-mono text-[11px] tracking-[0.06em] uppercase sm:gap-5">
          {config.nav.map((item) => (
            <Link
              key={item.to}
              to={item.to}
              data-scramble
              className="text-muted transition-colors duration-200 hover:text-fg"
              activeProps={{ className: 'text-fg' }}
            >
              {item.label}
            </Link>
          ))}
        </nav>
        <button
          type="button"
          onClick={toggleTheme}
          aria-label={theme === 'dark' ? 'Switch to paper theme' : 'Switch to ink theme'}
          className="flex size-9 items-center justify-center rounded-full border border-line text-muted transition-colors duration-200 hover:text-fg"
        >
          {theme === 'dark' ? <Sun className="size-4" /> : <Moon className="size-4" />}
        </button>
      </div>
    </header>
  )
}
