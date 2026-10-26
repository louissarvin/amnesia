export const config = {
  appName: 'Amnesia',
  appDescription:
    'A Zcash testnet donation desk that forgets its donors. Seize it in the lab and see who can be named.',
  // zcashexplorer.app stayed on the pre-NU7 testnet chain (75 s blocks) and cannot see current txs.
  explorerTxUrl: 'https://blockexplorer.one/zcash/testnet/tx/',
  // The desk's real intake. Donors open it in Tor Browser; it is never served on the clearnet.
  onionUrl: 'http://3ifmcl72upgshv4nbcum4qzwk32whg6vrditxqgzhmfuwttw67slxoqd.onion',
  blockSeconds: 25,
  nav: [
    { to: '/seize', label: 'Seize' },
    { to: '/donate', label: 'Donate' },
    { to: '/audit', label: 'Audit' },
    { to: '/verify', label: 'Verify' },
  ],
} as const
