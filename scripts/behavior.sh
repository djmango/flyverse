#!/usr/bin/env bash
# Behaviour tooling for the embodied fly.
#
#   scripts/behavior.sh run   [SECONDS] [SEED ...]   # analyse several seeds into runs/
#   scripts/behavior.sh table [runs/seed*]           # one-row-per-run comparison
#   scripts/behavior.sh show  runs/seed7             # headline block for one run
#
# Every number comes from a real headless run of the full 166,700-neuron model.
# Nothing here is simulated for display.

set -euo pipefail
cd "$(dirname "$0")/.."

BIN=./target/release/flyverse
JQ=$(command -v jq || true)

usage() { sed -n '2,9p' "$0"; exit 2; }

cmd_run() {
  local secs="${1:-30}"; shift || true
  local seeds=("$@")
  [ ${#seeds[@]} -eq 0 ] && seeds=(7 11 23)
  for s in "${seeds[@]}"; do
    echo "=== seed $s, ${secs}s ==="
    "$BIN" analyze --seconds "$secs" --seed "$s" --every 10 --out "runs/seed$s"
    echo
  done
}

cmd_table() {
  local dirs=("$@")
  [ ${#dirs[@]} -eq 0 ] && dirs=(runs/seed*)
  [ -n "$JQ" ] || { echo "jq is required for the table"; exit 1; }
  printf '%-6s %-9s %-8s %-9s %-9s %-9s %-9s %-8s %s\n' \
    seed cruise% wall/min tortuosity speed alt_mean yaw_rate corr_loom_steer verdict
  for d in "${dirs[@]}"; do
    [ -f "$d/summary.json" ] || continue
    "$JQ" -r --arg d "$d" '
      def n(x; p): if (x==null or (x|type)!="number") then "-" else (x*1|tostring) end;
      [ .config.seed,
        (.mode_percent.CRUISE|.*100|round/100),
        (.events.wall_hits_per_min|.*10|round/10),
        (.movement.tortuosity|.*100|round/100),
        (.movement.mean_speed_mm_s|round),
        (.altitude_mm.mean|.*10|round/10),
        (.turning.mean_abs_yaw_rate_rad_s|.*100|round/100),
        (.loom_response.pearson_loom_vs_steer_differential|.*1000|round/1000),
        (if (.loom_response.pearson_loom_vs_steer_differential|fabs) < 0.1
           then "no loom->steer" else "loom steers" end)
      ] | @tsv' "$d/summary.json" \
    | awk -F'\t' '{printf "%-6s %-9s %-8s %-9s %-9s %-9s %-9s %-8s %s\n",$1,$2,$3,$4,$5,$6,$7,$8,$9}'
  done
}

cmd_show() {
  local d="${1:-runs/seed7}"
  "$JQ" -r '.' "$d/summary.json" >/dev/null
  echo "run: $d"
  "$JQ" -r '
    "mode %      ground \(.mode_percent.GROUND|.*10|round/10)  takeoff \(.mode_percent.TAKEOFF|.*10|round/10)  cruise \(.mode_percent.CRUISE|.*10|round/10)  landing \(.mode_percent.LANDING|.*10|round/10)  feeding \(.mode_percent.FEEDING|.*10|round/10)",
    "events      takeoffs \(.events.takeoffs)  landings \(.events.landings)  wall hits \(.events.wall_hits)  meals \(.events.eats)",
    "walls       \(.events.wall_hits_per_min|.*10|round/10)/min, \(.walls.pct_airborne_within_20mm|.*10|round/10)% of airborne time within 20 mm of a wall",
    "movement    path \(.movement.path_horizontal_mm|round) mm, net \(.movement.net_horizontal_displacement_mm|round) mm, tortuosity \(.movement.tortuosity|.*100|round/100)",
    "altitude    mean \(.altitude_mm.mean|.*10|round/10) mm, p05 \(.altitude_mm.p05|round), p95 \(.altitude_mm.p95|round), max \(.altitude_mm.max|round)",
    "steering    mean |yaw rate| \(.turning.mean_abs_yaw_rate_rad_s|.*100|round/100) rad/s, \(.turning.full_circles_airborne|.*100|round/100) circles airborne",
    "LOOM TEST   corr(loom, steer) \(.loom_response.pearson_loom_vs_steer_differential|.*1000|round/1000)  corr(loom, yaw) \(.loom_response.pearson_loom_vs_yaw_rate|.*1000|round/1000)",
    "neural      \(.neural.total_spikes) spikes, \(.neural.mean_rate_hz|.*100|round/100) Hz mean rate"
  ' "$d/summary.json"
  [ -f "$d/report.html" ] && echo "report: $d/report.html"
}

case "${1:-}" in
  run)   shift; cmd_run "$@" ;;
  table) shift; cmd_table "$@" ;;
  show)  shift; cmd_show "$@" ;;
  *)     usage ;;
esac
