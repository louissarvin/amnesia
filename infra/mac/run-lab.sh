#!/bin/sh
# Lab daemon on the Mac host. Managed by launchd: com.amnesia.lab
cd /Users/macbookair/amnesia-host
# Seizures copy live epochs: keep them on a RAM disk, never the SSD (var/lab/seizures links there).
sh var/ramdisk.sh AmnesiaSeize 524288 || exit 1
mkdir -p /Volumes/AmnesiaSeize/seizures && chmod 700 /Volumes/AmnesiaSeize/seizures
export AMNESIA_LAB_ROOT=var/lab AMNESIA_LAB_ADDR=127.0.0.1:8090 AMNESIA_DESK_INTAKE=127.0.0.1:8081
export AMNESIA_SEIZE_SOURCES=var/desk,/Volumes/AmnesiaRAM/epochs AMNESIA_LAB_DONATIONS_PER_H=1 AMNESIA_LAB_DECOYS_PER_H=2 AMNESIA_FAUCET=1
export AMNESIA_WEB_DIR=/Users/macbookair/amnesia-host/web
exec /Users/macbookair/amnesia-host/bin/amnesia-lab serve >> var/lab.log 2>&1
