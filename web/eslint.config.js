//  @ts-check

import { tanstackConfig } from '@tanstack/eslint-config'

export default [
  { ignores: ['dist/**', '.output/**', 'src/routeTree.gen.ts', 'src/components/reactbits/**', 'public/**', '*.config.js'] },
  ...tanstackConfig,
]
