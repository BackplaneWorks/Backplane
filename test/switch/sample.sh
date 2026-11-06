#!/bin/bash
# Counts only: every N seconds, one line "epoch hub_rss_mb helper_rss_mb scope_current_mb scope_peak_mb" until the stop file exists.
# usage: sample.sh SWITCH_DIR OUT.tsv [SECONDS=5]
SW=$1; OUT=$2; N=${3:-5}
mb() { awk -v v="$1" 'BEGIN{printf "%d", v/1048576}'; }
miss=0
while [ ! -e "$SW/sample.stop" ] && [ $miss -lt 24 ]; do
  hp=$(cat "$SW/hub.pid" 2>/dev/null); pp=; cur=; peak=
  if [ -n "$hp" ] && kill -0 "$hp" 2>/dev/null; then
    miss=0
    pp=$(pgrep -P "$hp" | head -1)
    cg=/sys/fs/cgroup$(cut -d: -f3 /proc/$hp/cgroup)
    cur=$(cat $cg/memory.current 2>/dev/null); peak=$(cat $cg/memory.peak 2>/dev/null)
  fi
  r() { [ -n "$1" ] && awk '/^VmRSS/{printf "%d", $2/1024}' /proc/$1/status 2>/dev/null; }
  printf '%s %s %s %s %s\n' "$(date +%s)" "$(r "$hp")" "$(r "$pp")" "$([ -n "$cur" ] && mb $cur)" "$([ -n "$peak" ] && mb $peak)" >> "$OUT"
  [ -n "$cur" ] || miss=$((miss+1))   # no hub for 24 samples: this sampler ends by itself
  sleep "$N"
done
