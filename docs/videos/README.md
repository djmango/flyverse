# Run-to-video pipeline

Every video in this directory is one recorded `flyverse analyze` run replayed
through the project's own three.js visualiser (`web/`), with the run's own
read-outs drawn over it, encoded at a fixed frame rate.

Nothing here is a screen recording. A screen recording of the live page would
drop frames, would not be reproducible, and would show whatever moment the
wall clock happened to be at. Instead the visualiser is stepped by hand: output
frame *k* is the run's `trace.csv` interpolated at simulated time
`t0 + k*dt`, so a captured frame is tied to the simulation, not to the clock.

---

## 1. The command

```bash
tools/video/fly2video.sh --run DIR --name NAME \
    [--view chase|room|top|orbit] [--fps 24] [--width 1600] [--height 900] \
    [--time-scale 1.0] [--label "TEXT"] [--sim-start S] [--sim-end S] \
    [--out docs/videos] [--keep-frames]
```

One run directory in, one MP4 out: `docs/videos/NAME.mp4` (1600x900, H.264,
`yuv420p`, 24 fps, faststart). `DIR` must hold both `trace.csv` and
`summary.json`; a directory that is missing, or holds an empty trace, is
reported and the command exits non-zero rather than substituting another run
(`tools/video/fly2video.sh`, the three `[ ! -s ... ]` guards).

The whole standing set (the table in §3) is one command:

```bash
tools/video/render-all.sh --jobs 3
```

`--jobs N` runs N videos at a time, each child on its own static-server and
DevTools port pair.

### What it does

| step | tool | file |
|---|---|---|
| serve `web/` over static HTTP (modules, `assets/rig.json`, STL meshes) | `python3 -m http.server` | `fly2video.sh` |
| launch headless Chrome with a DevTools endpoint | `chrome-headless-shell` (Playwright bundle) + nix `pw-chrome-libs` for `LD_LIBRARY_PATH` | `fly2video.sh` |
| stop the page's rAF loop, size the arena, replay the trace, capture one PNG per frame | DevTools Protocol over Node's built-in `WebSocket` | `tools/video/driver.mjs`, `tools/video/cdp.mjs` |
| draw the evidence panel (identity, live read-outs, sparkline, top-down path) | injected DOM + `<canvas>`, no library | `tools/video/overlay.mjs` |
| encode the PNG sequence | `ffmpeg -framerate 24 -c:v libx264 -crf 18 -pix_fmt yuv420p -threads 1 -movflags +faststart` | `fly2video.sh` |

### Reproducibility

The frame count is derived from the run's own timestamps
(`nFrames = round(simSpan * time_scale * fps)`), each frame is the trace
interpolated at a fixed simulated time, and the page's animation clock is
stopped and reset before replay (`web/app.js`, `replayStart`). The driver writes
exactly `nFrames` numbered PNGs and `fly2video.sh` refuses to encode if the
directory holds a different count, so a frame cannot be silently dropped or
duplicated.

Verified: two independent renders of the same run produce byte-identical PNG
frames and byte-identical MP4s (`md5sum` of all 24 frames and of the encoded
file matched across separate browser launches). The same was borne out
incidentally while building the set: a row rendered twice (once before a
batch-parsing fix, once after) produced MP4s of exactly
`5524535` and `4128772` bytes respectively — the same byte counts as its
correctly-named twin.

It was then re-confirmed deliberately end to end: after fixing the batch
driver, three rows (`room-1x-seed7`, `room-2x-seed7`, `room-4x-seed7`) were
re-rendered from scratch into a different output directory, concurrently, in a
fresh set of browsers. All three came out byte-identical to the committed
copies (`4570032`, `3942550`, `3772673` bytes), i.e. the pipeline is
re-runnable and the batch driver dispatches each row to its own run directory.

