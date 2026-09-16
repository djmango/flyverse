#!/usr/bin/env bash
# fly2video.sh -- turn one recorded flyverse run into an MP4, reproducibly.
#
#   tools/video/fly2video.sh --run /tmp/fv/part1/base_s7 --name room-1x-seed7 \
#       [--view chase|room|top|orbit] [--fps 24] [--width 1600] [--height 900] \
#       [--time-scale 1.0] [--label "TEXT"] [--sim-start S] [--sim-end S] \
#       [--out docs/videos] [--keep-frames]
#
# What it does, in order:
#   1. serves web/ over static HTTP (the visualiser needs modules + assets),
#   2. launches a headless Chrome with a DevTools endpoint,
#   3. replays the run's trace.csv through the page (tools/video/driver.mjs),
#      one PNG per simulated frame,
#   4. encodes the PNG sequence with ffmpeg at a constant frame rate,
#   5. prints the path, duration, resolution and size of the MP4.
#
# Determinism: the frame count is derived from the trace's own timestamps
# (simSpan * time_scale * fps) and every frame is the trace interpolated at a
# fixed simulated time, so the same run and the same flags always produce the
# same number of frames of the same content. No frame is dropped and none is
# duplicated: the driver writes exactly N numbered frames and this script refuses
# to encode if the directory holds a different count.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WEB_ROOT="$REPO_ROOT/web"
DRIVER="$REPO_ROOT/tools/video/driver.mjs"

RUN=""
NAME=""
VIEW="chase"
FPS=24
WIDTH=1600
HEIGHT=900
TIME_SCALE=1.0
LABEL=""
SIM_START=""
SIM_END=""
OUT_DIR="$REPO_ROOT/docs/videos"
KEEP_FRAMES=0
SERVER_PORT="${FV_SERVER_PORT:-8137}"
CDP_PORT="${FV_CDP_PORT:-9333}"
WORK_ROOT="${FV_WORK_ROOT:-/tmp/fvsrv}"

while [ $# -gt 0 ]; do
  case "$1" in
    --run)         RUN="$2"; shift 2 ;;
    --name)        NAME="$2"; shift 2 ;;
    --view)        VIEW="$2"; shift 2 ;;
    --fps)         FPS="$2"; shift 2 ;;
    --width)       WIDTH="$2"; shift 2 ;;
    --height)      HEIGHT="$2"; shift 2 ;;
    --time-scale)  TIME_SCALE="$2"; shift 2 ;;
    --label)       LABEL="$2"; shift 2 ;;
    --sim-start)   SIM_START="$2"; shift 2 ;;
    --sim-end)     SIM_END="$2"; shift 2 ;;
    --out)         OUT_DIR="$2"; shift 2 ;;
    --keep-frames) KEEP_FRAMES=1; shift ;;
    -h|--help)     sed -n '2,25p' "$0"; exit 0 ;;
    *) echo "fly2video: unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [ -z "$RUN" ]; then echo "fly2video: --run DIR is required" >&2; exit 2; fi
if [ -z "$NAME" ]; then NAME="$(basename "$(readlink -f "$RUN")")"; fi

# A run that is not there, or is there but empty, is reported -- never substituted.
if [ ! -d "$RUN" ]; then
  echo "fly2video: RUN DIR DOES NOT EXIST: $RUN" >&2; exit 3
fi
if [ ! -s "$RUN/trace.csv" ]; then
  echo "fly2video: RUN HAS NO trace.csv (or it is empty): $RUN" >&2; exit 3
fi
if [ ! -s "$RUN/summary.json" ]; then
  echo "fly2video: RUN HAS NO summary.json (or it is empty): $RUN" >&2; exit 3
fi

WORK="$WORK_ROOT/$NAME"
FRAMES="$WORK/frames"
mkdir -p "$FRAMES" "$OUT_DIR"
rm -f "$FRAMES"/*.png

WEBGL_FLAGS=(
  --no-sandbox --disable-setuid-sandbox --disable-dev-shm-usage
  --use-gl=angle --use-angle=swiftshader --enable-unsafe-swiftshader
  --hide-scrollbars --force-device-scale-factor=1 --mute-audio
  --disable-background-timer-throttling --disable-renderer-backgrounding
  --window-size="${WIDTH},${HEIGHT}"
)

CHROME=""
for c in \
  $(ls -d /var/lib/hermes/data/home/.cache/ms-playwright/chromium_headless_shell-*/chrome-headless-shell-linux64/chrome-headless-shell 2>/dev/null | sort -r) \
  $(ls -d /opt/hermes/.playwright/chromium_headless_shell-*/chrome-headless-shell-linux64/chrome-headless-shell 2>/dev/null | sort -r) \
  $(ls -d /var/lib/hermes/data/home/.cache/ms-playwright/chromium-*/chrome-linux64/chrome 2>/dev/null | sort -r) ; do
  [ -x "$c" ] && CHROME="$c" && break
