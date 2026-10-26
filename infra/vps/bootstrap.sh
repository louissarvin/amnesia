#!/usr/bin/env bash
# Fresh Hetzner CX33, Debian 13 (trixie), x86_64. Run as root; safe to re-run.
# A reboot afterwards applies the GRUB cmdline (init_on_free=1).
set -euo pipefail

[[ $EUID -eq 0 ]] || { echo "run as root" >&2; exit 1; }
. /etc/os-release
[[ $ID == debian && $VERSION_CODENAME == trixie ]] || { echo "expects Debian 13 (trixie)" >&2; exit 1; }

export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y ca-certificates curl git ufw unattended-upgrades apt-listchanges

# Docker Engine from Docker's apt repo (docs.docker.com/engine/install/debian).
install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/debian/gpg -o /etc/apt/keyrings/docker.asc
chmod a+r /etc/apt/keyrings/docker.asc
cat > /etc/apt/sources.list.d/docker.sources <<EOF
Types: deb
URIs: https://download.docker.com/linux/debian
Suites: ${VERSION_CODENAME}
Components: stable
Architectures: $(dpkg --print-architecture)
Signed-By: /etc/apt/keyrings/docker.asc
EOF
apt-get update
apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
systemctl enable --now docker

# Swap off for good: the desk refuses to start (AMNESIA_STRICT=1) while any swap is active.
swapoff -a
sed -i.amnesia.bak -E '/^[^#].*\sswap\s/s/^/# amnesia: /' /etc/fstab
for unit in $(systemctl list-unit-files --type=swap --no-legend | awk '{print $1}'); do
  systemctl mask "$unit"
done
if dpkg -s systemd-zram-generator >/dev/null 2>&1; then apt-get purge -y systemd-zram-generator; fi
if dpkg -s zram-tools >/dev/null 2>&1; then apt-get purge -y zram-tools; fi

# No crash dumps, no hibernation image: both would write key material to disk.
if dpkg -s kdump-tools >/dev/null 2>&1; then apt-get purge -y kdump-tools; fi
systemctl mask sleep.target suspend.target hibernate.target hybrid-sleep.target

cat > /etc/sysctl.d/90-amnesia.conf <<'EOF'
kernel.core_pattern=|/bin/false
fs.suid_dumpable=0
kernel.yama.ptrace_scope=2
vm.swappiness=0
EOF
sysctl --system >/dev/null

# Freed pages are zeroed by the kernel; takes effect after reboot.
install -d /etc/default/grub.d
echo 'GRUB_CMDLINE_LINUX_DEFAULT="$GRUB_CMDLINE_LINUX_DEFAULT init_on_free=1"' > /etc/default/grub.d/amnesia.cfg
update-grub

# Journal in RAM only, small and short-lived.
install -d /etc/systemd/journald.conf.d
cat > /etc/systemd/journald.conf.d/90-amnesia.conf <<'EOF'
[Journal]
Storage=volatile
RuntimeMaxUse=64M
MaxRetentionSec=7day
EOF
systemctl restart systemd-journald
rm -rf --one-file-system /var/log/journal

# Inbound 22/80/443 only. Docker-published ports bypass ufw; only caddy publishes, on 80/443.
ufw default deny incoming
ufw default allow outgoing
ufw allow 22/tcp
ufw allow 80/tcp
ufw allow 443/tcp
ufw --force enable

cat > /etc/apt/apt.conf.d/20auto-upgrades <<'EOF'
APT::Periodic::Update-Package-Lists "1";
APT::Periodic::Unattended-Upgrade "1";
EOF
systemctl enable --now unattended-upgrades

install -d -m 0750 /srv/amnesia

echo "bootstrap done; reboot to apply init_on_free=1, then check: cat /proc/swaps /proc/cmdline"