"Same run in, same video out" is relative to the **content** of the run: the
identity of a video is its `trace.csv`. If a run is regenerated (new columns,
different sim), the same command produces a different video, correctly. The
hashes below are the traces these MP4s were built from (each file's mtime
predates the render, so these are the exact bytes that were encoded):

| run directory | `trace.csv` md5 |
|---|---|
| `runs/analyze` | `4511986ad15254da81232659f82f7827` |
| `/tmp/fv/part1/base_s7` | `fba51b2eae4a7009ec4f549698f1d846` |
| `/tmp/fv/part1/s2_s7` | `a18f33a700df112d4ccbb51829917557` |
| `/tmp/fv/part1/s4_s7` | `0fa1668c0cbc3ed653f3dbfbba68f492` |
| `/tmp/fv/part1/s4_h220_s7` | `624947aea88bcd1bb34ea6222242affc` |
| `/tmp/fv/part1/s8_s7` | `57705efc781dfc39143346628b846321` |
| `/tmp/fv/part1/s1_h1200_s7` | `81818ee8ac1d465fd351d2f2d6f3d8cb` |
| `/tmp/fv/part1/base_s11` | `b2da4b5a8b3eb23cad18cee2fd2e0cf3` |
| `/tmp/fv/part1/base_s23` | `c5dd1e982b476e1b006646573b2ea98a` |
| `/tmp/fv/part1/s4_s23` | `3350c76e47b9093eab7218772abb1cfd` |
| `/tmp/attr/baseline` | `64d4b445381450b2147ab794cc9060d1` |
| `/tmp/attr/no-retina` | `b15fe24571bdde77d74df83b5de7ddf2` |

(Re-run `md5sum DIR/trace.csv` before re-rendering to confirm you are building
from the same data; the driver maps columns **by header name**, not by
position, so added columns do not shift the overlay.)

---

## 2. How the visualiser is driven (recon)

**It does not replay a trace, and it cannot be pointed at a run directory.**
The visualiser renders a *live* simulation:

- The Rust server runs the sim in its own thread and pushes a JSON frame every
  40 ms — `src/sim.rs:1208` (sim thread), `src/sim.rs:1246`
  (`STREAM_MS`-gated push), `src/sim.rs:1036` (`frame_json`, the front-end
  contract).
- The page subscribes with `EventSource` to `/api/stream` —
  `web/app.js:891`, handler at `web/app.js:899`, applied by `applyFrame` at
  `web/app.js:814`.
- There is no trace input and no `?run=`/`?seed=` URL parameter. The only
  query parameters are `?skip=`, `?dbg=`, `?iso=`, `?lite=`, `?model=`
  (`web/app.js:1220`-`1224`, `web/app.js:125`, `web/app.js:460`).
- A specific run is selected on the *server* side:
  `flyverse serve --seed S [--seconds N] [--rate HZ] [--port N]`
  (`src/main.rs:264`). Room size and conditions are environment variables read
  at construction (`FLYVERSE_ROOM_SCALE`, `FLYVERSE_ROOM_HEIGHT` —
  `src/room.rs:69`, `src/room.rs:83`; and the ablation flags
  `FLYVERSE_NO_RETINA`, `FLYVERSE_NO_ODOR`, `FLYVERSE_NO_HALTERE`, …).
- The only playback controls the server exposes are
  `/api/control?cmd=toggle_pause` and `?cmd=reset` (`src/sim.rs:1321`). There is
  **no step/seek command**, which is why frame-accurate capture is done by
  replaying the recorded trace in the page rather than by pacing the live
  server.

**Cameras.** All four exist and are selectable, by button or by key `0`-`3`:

| # | name | selection | definition |
|---|---|---|---|
| 0 | chase | `#cam-0`, key `0` | `web/app.js:992` — clamped inside the room, follows the fly |
| 1 | room | `#cam-1`, key `1` | `web/app.js:1006` — fixed outside the room at `ROOM_CAM_POS` (`web/app.js:979`) |
| 2 | top | `#cam-2`, key `2` | `web/app.js:1011` |
| 3 | orbit | `#cam-3`, key `3` | `web/app.js:1015` — drag to rotate, wheel to zoom |