done
if [ -z "$CHROME" ]; then
  echo "fly2video: no chrome/headless-shell binary found" >&2; exit 4
fi

# The Playwright Chromium bundle needs its own shared-library set on this host
# (nix-store libnspr4 and friends); without it the binary dies before opening a
# DevTools port.
PW_LIBS="$(ls -d /nix/store/*pw-chrome-libs*/lib 2>/dev/null | head -1)"
if [ -n "$PW_LIBS" ]; then
  export LD_LIBRARY_PATH="$PW_LIBS:${LD_LIBRARY_PATH:-}"
fi

cleanup() {
  [ -n "${SERVER_PID:-}" ] && kill "$SERVER_PID" 2>/dev/null || true
  [ -n "${CHROME_PID:-}" ] && kill "$CHROME_PID" 2>/dev/null || true
  wait 2>/dev/null || true
}
trap cleanup EXIT

echo "[fly2video] serving $WEB_ROOT on 127.0.0.1:$SERVER_PORT"
python3 -m http.server "$SERVER_PORT" --bind 127.0.0.1 --directory "$WEB_ROOT" >"$WORK/http.log" 2>&1 &
SERVER_PID=$!
for _ in $(seq 1 100); do
  curl -fsS "http://127.0.0.1:$SERVER_PORT/index.html" -o /dev/null 2>/dev/null && break
  sleep 0.1
done
curl -fsS "http://127.0.0.1:$SERVER_PORT/index.html" -o /dev/null \
  || { echo "fly2video: static server did not come up (see $WORK/http.log)" >&2; exit 5; }

rm -rf "$WORK/profile"
echo "[fly2video] launching $CHROME (CDP 127.0.0.1:$CDP_PORT)"
"$CHROME" "${WEBGL_FLAGS[@]}" \
  --user-data-dir="$WORK/profile" \
  --remote-debugging-port="$CDP_PORT" \
  --remote-allow-origins='*' \
  about:blank >"$WORK/chrome.log" 2>&1 &
CHROME_PID=$!
for _ in $(seq 1 150); do
  curl -fsS "http://127.0.0.1:$CDP_PORT/json/version" -o /dev/null 2>/dev/null && break
  sleep 0.1
done
curl -fsS "http://127.0.0.1:$CDP_PORT/json/version" -o /dev/null \
  || { echo "fly2video: chrome DevTools endpoint did not come up (see $WORK/chrome.log)" >&2; exit 6; }

META="$WORK/$NAME.meta.json"
DRIVER_ARGS=(
  --run "$RUN" --frames "$FRAMES" --json "$META"
  --view "$VIEW" --fps "$FPS" --width "$WIDTH" --height "$HEIGHT"
  --time-scale "$TIME_SCALE"
  --cdp "http://127.0.0.1:$CDP_PORT"
  --url "http://127.0.0.1:$SERVER_PORT/index.html?skip=brain,spikes"
)
[ -n "$LABEL" ] && DRIVER_ARGS+=(--label "$LABEL")
[ -n "$SIM_START" ] && DRIVER_ARGS+=(--sim-start "$SIM_START")
[ -n "$SIM_END" ] && DRIVER_ARGS+=(--sim-end "$SIM_END")

node "$DRIVER" "${DRIVER_ARGS[@]}" < /dev/null

EXPECTED="$(node -e "process.stdout.write(String(JSON.parse(require('fs').readFileSync('$META','utf8')).framesWritten))")"
ACTUAL="$(find "$FRAMES" -maxdepth 1 -name '*.png' | wc -l | tr -d ' ')"
if [ "$EXPECTED" != "$ACTUAL" ]; then
  echo "fly2video: frame count mismatch: driver reported $EXPECTED, directory holds $ACTUAL" >&2
  exit 7
fi

MP4="$OUT_DIR/$NAME.mp4"
ffmpeg -hide_banner -loglevel error -y \
  -framerate "$FPS" -i "$FRAMES/%06d.png" \
  -c:v libx264 -preset slow -crf 18 -pix_fmt yuv420p -threads 1 \
  -movflags +faststart \
  "$MP4"

if [ "$KEEP_FRAMES" = "0" ]; then rm -rf "$FRAMES"; fi

DUR="$(ffprobe -v error -show_entries format=duration -of default=nw=1:nk=1 "$MP4")"
RES="$(ffprobe -v error -select_streams v:0 -show_entries stream=width,height -of csv=p=0:s=x "$MP4")"
SIZE="$(stat -c%s "$MP4")"
FRAMES_WRITTEN="$EXPECTED"
echo "[fly2video] OK  $MP4"
echo "            frames=$FRAMES_WRITTEN  duration=${DUR}s  resolution=$RES  bytes=$SIZE"
echo "            meta=$META"