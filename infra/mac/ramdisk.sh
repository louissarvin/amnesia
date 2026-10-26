#!/bin/sh
# Ensures the epoch RAM disk is mounted. ram:// images attach in-kernel with wired memory
# (hdiutil(1)), so epoch files never reach the SSD or swap. A reboot loses them: the desk
# then reports those epochs as lost, like tmpfs on the VPS.
# Usage: ramdisk.sh [NAME] [SECTORS]. Defaults: the epoch disk, 256 MB (512-byte sectors).
NAME=${1:-AmnesiaRAM}
SECTORS=${2:-524288}
MP=/Volumes/$NAME

is_ram() {
  dev=$(mount | awk -v mp="$MP" '$3 == mp {print $1}')
  [ -n "$dev" ] && hdiutil info | grep -B30 "^$dev[[:space:]]" | grep -q 'image-path *: ram://'
}

if ! is_ram; then
  if mount | grep -q " on $MP "; then
    echo "ramdisk: $MP is mounted but not a RAM disk" >&2
    exit 1
  fi
  dev=$(hdiutil attach -nomount "ram://$SECTORS" | awk '{print $1}')
  [ -n "$dev" ] || exit 1
  diskutil erasevolume HFS+ "$NAME" "$dev" >/dev/null || exit 1
fi
is_ram || { echo "ramdisk: verification failed" >&2; exit 1; }
# Spotlight would copy file contents into an index on the SSD.
touch "$MP/.metadata_never_index"
mdutil -i off "$MP" >/dev/null 2>&1
chmod 700 "$MP"
[ "$NAME" = AmnesiaRAM ] && mkdir -p "$MP/epochs" && chmod 700 "$MP/epochs"
exit 0
