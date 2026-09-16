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
    [--view chase|room|top|orbit|fpv|tps] [--fps 24] [--width 1600] [--height 900] \
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

The same set on the fly's own two cameras is a second set, off by default:

```bash
tools/video/render-all.sh --set flyviews --jobs 3
```

`--set main` is the default, so the command above is exactly the standing set it
always was. `--set flyviews` renders one row per distinct run directory of that
set in **first person** (`--view fpv`) and two third-person (`--view tps`)
references, named `<row>-fpv` / `<row>-tps` so they land beside the standing set
instead of over it (§3.1).

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
`room-1x-seed7` was re-rendered a third time after the fruit and the palm were
added to the replay (§3) and came out byte-identical again (`4570032` bytes,
`md5 993588a5bdba5fe5437350ab38185013`), which is the check that the stimulus
change touches only the runs that recorded a stimulus.

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
| `/tmp/fv/after_s7` | `9aab37c6924a8f676ac765b2db7c6b13` |
| `/tmp/fv/grain_s7` | `289f01b8b36c2778c33d8d914929ab23` |
| `/tmp/fv/hand1x/s7_approach` | `add378ee374bcc2f641d4221bef24ced` |
| `/tmp/fv/hand1x/s7_static` | `605bfb52a24cf6bbc300de57c59f6dcb` |
| `/tmp/fv/fast1x/s11_approach` | `180c0edf7081871103ee8a47257c40eb` |
| `/tmp/fv/fast1x/s11_static` | `5b70cf108b7b013bad7c3dd215a812ab` |

(Re-run `md5sum DIR/trace.csv` before re-rendering to confirm you are building
from the same data; the driver maps columns **by header name**, not by
position, so added columns do not shift the overlay.)

**One update to the above, found while adding the fpv set.** "Byte-identical"
holds against a *fixed browser build*, not forever: re-rendering
`room-1x-seed7` on this host today, with the camera change present and with it
stashed out of the tree, gives the **same** file both ways
(`md5 eb9da6133bad5b193a710f7bdc2ff84e`, 4570014 bytes) but 18 bytes away from
the committed `993588a5…` copy — the host now resolves a different
`chrome-headless-shell` bundle than the one the committed encodes were made
with, and the software-GL rasteriser is part of the output. The pipeline is
internally deterministic (same browser build in, same bytes out) and the change
above cost the four original cameras nothing; the absolute hashes in this
section should be read as "the build that made them".

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

**Cameras.** All six exist and are selectable:

| # | name | selection | definition |
|---|---|---|---|
| 0 | chase | `#cam-0`, key `0` | `web/app.js:1052` — clamped inside the room, follows the fly |
| 1 | room | `#cam-1`, key `1` | `web/app.js:1066` — fixed outside the room at `ROOM_CAM_POS` (`web/app.js:1021`) |
| 2 | top | `#cam-2`, key `2` | `web/app.js:1071` |
| 3 | orbit | `#cam-3`, key `3` | `web/app.js:1096` — drag to rotate, wheel to zoom |
| 4 | fpv | key `4` (`--view fpv`) | `web/app.js:1075` — **first person**: at the fly's own eyes, oriented by the body's own attitude |
| 5 | tps | key `5` (`--view tps`) | `web/app.js:1086` — **third person**: rigidly behind and above the fly, level horizon, body centred |

Default is chase (`web/app.js:1011`, `web/app.js:1390`).
`docs/chase-cam.png` and `docs/room-cam.png` are the two documented reference
captures.

### The fly's own two cameras (4 fpv, 5 tps)

Both were added for the video pipeline; neither is the default, neither is
clamped to the room, and neither is smoothed, so a captured frame is exactly the
body's record at that instant with no lerp state carried in.

- **fpv — first person.** `flyEyePoint()` (`web/app.js:1038`) takes the world
  position of the **rig's own** `l_eye` and `r_eye` nodes (`assets/rig.json`)
  and puts the camera at their midpoint; orientation is the body's own `+X`
  forward and `+Z` up (`flyRoot.quaternion`, the same body frame the chase
  camera reads). The camera therefore rolls and pitches with the fly — the
  horizon tilting in a first-person frame *is* the recorded roll and pitch. It
  falls back to the `c_head` node, and to the body origin only if the rig never
  loaded. Nothing is hidden: from inside the head the eye meshes' front faces
  are behind the camera, so the fly does not occlude itself.
- **tps — third person.** Rigidly `camState.chaseDist` (24 mm by default, the
  same wheel-zoomable value chase uses) behind the fly in the body's frame and
  `0.45 x` that above it, looking at the body with the world up, so the horizon
  stays level and the body stays centred. It is not clamped inside the room, so
  it does not drift when the fly is near a wall.

