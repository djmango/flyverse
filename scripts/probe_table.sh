#!/usr/bin/env bash
# Tabulate haltere-probe results.
#
# Usage: scripts/probe_table.sh runs/haltere-probe/*.json
#
# One row per run. The column that matters is the sign-conditioned difference
# in the steering motor neurons, because that is the only readout that a
# rotation-opposing reflex would have to move, and the Welch t is the guide to
# whether it moved by more than the trial-to-trial noise. Read the seed column
# next to it: a difference that flips sign across seeds is not a response.
set -euo pipefail

if [ "$#" -eq 0 ]; then
  echo "usage: $0 <probe.json> ..." >&2
  exit 2
fi

printf '%-14s %4s %5s %5s %4s | %10s %6s | %10s %6s | %10s\n' \
  run seed haltere flow amp "yaw steer" t "roll steer" t "yaw rate sig"
printf '%s\n' "$(printf '%.0s-' {1..95})"

for f in "$@"; do
  name=$(basename "$f" .json)
  jq -r --arg name "$name" '
    def ch(axis; key): .axes[axis][key] // {};
    [
      $name,
      (.seed // "?" | tostring),
      (if .haltere_connected then "on" else "off" end),
      (if .optic_flow_delivered == null then "?" elif .optic_flow_delivered then "on" else "off" end),
      (.amplitude_rad_s | tostring),
      (ch("yaw";  "steer differential (R-L)").difference // 0 | . * 10000 | round | . / 10000 | tostring),
      (ch("yaw";  "steer differential (R-L)").welch_t // 0 | . * 100 | round | . / 100 | tostring),
      (ch("roll"; "steer differential (R-L)").difference // 0 | . * 10000 | round | . / 10000 | tostring),
      (ch("roll"; "steer differential (R-L)").welch_t // 0 | . * 100 | round | . / 100 | tostring),
      (ch("yaw";  "yaw rate rad/s").difference // 0 | . * 100 | round | . / 100 | tostring)
    ] | @tsv
  ' "$f" | awk -F'\t' '{ printf "%-14s %4s %5s %5s %4s | %10s %6s | %10s %6s | %10s\n", $1,$2,$3,$4,$5,$6,$7,$8,$9,$10 }'
done
