#!/usr/bin/env bash
# SP-04: sample GPU and compositor load for N seconds and print a summary.
#
#   ./measure.sh <label> [seconds]      (default 30)
#
# What it records, once per second:
#   - whole-GPU utilisation and power draw (nvidia-smi --query-gpu)
#   - per-process GPU "sm" utilisation (nvidia-smi pmon), so gnome-shell and the game are
#     separated; gnome-shell near 0% while a fullscreen game runs means direct scanout
#   - gnome-shell CPU usage (pidstat)
# Raw data and the summary go to results/<label>.txt next to this script.
set -euo pipefail
export LC_ALL=C   # dot decimals from nvidia-smi and pidstat

label="${1:?usage: measure.sh <label> [seconds]}"
secs="${2:-30}"
here="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$here/results"
out="$here/results/$label.txt"

shell_pid="$(pgrep -x gnome-shell | head -1)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"; kill "${gpu_loop:-0}" 2>/dev/null || true' EXIT

nvidia-smi pmon -c "$secs" -s u -d 1 >"$tmp/pmon" 2>&1 &
pmon_pid=$!
nvidia-smi --query-gpu=utilization.gpu,power.draw,clocks.gr --format=csv,noheader,nounits -l 1 >"$tmp/gpu" 2>&1 &
gpu_loop=$!
pidstat -p "$shell_pid" 1 "$secs" >"$tmp/cpu" 2>&1
wait "$pmon_pid" || true
kill "$gpu_loop" 2>/dev/null || true

{
    echo "label: $label   seconds: $secs   date: $(date -Is)"
    awk -F', *' 'NF>=3 {u+=$1; p+=$2; c+=$3; n++}
        END {if (n) printf "gpu util avg: %.1f %%   power avg: %.1f W   graphics clock avg: %.0f MHz   (%d samples)\n", u/n, p/n, c/n, n}' "$tmp/gpu"
    awk -v secs="$secs" '!/^#/ && NF>=10 {v=($4=="-")?0:$4; sm[$10]+=v}
        END {for (k in sm) printf "pmon sm avg  %-16s %6.1f %%\n", k, sm[k]/secs}' "$tmp/pmon" | sort -k5 -nr | head -6
    awk '/gnome-shell/ && !/Average/ {s+=$8; n++} END {if (n) printf "gnome-shell CPU avg: %.1f %%   (%d samples)\n", s/n, n}' "$tmp/cpu"
} | tee "$out"