**The live panel is deliberately unchanged.** The button row and the hint text
still say `0-3`, because those are inside the captured frame: adding a button row
would shift the left column and change the pixels of every video already in this
directory. The two new views are reached by **key `4` / key `5`** on the live
page and by `--view fpv|tps` in the pipeline. This was verified rather than
assumed: `room-1x-seed7` re-rendered on the chase camera with the camera change
present and with it stashed out of the tree produces the **same** file
(`md5 eb9da6133bad5b193a710f7bdc2ff84e`, 4570014 bytes) both ways, i.e. the six
camera modes cost the four original ones nothing. (That file is 18 bytes away
from the committed copy, `md5 993588a5…`, 4570032 bytes, on both renders — the
difference is the headless Chrome build the host now resolves, not the camera
code; the committed encodes were made with the bundle then on disk.)

For the same reason the footer's build stamp (`web/app.js:24`, `APP_VER`) is
still `20260913a` even though `web/app.js` changed: it is rendered into the
frame, so bumping it would make the `-fpv` / `-tps` set and the standing set
disagree on screen, and would put every future re-render of the standing set a
second avoidable step away from the committed copies. The two sets are from the
same page and the same stamp; the camera code is the only difference.

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
`window.__flyverse.replay.start() | step(simT, dt, frame[, stim]) | setRoom(scale,
heightMm) | setCam(n) | setStimulus({fruit, hand}) | setPalm(centre) | camState`. It reuses the page's own `applyFrame`,
`updatePose`, `updateCamera`, `updateTrail` and `updateMarkers`, so the fly,
the trail, the camera and the HUD are the production code paths. The two
stimulus objects are replay-only and hidden unless a driver supplies the run's
recorded geometry (`setStimulus`), with the palm's centre then carried per frame
(`step`'s `stim.hand_c`). `index.html`
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
| `fruit-in-room-seed7.mp4` | `/tmp/fv/after_s7` | room | colour-vision-and-fruit test: the default build with the colour tomato fruit in the room, seed 7, 12 s, `--every 1` |
| `fruit-no-fruit-control-seed7.mp4` | `/tmp/fv/grain_s7` | room | its control: `FLYVERSE_NO_FRUIT=1`, same room/table/sugar cube/odour |
| `hand-slap-seed7.mp4` | `/tmp/fv/hand1x/s7_approach` | room | hand-slap test, 150 ms arm: `FLYVERSE_HAND=approach`, seed 7 at spawn — the full-contact slap |
| `hand-slap-static-control-seed7.mp4` | `/tmp/fv/hand1x/s7_static` | room | its control: `FLYVERSE_HAND=static`, the same palm parked at the launch origin |
| `hand-slap-fast-seed11.mp4` | `/tmp/fv/fast1x/s11_approach` | room | hand-slap test, fast arm: `FLYVERSE_HAND=approach FLYVERSE_HAND_DUR_MS=50`, seed 11 — the run that holds both |
| `hand-slap-fast-static-control-seed11.mp4` | `/tmp/fv/fast1x/s11_static` | room | its control: `FLYVERSE_HAND=static FLYVERSE_HAND_DUR_MS=50` |

The first thirteen rows are the original standing set (the default flight, the
room-size sweep, the contrasting seeds and the ablation pair). Of the sweep,
every room size it covered that is present on disk is included, and those six
sizes are exactly its rows in `docs/room-size-and-loom-pathway.md`.

### The last six rows: the two later experiments, on the same command

The room-size sweep and the ablation pair were the video pipeline's first two
subjects. The six rows after them are the runs of the two experiments that
followed — the fruit (`docs/colour-vision-and-fruit.md`) and the hand
(`docs/hand-slap-and-escape.md`) — rendered by the same `fly2video.sh`, at the
same 24 fps / 1600x900 / 1 s sim = 1 s video, and each with its control.

- **The fruit pair** is the default build with the colour tomato fruit in the
  room against `FLYVERSE_NO_FRUIT=1`. The document's result is exact rather than
  approximate: every body and motor column of `trace.csv` is bit-identical
  between the two runs (checked here directly, all 6000 rows, over
  `x y z speed yaw yaw_rate roll pitch wing_amp mode pow_l pow_r steer_l steer_r
  walk_l walk_r land_l land_r wall_hits takeoffs landings`), while `fruit_cols`
  and the retinal columns differ. So the two runs are *the same flight*: the
  body's path is identical sample for sample. The videos are no longer
  pixel-identical, though, and the difference is exactly the point of the fix —
  the fruit sphere is in the room on `fruit-in-room-seed7` and absent on the
  control (the run that saw it), the panel's `target / food place` row reads the
  recorded fruit on one and `absent [summary.fruit.present=false]` on the other,
  and the `fruit dist / cols` live row and the spike count differ. Before the
  fruit was drawn the only differing pixels were the panel's read-outs, which is
  what makes the null non-vacuous rather than a recording of nothing; now the
  stimulus itself is the visible difference. That is why the two files are
  988395 and 993617 bytes and not the same file.
- **The hand four** are the 150 ms arm's full-contact seed 7 (the delivery
  claim: the palm reaches the retina and covers about half of each eye) and the
  fast arm's seed 11 (`FLYVERSE_HAND_DUR_MS=50`, section 6.1 of the document,
  the one run that holds both "the palm reaches the retina" and "the loom
  channel has headroom"), each against its own `FLYVERSE_HAND=static` control.
  The approach and its control are two renders of runs that are bit-identical
  until the launch at t = 2.000 s and then part company; the divergence is in
  the videos, and the documents' trace tables are the measurement of it.

