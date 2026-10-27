import { HeadContent, Outlet, Scripts, createRootRouteWithContext } from '@tanstack/react-router'
import { MotionConfig } from 'motion/react'
import geistLatin from '@fontsource-variable/geist/files/geist-latin-wght-normal.woff2?url'
import geistMonoLatin from '@fontsource-variable/geist-mono/files/geist-mono-latin-wght-normal.woff2?url'
import appCss from '../styles.css?url'
import type { QueryClient } from '@tanstack/react-query'
import ErrorPage from '@/components/ErrorPage'
import MotionRuntime from '@/components/MotionRuntime'
import Noise from '@/components/reactbits/Noise'
import SiteFooter from '@/components/SiteFooter'
import SiteHeader from '@/components/SiteHeader'
import { config } from '@/config'
import LenisSmoothScrollProvider from '@/providers/LenisSmoothScrollProvider'
import { ThemeProvider } from '@/providers/ThemeProvider'

export const Route = createRootRouteWithContext<{ queryClient: QueryClient }>()({
  errorComponent: ({ error, reset }) => <ErrorPage error={error} reset={reset} />,
  head: () => ({
    meta: [
      { charSet: 'utf-8' },
      { name: 'viewport', content: 'width=device-width, initial-scale=1' },
      { title: config.appName },
      { name: 'description', content: config.appDescription },
      { name: 'theme-color', content: '#07080a' },
      // Link previews for judges who share the demo URL. Absolute image URL, as the OG spec requires.
      { property: 'og:type', content: 'website' },
      { property: 'og:title', content: 'Amnesia: a donation desk that forgets' },
      { property: 'og:description', content: config.appDescription },
      { property: 'og:url', content: 'https://amnesia.taila8827e.ts.net/' },
      { property: 'og:image', content: 'https://amnesia.taila8827e.ts.net/og.png' },
      { property: 'og:image:width', content: '1200' },
      { property: 'og:image:height', content: '630' },
      { name: 'twitter:card', content: 'summary_large_image' },
      { name: 'twitter:title', content: 'Amnesia: a donation desk that forgets' },
      { name: 'twitter:description', content: config.appDescription },
      { name: 'twitter:image', content: 'https://amnesia.taila8827e.ts.net/og.png' },
    ],
    links: [
      { rel: 'preload', href: geistLatin, as: 'font', type: 'font/woff2', crossOrigin: 'anonymous' },
      { rel: 'preload', href: geistMonoLatin, as: 'font', type: 'font/woff2', crossOrigin: 'anonymous' },
      { rel: 'stylesheet', href: appCss },
      { rel: 'icon', href: '/favicon.svg', type: 'image/svg+xml' },
      { rel: 'manifest', href: '/manifest.json' },
    ],
  }),
  component: RootLayout,
  shellComponent: RootDocument,
})

function RootDocument({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" suppressHydrationWarning>
      <head>
        <script src="/theme-init.js" />
        <HeadContent />
      </head>
      <body>
        {children}
        <Scripts />
      </body>
    </html>
  )
}

function RootLayout() {
  return (
    <ThemeProvider>
      <MotionConfig reducedMotion="user">
        <LenisSmoothScrollProvider />
        <MotionRuntime />
        <Noise />
        <SiteHeader />
        {/* Cell crosshairs straddle the frame edge; clip (not hidden) keeps sticky working. */}
        <div className="overflow-x-clip">
          <div className="frame mx-auto max-w-[1360px]">
            {/* At least one screen tall, so the footer never paints above the fold before a route renders (CLS). */}
            <main className="min-h-svh">
              <Outlet />
            </main>
            <SiteFooter />
          </div>
        </div>
      </MotionConfig>
    </ThemeProvider>
  )
}