Default is chase (`web/app.js:970`). `docs/chase-cam.png` and
`docs/room-cam.png` are the two documented reference captures.

**HUD.** Two panels plus a footer (`web/index.html`):

- Left column — `link`, `stimulus`, `sim time`, `realtime`, `seq`, `paused`;
  `mode`, `airborne`, `speed`, `altitude`, `pos mm`, `pitch/roll/yaw`,
  `food dist`, `wing amp`, `legs down`; connectome spike counts; the event
  counters; and the camera buttons. Written by `applyFrame`
  (`web/app.js:833`-`864`).
- Right column — neural-region bars, motor bars, the 3-D connectome panel, and
  sensory bars. Hidden in replay (§6).
- Footer — the **build/cache stamp**: `<span id="f-appver">`, written by
  `stampBuild()` at `web/app.js:26` to `app: 20260913a`. The same string is the
  cache-buster on the module and stylesheet
  (`web/index.html:7`, `web/index.html:151`), so the stamp on screen proves
  which revision the page actually ran. It stays visible in every video.

### The replay hook

`web/app.js` gained an opt-in replay API (`web/app.js`, "offline replay"):
`window.__flyverse.replay.start() | step(simT, dt, frame) | setRoom(scale,
heightMm) | setCam(n) | camState`. It reuses the page's own `applyFrame`,
`updatePose`, `updateCamera`, `updateTrail` and `updateMarkers`, so the fly,
the trail, the camera and the HUD are the production code paths. `index.html`
and the live path are unchanged: nothing runs unless `start()` is called.

---

## 3. Run → video mapping

All 12 s runs are rendered at `--time-scale 1.0`, i.e. **1 s sim = 1 s video**,
24 fps, 288 frames, ~12 s each.

| MP4 | run directory | camera | what it is |
|---|---|---|---|
| `default-flight-baseline.mp4` | `runs/analyze` | chase | the repo's committed default `analyze` output: seed 7, 12 s, 1x room, `--every 10` |
| `default-flight-baseline-roomcam.mp4` | `runs/analyze` | room | the same run on the other documented camera |
| `room-1x-seed7.mp4` | `/tmp/fv/part1/base_s7` | chase | room-size sweep, 1x = 600x440x220 mm |
| `room-2x-seed7.mp4` | `/tmp/fv/part1/s2_s7` | chase | room-size sweep, 2x = 1200x880x440 mm |
| `room-4x-seed7.mp4` | `/tmp/fv/part1/s4_s7` | chase | room-size sweep, 4x = 2400x1760x880 mm |
| `room-4x-floor-1x-ceiling-seed7.mp4` | `/tmp/fv/part1/s4_h220_s7` | chase | 4x floor with the 1x ceiling = 2400x1760x220 mm |
| `room-8x-seed7.mp4` | `/tmp/fv/part1/s8_s7` | chase | room-size sweep, 8x = 4800x3520x1760 mm |
| `room-1x-floor-5p5x-ceiling-seed7.mp4` | `/tmp/fv/part1/s1_h1200_s7` | chase | 1x floor with a 1200 mm ceiling = 600x440x1200 mm |
| `room-1x-seed11.mp4` | `/tmp/fv/part1/base_s11` | chase | contrasting seed 11, 1x |
| `room-1x-seed23.mp4` | `/tmp/fv/part1/base_s23` | chase | contrasting seed 23, 1x |
| `room-4x-seed23-degenerate.mp4` | `/tmp/fv/part1/s4_s23` | chase | the documented degenerate cell: at 4x seed 23 never took off (100 % GROUND, 0 % cruise) |
| `ablation-baseline-seed7.mp4` | `/tmp/attr/baseline` | chase | **control** of the ablation pair, seed 7, `--every 10` |
| `ablation-no-retina-seed7.mp4` | `/tmp/attr/no-retina` | chase | **ablation**: `FLYVERSE_NO_RETINA=1`, seed 7, `--every 10` |