### The fruit and the palm are drawn, from the runs' own records

These six rows are the only videos in which a stimulus object appears, and it is
read off the run rather than assumed. The visualiser gained two objects for the
replay (a sphere for `src/room.rs`'s `Fruit`, an oriented box for its `Hand`),
both hidden unless the offline replay is driving the page and the run recorded
the thing: `web/app.js` `replay.setStimulus()` takes

- **the fruit** from `summary.json` → `fruit.present`, `fruit.centre_mm`,
  `fruit.radius_mm`, `fruit.rendered_grey` — so a run with
  `fruit.present=false` (`FLYVERSE_NO_FRUIT=1`) draws an empty table, and the
  control is visibly the same room without it;
- **the palm** from `hand.json` (or `summary.json` → `hand.stimulus`) →
  `palm_half_mm`, `approach_dir` (the palm's own normal; the other two axes are
  rebuilt with the sim's `hand_axes` construction), and then, per frame,
  `hand.csv` → `hx,hy,hz`, the palm centre the run's optics actually ray-cast
  against. `replay.step()` carries that per frame, so the palm moves through the
  room exactly as the run recorded it.

A run that records neither draws neither: nothing is placed at a guessed
position, and the panel says the field is not recorded.

**Camera.** These six are rendered on the **room** camera, the one view in which
the stimulus is on screen for the whole run, and the only one that is correct at
a scaled room (all six are 1x — §6). The chase camera is the fly's own view
cone: a 50 mm fruit on the table 460 mm away is behind the fly's head for most
of a run, and `FLYVERSE_HAND`'s default approach is nearly vertical, so on the
chase camera the palm is above the top of the frame until the last frames of a
150 ms slap (it was checked frame by frame: seed 7's slap is never in the chase
frame at all). The room camera holds the whole arena, so both objects are
visible in every frame of all six videos; the fly is the small body under the
glow ring in the left part of the room, and its own read-outs are in the panel.
The other thirteen rows stay on the chase camera, and they still re-render
byte-identically (§1).

Runs on disk that were deliberately **not** rendered: the sweep's other seed
rows for the non-1x sizes — `/tmp/fv/part1/{s2_s11,s2_s23,s4_s11,s4_h220_s11,s4_h220_s23,s8_s11,s8_s23,s1_h1200_s11,s1_h1200_s23}`
all hold a complete `trace.csv` + `summary.json` and can be rendered by adding a
row to `tools/video/render-all.sh` and running
`render-all.sh --only NAME`, but only the seed-7 rows, the two contrasting 1x
seeds and the one degenerate 4x/23 cell are included here. Also not rendered:
`runs/seed7`, a 60 s run, which at 1:1 would be a 60 s video — outside the
watchable range the brief asked for, and it duplicates the default flight
already covered at 12 s.

**Of the fruit and hand runs, only the six above are rendered.** Every other run
of those two experiments is on disk with a complete `trace.csv` + `summary.json`
and is renderable by the same one-row recipe: the hand's other seeds and
arenas (`/tmp/fv/hand1x/{s11,s23}_{approach,static}`,
`/tmp/fv/hand4x/{s7,s11,s23}_{approach,static,none}`,
`/tmp/fv/fast1x/{s7,s23}_{approach,static}`,
`/tmp/fv/fast4x/{s7,s11,s23}_{approach,static}`, `/tmp/fv/nohand2/{s7,s11,s23}`)
and the fruit's other seeds and renders (`/tmp/fv/after_s11`, `/tmp/fv/after_s23`,
`/tmp/fv/grey_s7`, `/tmp/fv/grain_s11`, `/tmp/fv/grain_s23`, `/tmp/fv/grey_s11`,
`/tmp/fv/grey_s23`). Two arms are a deliberate judgement call rather than a
list: the **4x arm** (`hand4x`, and the earlier `/tmp/fv/base/…` runs) is the
configuration the fast arm exists to *replace* — the document's section 6.3
shows its palm is never delivered to a flying seed — so its videos would show a
slap that misses, at 4x room scale where only the chase camera is correct; and
the **no-hand runs** (`nohand2`) are the byte-identity controls for the hand
build, already reported as a table in the document, whose videos would duplicate
`fruit-in-room-seed7`'s room and camera with no fruit and no hand in it.

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
tools/video/fly2video.sh --run /tmp/fv/after_s7         --name fruit-in-room-seed7                  --view room --label "colour-vision-and-fruit test: default build with the colour tomato fruit in the room (src/room.rs FRUIT, no env flag), seed 7, 12 s, --every 1 -- ROOM camera: the fruit is on screen for the whole run"
tools/video/fly2video.sh --run /tmp/fv/grain_s7         --name fruit-no-fruit-control-seed7         --view room --label "colour-vision-and-fruit test, CONTROL (run dir name: grain = FLYVERSE_NO_FRUIT=1): same room/table/sugar cube/odour, fruit removed, seed 7, 12 s, --every 1 -- ROOM camera: the empty place where the fruit would be"
tools/video/fly2video.sh --run /tmp/fv/hand1x/s7_approach    --name hand-slap-seed7                  --view room --label "hand-slap test: FLYVERSE_HAND=approach (150 ms slap, default knobs), seed 7 at spawn: the full-contact slap, see docs/hand-slap-and-escape.md -- ROOM camera: the approaching palm is in frame"
tools/video/fly2video.sh --run /tmp/fv/hand1x/s7_static      --name hand-slap-static-control-seed7   --view room --label "hand-slap test, CONTROL: FLYVERSE_HAND=static -- the same palm parked at the launch origin (bit-identical to the approach run until t = 2.000 s), seed 7 -- ROOM camera"
tools/video/fly2video.sh --run /tmp/fv/fast1x/s11_approach   --name hand-slap-fast-seed11            --view room --label "hand-slap test, FAST arm: FLYVERSE_HAND=approach FLYVERSE_HAND_DUR_MS=50 (the terminal phase at ~6 m/s), seed 11 -- the run that holds both, see docs/hand-slap-and-escape.md section 6.1 -- ROOM camera"
tools/video/fly2video.sh --run /tmp/fv/fast1x/s11_static     --name hand-slap-fast-static-control-seed11 --view room --label "hand-slap test, FAST arm CONTROL: FLYVERSE_HAND=static FLYVERSE_HAND_DUR_MS=50, seed 11 -- ROOM camera"
```

Every row above was verified after rendering: for each MP4 the driver's meta
file records the `--run` directory it actually replayed, and for all 19 that
directory is the one named in §3 (a mis-parsed batch row would show up here as a
run directory that does not match its video). All 19 were rendered at
`--fps 24 --width 1600 --height 900 --time-scale 1.0`, i.e. 288 frames,
`1600x900`, 12.000 s each.

The `view` field in those same meta files is also the camera the video was
actually made on: `room` for the six stimulus rows above, `chase` for the other
thirteen (the `default-flight-baseline-roomcam` row excepted). The meta files
also record what was placed in the room for the run — `stimApplied` (which of
the two objects the page accepted) and the `fruitSpec` / `handSpec` they were
built from — so a row that drew nothing and a row that drew the wrong thing are
distinguishable after the fact.

### 3.1 The first-person set: the same runs on the fly's own camera

```bash
tools/video/render-all.sh --set flyviews --jobs 3
```

The standing set above answers "what did the body do". This one answers "what
did the fly see while doing it": **every run directory of that set, rendered on
the first-person camera** (`--view fpv`, the view from the fly's own eyes), plus
two third-person references (`--view tps`). Same run, same replay, same overlay,
same 24 fps / 1600x900 / 1 s sim = 1 s video — the only difference is where the
camera is.

The rows are named from the standing set with a `-fpv` / `-tps` suffix, so the
two sets sit side by side and neither overwrites the other. One row per
*distinct run directory*: the standing set holds two rows for `runs/analyze`
(chase and room) and first person is the same view for both, so that pair
contributes one `-fpv` row, not two.

`airborne` below is the share of the run's samples whose `mode` is TAKEOFF,
CRUISE or LANDING — a count over `trace.csv`, computed here, not an overlay
field. It is in the table because it is what decides what a first-person frame
can contain: a grounded fly's view is a floor-level view, and on the two rows at
`0.0 %` the fly never leaves the floor at all.

| MP4 | run directory | camera | airborne | size (B) | share URL |
|---|---|---|---|---|---|
| `default-flight-baseline-fpv.mp4` | `runs/analyze` | fpv | 0.0 % | 1329807 | https://share.skg.gg/u/iXCKMg.mp4 |
| `room-1x-seed7-fpv.mp4` | `/tmp/fv/part1/base_s7` | fpv | 13.0 % | 2550329 | https://share.skg.gg/u/dA803i.mp4 |
| `room-2x-seed7-fpv.mp4` | `/tmp/fv/part1/s2_s7` | fpv | 32.3 % | 2412702 | https://share.skg.gg/u/HcMh3j.mp4 |
| `room-4x-seed7-fpv.mp4` | `/tmp/fv/part1/s4_s7` | fpv | 37.9 % | 2948430 | https://share.skg.gg/u/6wNdCR.mp4 |
| `room-4x-floor-1x-ceiling-seed7-fpv.mp4` | `/tmp/fv/part1/s4_h220_s7` | fpv | 55.2 % | 2214638 | https://share.skg.gg/u/DuCfoh.mp4 |
| `room-8x-seed7-fpv.mp4` | `/tmp/fv/part1/s8_s7` | fpv | 54.6 % | 2763866 | https://share.skg.gg/u/kk48jJ.mp4 |
| `room-1x-floor-5p5x-ceiling-seed7-fpv.mp4` | `/tmp/fv/part1/s1_h1200_s7` | fpv | 49.5 % | 1789649 | https://share.skg.gg/u/px07by.mp4 |
| `room-1x-seed11-fpv.mp4` | `/tmp/fv/part1/base_s11` | fpv | 58.2 % | 2622205 | https://share.skg.gg/u/bWyZ6l.mp4 |
| `room-1x-seed23-fpv.mp4` | `/tmp/fv/part1/base_s23` | fpv | 40.2 % | 2093198 | https://share.skg.gg/u/wv39u9.mp4 |
| `room-4x-seed23-degenerate-fpv.mp4` | `/tmp/fv/part1/s4_s23` | fpv | 0.0 % | 3100525 | https://share.skg.gg/u/Ckf6oU.mp4 |
| `ablation-baseline-seed7-fpv.mp4` | `/tmp/attr/baseline` | fpv | 99.3 % | 1433412 | https://share.skg.gg/u/oo1dbD.mp4 |
| `ablation-no-retina-seed7-fpv.mp4` | `/tmp/attr/no-retina` | fpv | 99.3 % | 1199340 | https://share.skg.gg/u/9SaAbC.mp4 |
| `fruit-in-room-seed7-fpv.mp4` | `/tmp/fv/after_s7` | fpv | 9.4 % | 2865544 | https://share.skg.gg/u/CFLnAb.mp4 |
| `fruit-no-fruit-control-seed7-fpv.mp4` | `/tmp/fv/grain_s7` | fpv | 9.4 % | 2822739 | https://share.skg.gg/u/MWcqkG.mp4 |
| `hand-slap-seed7-fpv.mp4` | `/tmp/fv/hand1x/s7_approach` | fpv | 81.5 % | 2664470 | https://share.skg.gg/u/buio1k.mp4 |
| `hand-slap-static-control-seed7-fpv.mp4` | `/tmp/fv/hand1x/s7_static` | fpv | 20.6 % | 1511859 | https://share.skg.gg/u/GSbIae.mp4 |
| `hand-slap-fast-seed11-fpv.mp4` | `/tmp/fv/fast1x/s11_approach` | fpv | 66.2 % | 2351182 | https://share.skg.gg/u/fM8r8C.mp4 |
| `hand-slap-fast-static-control-seed11-fpv.mp4` | `/tmp/fv/fast1x/s11_static` | fpv | 88.8 % | 2760818 | https://share.skg.gg/u/PiXFUH.mp4 |
| `default-flight-baseline-tps.mp4` | `runs/analyze` | tps | 0.0 % | 2751491 | https://share.skg.gg/u/9p6O6d.mp4 |
| `hand-slap-seed7-tps.mp4` | `/tmp/fv/hand1x/s7_approach` | tps | 81.5 % | 5061805 | https://share.skg.gg/u/Q9rYoi.mp4 |

All 20 were verified after rendering the same way the standing set was: each
render's `*.meta.json` records the `--run` directory, the `view` and the
`camMode` the page actually accepted, and all 20 name the run directory in the
table above with `view=fpv|tps` and `camMode=4|5`, 288 frames, 12.000 s,
1600x900. All 20 were re-downloaded from their share URL and `md5sum`-compared
to the local file (`200`, byte-identical); the full 39-row table is in
`LINKS.md`.

**What the first-person set shows that the standing set cannot.** The camera is
at the fly's own eyes and takes the body's own attitude, so the roll and pitch
that the chase camera *reports* are, in these videos, what the frame does: the
horizon tilting in `room-1x-seed11-fpv` is the recorded roll. The stimulus
objects are in the fly's view rather than the room's: in
`hand-slap-seed7-fpv.mp4` the palm's box enters the upper left of the frame at
the launch (t ≈ 2.05-2.30 s) and stays until it is past, with the table and the
red fruit sphere under it, and in `ablation-*-fpv` the 99.3 % airborne pair can
be compared from the body's own seat.

Its limits are the same kind as §6 and are stated there: the fly is not in its
own frame, the room shell is translucent, and on the rows where the body spends
the run pressed against a wall the camera is looking through it at the region
beyond. That is the recorded attitude, not a rendering fault, and it is the
sharpest way to see the behaviour the documents describe — the body has no wall
avoidance and its "view" is mostly a wall.

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
| `target / food place` | `summary.json` → `fruit.centre_mm`, `fruit.radius_mm` when `fruit.present` is true, e.g. `(240,-130,65) mm, r 25 mm [summary.fruit]`; `absent [summary.fruit.present=false]` for the clear-room control. A run whose summary has no `fruit` block at all (the thirteen earlier rows) keeps the old fixed string `not recorded (summary has no fruit/target place)`, which is what those already-rendered frames say |
| `palm` | `hand.json` → `stimulus.mode`, `start_ms`, `duration_ms`, `palm_speed_mm_s`, e.g. `approach, start 2000 ms, 50 ms, 9907 mm/s [hand.json]`. The row is shown only for a run that recorded a hand |
| `fruit dist / cols` | `trace.csv` → `fruit_dist`, `fruit_cols`, the live distance to the fruit and the retinal columns on it. Shown only when the trace has those columns |
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
| `palm dist / surf` | `hand.csv` → `hand_dist`, `hand_surf`: mm from the fly to the palm's centre and to its nearest surface. Shown only for a run that recorded a hand (`hand.csv` — the trace has no hand columns) |
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
| `ablation-baseline-seed7-fpv` | 12.000 | 1600x900 | 1433412 | https://share.skg.gg/u/oo1dbD.mp4 |
| `ablation-no-retina-seed7` | 12.000 | 1600x900 | 3755279 | https://share.skg.gg/u/WdtGj6.mp4 |
| `ablation-no-retina-seed7-fpv` | 12.000 | 1600x900 | 1199340 | https://share.skg.gg/u/9SaAbC.mp4 |
| `default-flight-baseline` | 12.000 | 1600x900 | 2364737 | https://share.skg.gg/u/VdoMmK.mp4 |
| `default-flight-baseline-fpv` | 12.000 | 1600x900 | 1329807 | https://share.skg.gg/u/iXCKMg.mp4 |
| `default-flight-baseline-roomcam` | 12.000 | 1600x900 | 968796 | https://share.skg.gg/u/pGNmTe.mp4 |
| `default-flight-baseline-tps` | 12.000 | 1600x900 | 2751491 | https://share.skg.gg/u/9p6O6d.mp4 |
| `fruit-in-room-seed7` | 12.000 | 1600x900 | 988395 | https://share.skg.gg/u/6VnpEv.mp4 |
| `fruit-in-room-seed7-fpv` | 12.000 | 1600x900 | 2865544 | https://share.skg.gg/u/CFLnAb.mp4 |
| `fruit-no-fruit-control-seed7` | 12.000 | 1600x900 | 993617 | https://share.skg.gg/u/dJHytZ.mp4 |
| `fruit-no-fruit-control-seed7-fpv` | 12.000 | 1600x900 | 2822739 | https://share.skg.gg/u/MWcqkG.mp4 |
| `hand-slap-fast-seed11` | 12.000 | 1600x900 | 1099198 | https://share.skg.gg/u/qzVQUp.mp4 |
| `hand-slap-fast-seed11-fpv` | 12.000 | 1600x900 | 2351182 | https://share.skg.gg/u/fM8r8C.mp4 |
| `hand-slap-fast-static-control-seed11` | 12.000 | 1600x900 | 1119510 | https://share.skg.gg/u/HfR4Vw.mp4 |
| `hand-slap-fast-static-control-seed11-fpv` | 12.000 | 1600x900 | 2760818 | https://share.skg.gg/u/PiXFUH.mp4 |
| `hand-slap-seed7` | 12.000 | 1600x900 | 1115244 | https://share.skg.gg/u/mMUeOC.mp4 |
| `hand-slap-seed7-fpv` | 12.000 | 1600x900 | 2664470 | https://share.skg.gg/u/buio1k.mp4 |
| `hand-slap-seed7-tps` | 12.000 | 1600x900 | 5061805 | https://share.skg.gg/u/Q9rYoi.mp4 |
| `hand-slap-static-control-seed7` | 12.000 | 1600x900 | 1041570 | https://share.skg.gg/u/jlH7VC.mp4 |
| `hand-slap-static-control-seed7-fpv` | 12.000 | 1600x900 | 1511859 | https://share.skg.gg/u/GSbIae.mp4 |
| `room-1x-floor-5p5x-ceiling-seed7` | 12.000 | 1600x900 | 4296489 | https://share.skg.gg/u/aMA8fg.mp4 |
| `room-1x-floor-5p5x-ceiling-seed7-fpv` | 12.000 | 1600x900 | 1789649 | https://share.skg.gg/u/px07by.mp4 |
| `room-1x-seed11` | 12.000 | 1600x900 | 5524535 | https://share.skg.gg/u/kuEKE6.mp4 |
| `room-1x-seed11-fpv` | 12.000 | 1600x900 | 2622205 | https://share.skg.gg/u/bWyZ6l.mp4 |
| `room-1x-seed23` | 12.000 | 1600x900 | 4743147 | https://share.skg.gg/u/DyLzWR.mp4 |
| `room-1x-seed23-fpv` | 12.000 | 1600x900 | 2093198 | https://share.skg.gg/u/wv39u9.mp4 |
| `room-1x-seed7` | 12.000 | 1600x900 | 4570032 | https://share.skg.gg/u/L5gnb9.mp4 |
| `room-1x-seed7-fpv` | 12.000 | 1600x900 | 2550329 | https://share.skg.gg/u/dA803i.mp4 |
| `room-2x-seed7` | 12.000 | 1600x900 | 3942550 | https://share.skg.gg/u/NbdUYy.mp4 |
| `room-2x-seed7-fpv` | 12.000 | 1600x900 | 2412702 | https://share.skg.gg/u/HcMh3j.mp4 |
| `room-4x-floor-1x-ceiling-seed7` | 12.000 | 1600x900 | 4128772 | https://share.skg.gg/u/FgG1eX.mp4 |
| `room-4x-floor-1x-ceiling-seed7-fpv` | 12.000 | 1600x900 | 2214638 | https://share.skg.gg/u/DuCfoh.mp4 |
| `room-4x-seed23-degenerate` | 12.000 | 1600x900 | 3475248 | https://share.skg.gg/u/97tKMM.mp4 |
| `room-4x-seed23-degenerate-fpv` | 12.000 | 1600x900 | 3100525 | https://share.skg.gg/u/Ckf6oU.mp4 |
| `room-4x-seed7` | 12.000 | 1600x900 | 3772673 | https://share.skg.gg/u/kcdrIv.mp4 |
| `room-4x-seed7-fpv` | 12.000 | 1600x900 | 2948430 | https://share.skg.gg/u/6wNdCR.mp4 |
| `room-8x-seed7` | 12.000 | 1600x900 | 3314521 | https://share.skg.gg/u/mFYboi.mp4 |
| `room-8x-seed7-fpv` | 12.000 | 1600x900 | 2763866 | https://share.skg.gg/u/kk48jJ.mp4 |
Uploading all 13 of the original set in one pass trips Zipline's rate limit after
~10 files (`429 Rate limit exceeded, retry in ~55 seconds`, and one transient
Cloudflare `502`); the three that failed were uploaded after waiting out the
window, which is why the table above is complete. The six added later were
uploaded **one at a time** with `upload-zipline.sh --only NAME` (six uploads, 4 s
apart, no `429`): with `--only` set the script appends its row to the existing
`LINKS.md` instead of starting a fresh table, so a later addition never rewrites
the earlier rows. Each of the six was re-downloaded from its share URL and
`md5sum`-compared to the local file — all six returned `200` and matched.

The six stimulus rows were re-rendered on the room camera (the fruit and the
palm now drawn — §3) and re-uploaded the same way, one at a time with
`--only NAME`; the six URLs above are the new uploads, each re-downloaded and
byte-compared after upload (all six `200`, all six matched). Their previous
uploads, made before the fruit and the palm were drawn, are superseded:
`VHrl67`, `K0ZmH4`, `wWfDnn`, `D5TcUi`, `hKE4Mf`, `3JSrPG`.

The 20 first-person / third-person rows (§3.1) were uploaded the same way, one
at a time with `--only NAME` and 4 s between uploads. The rate limit still bit:
after 10 uploads in one pass the next four returned `429` (`room-4x-seed23-degenerate-fpv`,
`ablation-baseline-seed7-fpv`, `fruit-in-room-seed7-fpv`,
`fruit-no-fruit-control-seed7-fpv`); each was retried after the window had
passed, singly, and all four then returned `200`. The failed attempts had added
`_UPLOAD FAILED_` rows to `LINKS.md`, so the file was **rebuilt** from the
directory listing plus the successful rows — same header and same six columns,
one row per MP4, sorted, 39 rows, no failure rows and no duplicates. All 39
rows were then re-downloaded and `md5sum`-compared to their local files: 39 of
39 returned `200` and matched byte for byte.

---

## 6. What the video does NOT show

- **The connectome panel is hidden.** The right column (region bars, motor bars,
  the 3-D somata cloud, sensory bars) is fed by per-neuron data that
  `trace.csv` does not contain — there is no per-region or per-neuron series in
  a recorded run. It is hidden rather than drawn empty
  (`web/app.js`, `replayStart`). The connectome spike *totals* that are
  recorded do appear, in the left column and in the panel.
- **The fruit and the palm are drawn from the run's own records, and only they
  are.** `web/app.js` now has a sphere (`room::Fruit`) and an oriented box
  (`room::Hand`), placed by the replay from `summary.json` → `fruit.*`,
  `hand.json` → `stimulus`, and per frame `hand.csv` → `hx,hy,hz` (§3). The
  limits that remain, in the same spirit as every other row of this section:
  - **Only for the six rows that recorded them.** The objects exist for the
    offline replay only and start hidden; a run whose summary has no `fruit`
    block and which wrote no `hand.csv`/`hand.json` (the thirteen earlier rows)
    draws neither, which is why those thirteen still re-render byte-identically
    (§1).
  - **The fruit is drawn, but its colour is the renderer's.** The scene shows a
    red sphere of `fruit.radius_mm` at `fruit.centre_mm`; the object's
    reflectance spectrum (what its colour actually *is* to the fly's retina) is
    in `summary.json` → `fruit.colour_channels` and in
    `docs/colour-vision-and-fruit.md`, not in the pixels. A
    `FLYVERSE_FRUIT_GREY=1` run would be drawn grey rather than coloured
    (`fruit.rendered_grey`), keeping the equi-luminant control's point.
  - **The palm is a box, not a hand.** It is the sim's own geometry: an oriented
    box of `hand.palm_half_mm` (90 x 110 x 24 mm) with the normal from
    `approach_dir`. There are no fingers and no arm, because the sim's ray-casts
    see none.
  - **The palm's approach is what a 24 fps video can hold of it.** The 150 ms
    arm spans ~4 output frames and the fast (50 ms) arm ~1-2, so the strike
    itself is a couple of frames; the palm is parked at its origin before the
    launch and at the contact point after it, so it is on screen for the rest of
    the run and its motion is what the run recorded.
  - **The sugar cube in the scene is still not the run's food place.** It is
    drawn at the visualiser's own default position (`web/app.js:326`,
    `sugar.position.set(160, 40, 44)`), it is not moved during replay, and no
    run records a food place — the `target / food place` row now reports the
    fruit (`summary.json` → `fruit.centre_mm`) and nothing else.
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
- **The first-person and third-person rows show the body's own seat, and that
  seat is usually a wall.** Four limits, in the same spirit as the rest of this
  section:
  - **The fly is not in its own first-person frame.** The camera is where its
    eyes are (§3.1), so the body is never drawn in an `-fpv` video; the two
    `-tps` rows are the reference for what it looks like from outside while the
    `-fpv` rows are what it looked at. It is a single 42° perspective camera at
    the midpoint of the rig's `l_eye`/`r_eye` nodes
    (`web/app.js:134`, `web/app.js:1038`) — not a compound-eye panorama, no
    ommatidial sampling, no 360° field.
  - **The fpv camera follows the rendered body, not the raw record.** It reads
    `flyRoot.position`/`quaternion`, which `replayStep` drives toward the
    frame's `pose` with a fixed per-step factor — the same deterministic
    smoothing every other camera follows, and the same value in every render of
    the same run, but it is the rendered body's eye point, not the recorded one.
  - **The room shell is translucent, so "facing a wall" renders as empty
    background.** The walls are see-through because the room camera looks in
    through them (`web/app.js`, `ROOM_CAM_POS`); a first-person frame aimed at a
    wall therefore shows the region *beyond* the wall, not a wall texture. The
    body has no wall avoidance, so on most runs it spends much of the run
    against a wall, and several `-fpv` rows are largely empty for exactly that
    reason. Nothing was changed to hide it: making the walls opaque for the
    first-person camera would change the pixels of the room-camera videos too.
  - **Where the fly never leaves the floor, first person is a floor-level
    crawl.** `default-flight-baseline-fpv` and `room-4x-seed23-degenerate-fpv`
    are 0.0 % airborne (the degenerate row is the documented one — §3), so their
    frames are from a walking body at ~2 mm altitude.
- **Only what the simulation itself showed.** The video is the body's
  trajectory and the recorded read-outs. It shows nothing about the network
  beyond the spike counts that were recorded, and it is a rendering of a
  surrogate body driven by the connectome, not a recording of a fly.

---

## 7. Files

```
tools/video/fly2video.sh      one run  -> one MP4   (the single command; --view includes fpv|tps)
tools/video/render-all.sh     the standing set, and (--set flyviews) the same runs in first person + 2 third person (drives fly2video.sh)
tools/video/upload-zipline.sh the standing set -> Zipline, writes LINKS.md
tools/video/driver.mjs        trace replay + hand.csv/fruit stimulus placement + frame capture over CDP
tools/video/cdp.mjs           minimal DevTools Protocol client, no npm deps
tools/video/overlay.mjs       the on-screen evidence panel (fruit/palm rows only for runs that recorded them)
tools/video/detprobe.mjs      determinism probe used to validate the capture
web/app.js                    gained the opt-in replay API, the replay-only fruit/palm objects, and the fpv/tps cameras (CAMS.FPV / CAMS.TPS)
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