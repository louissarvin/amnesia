#!/bin/sh
# Desk on the Mac host (Testnet, demo profile). Managed by launchd: com.amnesia.desk
cd /Users/macbookair/amnesia-host || exit 1
# Refuse to run with epochs on the SSD.
sh var/ramdisk.sh || exit 1
export TREASURY_UA="$(cat var/lab/treasury.ua)"
export AMNESIA_PROFILE=demo AMNESIA_TOR=1 AMNESIA_POW_BITS=16 AMNESIA_ROOT=var/desk
export AMNESIA_EPOCHS_DIR=/Volumes/AmnesiaRAM/epochs
# libmalloc fills freed blocks with 0x55 (malloc(3)): covers SQLite's own heap, which the
# desk's zero-on-free allocator does not see.
export MallocScribble=1
exec /Users/macbookair/amnesia-host/bin/amnesia-desk >> var/desk/log/desk.log 2>&1
