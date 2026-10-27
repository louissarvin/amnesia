import { createRouter } from '@tanstack/react-router'
import * as TanstackQuery from './integrations/tanstack-query/root-provider'
import { routeTree } from './routeTree.gen'

const motionOk = () => typeof window !== 'undefined' && !window.matchMedia('(prefers-reduced-motion: reduce)').matches

export const getRouter = () => {
  const { queryClient } = TanstackQuery.getContext()

  return createRouter({
    routeTree,
    context: { queryClient },
    defaultPreload: 'intent',
    scrollRestoration: true,
    defaultViewTransition: motionOk(),
    Wrap: ({ children }) => (
      <TanstackQuery.Provider queryClient={queryClient}>{children}</TanstackQuery.Provider>
    ),
  })
}
