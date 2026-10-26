#!/bin/sh
# Builds the judge UI, precompresses text assets for the lab's gzip path, swaps it in, restarts the lab.
set -eu
REPO=$(cd "$(dirname "$0")/../.." && pwd)
HOST=/Users/macbookair/amnesia-host
cd "$REPO/web"
bun run build
find dist/client -type f \( -name '*.js' -o -name '*.css' -o -name '*.html' -o -name '*.svg' -o -name '*.json' -o -name '*.txt' \) -size +1k -exec gzip -9 -k -f {} \;
rm -rf "${HOST:?}/web.new"
cp -R dist/client "$HOST/web.new"
[ -d "$HOST/web" ] && mv "$HOST/web" "$HOST/web.old"
mv "$HOST/web.new" "$HOST/web"
rm -rf "${HOST:?}/web.old"
launchctl kickstart -k "gui/$(id -u)/com.amnesia.lab"
