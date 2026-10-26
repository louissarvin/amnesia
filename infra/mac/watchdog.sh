#!/bin/sh
# Host watchdog, every 5 min via launchd com.amnesia.watchdog. Alerts the operator and
# restarts a service only when its HTTP endpoint stops answering.
cd /Users/macbookair/amnesia-host || exit 1
LOG=var/watchdog.log
UID_=$(id -u)
ts=$(date -u +%Y-%m-%dT%H:%M:%SZ)

alert() {
  printf '{"ts":"%s","kind":"%s"}\n' "$ts" "$1" >> "$LOG"
  osascript -e "display notification \"$1\" with title \"Amnesia watchdog\"" >/dev/null 2>&1
}

health=$(curl -s -m 10 127.0.0.1:8082/v1/health)
if [ -z "$health" ]; then
  alert desk_unreachable_restarted
  launchctl kickstart -k "gui/$UID_/com.amnesia.desk"
elif ! printf %s "$health" | grep -q '"fresh":true'; then
  alert desk_stale
fi

status=$(curl -s -m 10 127.0.0.1:8090/demo/status)
if [ -z "$status" ]; then
  alert lab_unreachable_restarted
  launchctl kickstart -k "gui/$UID_/com.amnesia.lab"
else
  zats=$(printf %s "$status" | sed -n 's/.*"exchange_spendable_zats":\([0-9]*\).*/\1/p')
  [ -n "$zats" ] && [ "$zats" -lt 1500000 ] && alert lab_float_low
fi

# Donors arrive only through the onion: tor down means intake is unreachable.
if ! launchctl list com.amnesia.tor 2>/dev/null | grep -q '"PID"'; then
  alert tor_down_restarted
  launchctl kickstart -k "gui/$UID_/com.amnesia.tor"
fi

# Judges reach /v1 and /demo only through Tailscale Funnel.
TS=/Applications/Tailscale.app/Contents/MacOS/Tailscale
if [ -x "$TS" ] && ! "$TS" funnel status 2>/dev/null | grep -q '/demo'; then
  alert funnel_down
fi

slow=$(tail -n 300 var/desk/log/desk.log 2>/dev/null | grep -c slow_tick)
[ "$slow" -gt 5 ] && alert desk_slow_ticks
pmset -g batt | grep -q "AC Power" || alert on_battery_will_sleep
free_kb=$(df -k /Users | awk 'NR==2 {print $4}')
[ "$free_kb" -lt 5242880 ] && alert disk_below_5gb
ram_kb=$(df -k /Volumes/AmnesiaRAM 2>/dev/null | awk 'NR==2 {print $4}')
if [ -z "$ram_kb" ]; then alert ramdisk_missing; elif [ "$ram_kb" -lt 32768 ]; then alert ramdisk_below_32mb; fi

# Desk RSS above 1.5 GB over a 25-day run means a leak: restart in time, not at OOM.
rss_kb=$(ps -o rss= -p "$(pgrep -f amnesia-host/bin/amnesia-desk | head -1)" 2>/dev/null | tr -d ' ')
[ -n "$rss_kb" ] && [ "$rss_kb" -gt 1572864 ] && alert desk_rss_high

# Logs grow for weeks of judging: trim any above 20 MB to its last 50,000 lines.
for f in var/lab.log var/desk/log/desk.log var/tor/tor.log; do
  if [ -f "$f" ] && [ "$(stat -f %z "$f")" -gt 20971520 ]; then
    tail -n 50000 "$f" > "$f.tmp" && cat "$f.tmp" > "$f" && rm -f "$f.tmp"
  fi
done

# Keep the log small: last 2,000 lines.
tail -n 2000 "$LOG" > "$LOG.tmp" 2>/dev/null && mv "$LOG.tmp" "$LOG"
exit 0
