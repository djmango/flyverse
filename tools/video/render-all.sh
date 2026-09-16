#!/usr/bin/env bash
# render-all.sh -- render the standing set of run-to-MP4 videos into docs/videos/.
#
# This is the project's "videos of all the tests" set: the default flight, every
# room size the room-size sweep covered, contrasting seeds, and one ablation pair.
# Each row names a run directory on disk; a run that is missing or empty is
# reported by fly2video.sh and does NOT abort the batch (its row prints FAIL).
#
# Usage: tools/video/render-all.sh [--jobs 3] [--out DIR] [--only NAME]
#
# The mapping is the same one documented in docs/videos/README.md.
# Row format:  name | run dir | camera view | on-screen label

set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$HERE/../.." && pwd)"
FLY2VIDEO="$HERE/fly2video.sh"

JOBS=3
OUT="$REPO_ROOT/docs/videos"
ONLY=""
SKIP_EXISTING=0
while [ $# -gt 0 ]; do
  case "$1" in
    --jobs) JOBS="$2"; shift 2 ;;
    --out)  OUT="$2"; shift 2 ;;
    --only) ONLY="$2"; shift 2 ;;
    --skip-existing) SKIP_EXISTING=1; shift ;;
    *) echo "render-all: unknown argument: $1" >&2; exit 2 ;;
  esac
done

FPS=24
WIDTH=1600
HEIGHT=900
TIME_SCALE=1.0

SPECS=$(cat <<'EOF'
default-flight-baseline|runs/analyze|chase|repo default "flyverse analyze" output (runs/analyze): seed 7, 12 s, 1x room, --every 10
default-flight-baseline-roomcam|runs/analyze|room|repo default "flyverse analyze" output (runs/analyze): seed 7, 12 s, 1x room -- ROOM camera
room-1x-seed7|/tmp/fv/part1/base_s7|chase|room-size sweep: 1x = 600x440x220 mm, seed 7, 12 s, --every 1
room-2x-seed7|/tmp/fv/part1/s2_s7|chase|room-size sweep: 2x = 1200x880x440 mm, seed 7, 12 s, --every 1
room-4x-seed7|/tmp/fv/part1/s4_s7|chase|room-size sweep: 4x = 2400x1760x880 mm, seed 7, 12 s, --every 1
room-4x-floor-1x-ceiling-seed7|/tmp/fv/part1/s4_h220_s7|chase|room-size sweep: 4x floor, 1x ceiling = 2400x1760x220 mm, seed 7, 12 s, --every 1
room-8x-seed7|/tmp/fv/part1/s8_s7|chase|room-size sweep: 8x = 4800x3520x1760 mm, seed 7, 12 s, --every 1
room-1x-floor-5p5x-ceiling-seed7|/tmp/fv/part1/s1_h1200_s7|chase|room-size sweep: 1x floor, 5.5x ceiling = 600x440x1200 mm, seed 7, 12 s, --every 1
room-1x-seed11|/tmp/fv/part1/base_s11|chase|room-size sweep: 1x, CONTRASTING SEED 11, 12 s, --every 1
room-1x-seed23|/tmp/fv/part1/base_s23|chase|room-size sweep: 1x, CONTRASTING SEED 23, 12 s, --every 1
room-4x-seed23-degenerate|/tmp/fv/part1/s4_s23|chase|room-size sweep: 4x, seed 23 -- DEGENERATE ROW: never took off (100% GROUND, 0% cruise), see docs/room-size-and-loom-pathway.md
ablation-baseline-seed7|/tmp/attr/baseline|chase|CONTROL of the ablation pair (run dir name: baseline): seed 7, 12 s, --every 10
ablation-no-retina-seed7|/tmp/attr/no-retina|chase|ABLATION (run dir name: no-retina = FLYVERSE_NO_RETINA=1): seed 7, 12 s, --every 10
fruit-in-room-seed7|/tmp/fv/after_s7|room|colour-vision-and-fruit test: default build with the colour tomato fruit in the room (src/room.rs FRUIT, no env flag), seed 7, 12 s, --every 1 -- ROOM camera: the fruit is on screen for the whole run
fruit-no-fruit-control-seed7|/tmp/fv/grain_s7|room|colour-vision-and-fruit test, CONTROL (run dir name: grain = FLYVERSE_NO_FRUIT=1): same room/table/sugar cube/odour, fruit removed, seed 7, 12 s, --every 1 -- ROOM camera: the empty place where the fruit would be
hand-slap-seed7|/tmp/fv/hand1x/s7_approach|room|hand-slap test: FLYVERSE_HAND=approach (150 ms slap, default knobs), seed 7 at spawn: the full-contact slap, see docs/hand-slap-and-escape.md -- ROOM camera: the approaching palm is in frame
hand-slap-static-control-seed7|/tmp/fv/hand1x/s7_static|room|hand-slap test, CONTROL: FLYVERSE_HAND=static -- the same palm parked at the launch origin (bit-identical to the approach run until t = 2.000 s), seed 7 -- ROOM camera
hand-slap-fast-seed11|/tmp/fv/fast1x/s11_approach|room|hand-slap test, FAST arm: FLYVERSE_HAND=approach FLYVERSE_HAND_DUR_MS=50 (the terminal phase at ~6 m/s), seed 11 -- the run that holds both, see docs/hand-slap-and-escape.md section 6.1 -- ROOM camera
hand-slap-fast-static-control-seed11|/tmp/fv/fast1x/s11_static|room|hand-slap test, FAST arm CONTROL: FLYVERSE_HAND=static FLYVERSE_HAND_DUR_MS=50, seed 11 -- ROOM camera
EOF
)