Every room size the sweep covered that is present on disk is included; the six
sizes above are exactly the sweep's rows in
`docs/room-size-and-loom-pathway.md`.

Runs on disk that were deliberately **not** rendered: the sweep's other seed
rows for the non-1x sizes — `/tmp/fv/part1/{s2_s11,s2_s23,s4_s11,s4_h220_s11,s4_h220_s23,s8_s11,s8_s23,s1_h1200_s11,s1_h1200_s23}`
all hold a complete `trace.csv` + `summary.json` and can be rendered by adding a
row to `tools/video/render-all.sh` and running
`render-all.sh --only NAME`, but only the seed-7 rows, the two contrasting 1x
seeds and the one degenerate 4x/23 cell are included here. Also not rendered:
`runs/seed7`, a 60 s run, which at 1:1 would be a 60 s video — outside the
watchable range the brief asked for, and it duplicates the default flight
already covered at 12 s.

### Exact command for each video

`render-all.sh` runs exactly these (add `--fps 24 --width 1600 --height 900
--time-scale 1.0`, which the script fixes for every row):

```bash
tools/video/fly2video.sh --run runs/analyze             --name default-flight-baseline              --view chase --label "repo default \"flyverse analyze\" output (runs/analyze): seed 7, 12 s, 1x room, --every 10"
tools/video/fly2video.sh --run runs/analyze             --name default-flight-baseline-roomcam      --view room  --label "repo default \"flyverse analyze\" output (runs/analyze): seed 7, 12 s, 1x room -- ROOM camera"
tools/video/fly2video.sh --run /tmp/fv/part1/base_s7    --name room-1x-seed7                        --view chase --label "room-size sweep: 1x = 600x440x220 mm, seed 7, 12 s, --every 1"
tools/video/fly2video.sh --run /tmp/fv/part1/s2_s7      --name room-2x-seed7                        --view chase --label "room-size sweep: 2x = 1200x880x440 mm, seed 7, 12 s, --every 1"
tools/video/fly2video.sh --run /tmp/fv/part1/s4_s7      --name room-4x-seed7                        --view chase --label "room-size sweep: 4x = 2400x1760x880 mm, seed 7, 12 s, --every 1"
tools/video/fly2video.sh --run /tmp/fv/part1/s4_h220_s7 --name room-4x-floor-1x-ceiling-seed7       --view chase --label "room-size sweep: 4x floor, 1x ceiling = 2400x1760x220 mm, seed 7, 12 s, --every 1"
tools/video/fly2video.sh --run /tmp/fv/part1/s8_s7      --name room-8x-seed7                        --view chase --label "room-size sweep: 8x = 4800x3520x1760 mm, seed 7, 12 s, --every 1"
tools/video/fly2video.sh --run /tmp/fv/part1/s1_h1200_s7 --name room-1x-floor-5p5x-ceiling-seed7    --view chase --label "room-size sweep: 1x floor, 5.5x ceiling = 600x440x1200 mm, seed 7, 12 s, --every 1"
tools/video/fly2video.sh --run /tmp/fv/part1/base_s11   --name room-1x-seed11                       --view chase --label "room-size sweep: 1x, CONTRASTING SEED 11, 12 s, --every 1"
tools/video/fly2video.sh --run /tmp/fv/part1/base_s23   --name room-1x-seed23                       --view chase --label "room-size sweep: 1x, CONTRASTING SEED 23, 12 s, --every 1"
tools/video/fly2video.sh --run /tmp/fv/part1/s4_s23     --name room-4x-seed23-degenerate            --view chase --label "room-size sweep: 4x, seed 23 -- DEGENERATE ROW: never took off (100% GROUND, 0% cruise), see docs/room-size-and-loom-pathway.md"
tools/video/fly2video.sh --run /tmp/attr/baseline       --name ablation-baseline-seed7              --view chase --label "CONTROL of the ablation pair (run dir name: baseline): seed 7, 12 s, --every 10"
tools/video/fly2video.sh --run /tmp/attr/no-retina      --name ablation-no-retina-seed7             --view chase --label "ABLATION (run dir name: no-retina = FLYVERSE_NO_RETINA=1): seed 7, 12 s, --every 10"
```

