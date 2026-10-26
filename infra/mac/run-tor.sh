#!/bin/sh
# Native tor for the donor onion. Managed by launchd: com.amnesia.tor
cd /Users/macbookair/amnesia-host || exit 1
# Tor relays the issued-address page: scrub freed heap like the desk (malloc(3)).
export MallocScribble=1
exec /Users/macbookair/amnesia-host/tor/bin/tor -f var/tor/torrc >> var/tor/tor.log 2>&1