mkdir -p "$OUT"

run_one() {
  local name="$1" rundir="$2" view="$3" label="$4"
  local dir="$rundir"
  case "$dir" in /*) ;; *) dir="$REPO_ROOT/$rundir" ;; esac
  local log="$OUT/.$name.log"
  if bash "$FLY2VIDEO" --run "$dir" --name "$name" \
      --view "$view" --label "$label" \
      --fps "$FPS" --width "$WIDTH" --height "$HEIGHT" --time-scale "$TIME_SCALE" \
      --out "$OUT" >"$log" 2>&1; then
    local line
    line="$(grep -E '^\[fly2video\] OK|^ +frames=' "$log" | tr '\n' ' ' | tr -s ' ')"
    rm -f "$log"
    echo "OK   $name :: $line"
  else
    echo "FAIL $name  (run dir: $dir)  -- see $log"
    tail -3 "$log" | sed 's/^/       /'
  fi
}
export -f run_one
export FLY2VIDEO OUT FPS WIDTH HEIGHT TIME_SCALE

# The spec list is read into an array BEFORE any child is launched. A child that
# inherits the loop's stdin can consume part of a here-string and desynchronise
# the reads, producing rows with the wrong name/run dir; the array plus the
# `< /dev/null` on every child below removes that whole class of failure.
mapfile -t ROWS <<< "$SPECS"
pids=()
nports=0
for row in "${ROWS[@]}"; do
  IFS='|' read -r name rundir view label <<< "$row"
  [ -z "$name" ] && continue
  if [ -n "$ONLY" ] && [ "$ONLY" != "$name" ] && ! printf '%s' ",$ONLY," | grep -q ",$name,"; then continue; fi
  if [ "$SKIP_EXISTING" = "1" ] && [ -s "$OUT/$name.mp4" ]; then
    echo "SKIP $name (already in $OUT)"
    continue
  fi
  # A monotone counter mod JOBS: a port pair is reused only after the job that
  # held it has been waited on, so two concurrent browsers never share a port.
  slot=$(( nports % JOBS )); nports=$(( nports + 1 ))
  # distinct port pair per concurrent job so the browsers never collide
  port=$((8137 + slot))
  cdp=$((9333 + slot))
  (
    FV_SERVER_PORT=$port FV_CDP_PORT=$cdp FV_WORK_ROOT=/tmp/fvsrv \
      run_one "$name" "$rundir" "$view" "$label"
  ) < /dev/null &
  pids+=($!)
  if [ "${#pids[@]}" -ge "$JOBS" ]; then
    wait "${pids[0]}" || true
    pids=("${pids[@]:1}")
  fi
done
for p in "${pids[@]:-}"; do [ -n "$p" ] && wait "$p" || true; done

echo "---- $OUT ----"
ls -l "$OUT"/*.mp4 2>/dev/null || echo "(no mp4 files)"