Every row above was verified after rendering: for each MP4 the driver's meta
file records the `--run` directory it actually replayed, and for all 13 that
directory is the one named in §3 (a mis-parsed batch row would show up here as a
run directory that does not match its video).

---

## 4. Overlay fields and where each comes from

Panel title: `RUN EVIDENCE`. Every number is labelled with the `trace.csv`
column or `summary.json` key it came from, in brackets. There is no smoothing
and no derived metric that summary.json does not already define.

| on screen | source |
|---|---|
| `run dir` | the `--run` argument |
| `seed`, `sim seconds`, `pack`, `sample every` | `summary.json` → `config.seed`, `config.seconds`, `config.pack`, `config.sample_every_windows` (x `config.window_ms`) |
| `room` | `summary.json` → `config.arena_mm` (x/y/z extents), `config.room_scale`, `config.room_height_mm` |
| `target / food place` | reports *not recorded* — see §6 |
| `TIME SCALE` | `nFrames`, `fps` and `simSpan` as computed by the driver; also states `1 s sim = R s video` |
| `sim t` | `trace.t` |
| `mode`, `airborne` | `trace.mode` (0 GROUND, 1 TAKEOFF, 2 CRUISE, 3 LANDING, 4 FEEDING); airborne is `mode` in {TAKEOFF, CRUISE, LANDING}, the same predicate as `src/sim.rs:1031` |
| `altitude` | `trace.z` (mm above the floor; the sim's frame calls this `body.pos[2]`) |
| `speed` | `trace.speed` (mm/s) |
| `yaw rate` | `trace.yaw_rate` (rad/s) |
| `steering diff` | `trace.steer_l - trace.steer_r` — the difference of the two recorded steering read-outs, and nothing else |
| `steering L / R` | `trace.steer_l`, `trace.steer_r` |
| `power L / R` | `trace.pow_l`, `trace.pow_r` |
| `wing amp` | `trace.wing_amp` |
| `wall dist` | `trace.wall_dist` (mm to the nearest wall) |
| `walls / takeoffs / landings` | `trace.wall_hits`, `trace.takeoffs`, `trace.landings` |
| `loom` | `trace.loom` |
| `odor L / R` | `trace.odor_l`, `trace.odor_r` |
| `flow L / R` | `trace.flow_l`, `trace.flow_r` |
| `spikes win / total` | `trace.win_spikes`, `trace.tot_spikes` |
| sparkline, cyan | `trace.speed` vs `trace.t` |
| sparkline, amber | `trace.steer_l - trace.steer_r` vs `trace.t` |
| sparkline, white playhead | the simulated time of the frame being shown |
| top-down box, grey outline | `summary.json` → `config.arena_mm`, in the arena's own frame |
| top-down box, cyan path | `trace.x`, `trace.y` over the whole run |
| top-down box, amber dot | `trace.x`, `trace.y` at the frame's simulated time |
| footer `app: …` | the visualiser's own build stamp (`web/app.js:26`) |

The left column of the *visualiser's own* HUD is filled from the same frame
payload (`applyFrame`), so the two panels agree. Two of its rows are worth a
caveat: its `spikes/s` is `win_spikes` divided by the simulated step, so it is
spikes per *simulated* second (there is no wall clock in a replay); and
`neurons active` / `mean rate Hz` show `-` because the trace does not record
them.

---

## 5. Sharing

Each MP4 is uploaded with

```bash
curl -fsS -X POST "$ZIPLINE_URL/api/upload" -H "Authorization: $ZIPLINE_TOKEN" \
     -F "file=@docs/videos/NAME.mp4"
```

The URL from the JSON response is listed in `LINKS.md` in this directory. The
token is read from the environment and is never echoed, never written to a file
and never printed — only the returned URL is.

`tools/video/upload-zipline.sh` does this for every MP4 in the directory and
writes `LINKS.md`. An upload that fails is reported with its local path and the
HTTP status and is **not** retried in a loop; a re-run for the failures adds the
missing rows to the existing table.

The uploaded share links (all verified `200` and byte-identical to the local
files) are:

| video | duration | resolution | size (B) | share URL |
|---|---|---|---|---|
| `ablation-baseline-seed7` | 12.000 | 1600x900 | 3767151 | https://share.skg.gg/u/0c6YfX.mp4 |
| `ablation-no-retina-seed7` | 12.000 | 1600x900 | 3755279 | https://share.skg.gg/u/WdtGj6.mp4 |
| `default-flight-baseline` | 12.000 | 1600x900 | 2364737 | https://share.skg.gg/u/VdoMmK.mp4 |
| `default-flight-baseline-roomcam` | 12.000 | 1600x900 | 968796 | https://share.skg.gg/u/pGNmTe.mp4 |
| `room-1x-floor-5p5x-ceiling-seed7` | 12.000 | 1600x900 | 4296489 | https://share.skg.gg/u/aMA8fg.mp4 |
| `room-1x-seed11` | 12.000 | 1600x900 | 5524535 | https://share.skg.gg/u/kuEKE6.mp4 |
| `room-1x-seed23` | 12.000 | 1600x900 | 4743147 | https://share.skg.gg/u/DyLzWR.mp4 |
| `room-1x-seed7` | 12.000 | 1600x900 | 4570032 | https://share.skg.gg/u/L5gnb9.mp4 |
| `room-2x-seed7` | 12.000 | 1600x900 | 3942550 | https://share.skg.gg/u/NbdUYy.mp4 |
| `room-4x-floor-1x-ceiling-seed7` | 12.000 | 1600x900 | 4128772 | https://share.skg.gg/u/FgG1eX.mp4 |
| `room-4x-seed23-degenerate` | 12.000 | 1600x900 | 3475248 | https://share.skg.gg/u/97tKMM.mp4 |
| `room-4x-seed7` | 12.000 | 1600x900 | 3772673 | https://share.skg.gg/u/kcdrIv.mp4 |
| `room-8x-seed7` | 12.000 | 1600x900 | 3314521 | https://share.skg.gg/u/mFYboi.mp4 |

Uploading all 13 in one pass trips Zipline's rate limit after ~10 files
(`429 Rate limit exceeded, retry in ~55 seconds`, and one transient Cloudflare
`502`); the three that failed were uploaded after waiting out the window, which
is why the table above is complete.

---

## 6. What the video does NOT show

- **The connectome panel is hidden.** The right column (region bars, motor bars,
  the 3-D somata cloud, sensory bars) is fed by per-neuron data that
  `trace.csv` does not contain — there is no per-region or per-neuron series in
  a recorded run. It is hidden rather than drawn empty
  (`web/app.js`, `replayStart`). The connectome spike *totals* that are
  recorded do appear, in the left column and in the panel.
- **There is no distance-to-target trace, because no run records a target.**
  None of the runs on disk has a `fruit` block in `summary.json` (that block
  appears in a later schema), so no run records where the fruit/food place was
  and no run records a distance to it. The panel says `not recorded` instead of
  drawing a distance curve against a guessed target. The food cube in the scene
  is drawn at the visualiser's own default position
  (`web/app.js:326`, `sugar.position.set(160, 40, 44)`) — it is **not** the
  run's food place, and it is not moved during replay.
- **Environment flags are not on screen as recorded facts.** `summary.json`'s
  `config` records only `pack`, `seed`, `seconds`, `sample_every_windows`,
  `window_ms`, `dt_ms`, `samples`, `arena_mm`, `room_scale` and
  `room_height_mm`. It does **not** record which `FLYVERSE_*` flags were set.
  The caption under the panel says so, and the per-video `--label` (which
  states e.g. "FLYVERSE_NO_RETINA=1") comes from the pipeline caller reading the
  run's *directory name*, not from the run's own record.
- **The attitude is reconstructed, not recorded.** `trace.csv` stores ZYX Euler
  angles, not the quaternion the sim integrates. The driver rebuilds the
  quaternion from `yaw`/`pitch`/`roll` with the same ZYX convention the sim
  writes them with (`src/body.rs:205`, `qeuler`), so a tumble past the Euler
  chart wraps the same way it does in the telemetry.
- **The wingbeat is a visual reconstruction.** The trace records `wing_amp` but
  not `wing_phase`. The scene's own pose code therefore drives the wings from
  the recorded amplitude at its own display rate (`web/app.js:540-542`: `amp`
  read from the frame, `flapHz` derived from it, `wingPhase` advanced by `dt`);
  the real ~200 Hz wingbeat is not represented and never was, even live.
- **The trail is capped.** The scene keeps at most `TRAIL_MAX = 700` recent
  trail points (`web/app.js:348`), so the 3-D trail shows recent travel only.
  The top-down box in the evidence panel is what shows the whole path.
- **The drawn arena shell at room sizes other than 1x is scaled, not
  re-modelled.** `replaySetRoom` scales the whole room group and widens the
  camera's clamp bounds by `room_scale`/`room_height_mm`; the wall texture and
  the table are scaled with it. That matches the sim's own scaling, but it is
  still the same geometry stretched.
- **The room camera is only correct at 1x.** Its fixed outside position
  (`web/app.js:979`) is not scaled, so `default-flight-baseline-roomcam.mp4`
  uses it at 1x and every scaled room is rendered on the chase camera instead.
- **Only what the simulation itself showed.** The video is the body's
  trajectory and the recorded read-outs. It shows nothing about the network
  beyond the spike counts that were recorded, and it is a rendering of a
  surrogate body driven by the connectome, not a recording of a fly.

---

## 7. Files

```
tools/video/fly2video.sh      one run  -> one MP4   (the single command)
tools/video/render-all.sh     the standing set      (drives fly2video.sh)
tools/video/upload-zipline.sh the standing set -> Zipline, writes LINKS.md
tools/video/driver.mjs        trace replay + frame capture over CDP
tools/video/cdp.mjs           minimal DevTools Protocol client, no npm deps
tools/video/overlay.mjs       the on-screen evidence panel
tools/video/detprobe.mjs      determinism probe used to validate the capture
web/app.js                    gained the opt-in replay API (everything else unchanged)
docs/videos/LINKS.md          video -> share URL table
```

Working output (PNG sequences, browser profiles) goes to `/tmp/fvsrv/`, never
into the repo.

### Pitfalls worth keeping

- A background job that inherits the batch loop's stdin **will** consume part of
  the loop's input and desynchronise the reads, producing rows with another
  row's name and run directory. `render-all.sh` therefore materialises the spec
  list into an array before launching anything and closes stdin (`< /dev/null`)
  on every child. Row identity is checked after the fact: each render's
  `*.meta.json` records the `--run` directory it actually replayed.
- The Playwright Chrome bundle needs nix's shared libraries;
  `fly2video.sh` sets `LD_LIBRARY_PATH` from `/nix/store/*pw-chrome-libs*/lib`
  or the browser dies at launch with a bare "error while loading shared
  libraries".
- Headless Chrome here renders through software GL, so a 12 s/288-frame video
  takes ~2.5 min per job single-threaded. `--jobs 3` is about the useful limit
  on this box; more jobs slows every job down without raising throughput.
- Each job needs its own static-server port and DevTools port pair.
  `render-all.sh` assigns them from a monotone counter mod `--jobs`, so a port
  pair is only reused after the job holding it has been waited on.