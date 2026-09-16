#!/usr/bin/env node
// Replay one recorded flyverse run through the project's own three.js visualiser
// and write a frame-perfect PNG sequence, which the shell wrapper encodes to MP4.
//
// Why replay a trace instead of screen-recording the live server: the live page
// is driven by wall-clock SSE pushes, so a capture would miss frames and would
// not be reproducible. Here every output frame is tied to a simulated time
// (frame k is the trace interpolated at t0 + k*dt), the page's rAF loop is
// stopped and the renderer is stepped by hand, so the same run always produces
// the same frames. Nothing is dropped: the frame count is printed and every
// write is checked.
//
// Usage: driver.mjs --run DIR --frames DIR --json META.json [--view chase|room|top|orbit]
//                   [--fps 24] [--width 1600] [--height 900] [--time-scale 1.0]
//                   [--cdp http://127.0.0.1:9333] [--url http://127.0.0.1:8137/index.html]

import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { CDP, firstPageTarget } from './cdp.mjs';
import { OVERLAY_JS } from './overlay.mjs';

const MODES = ['GROUND', 'TAKEOFF', 'CRUISE', 'LANDING', 'FEEDING'];
const VIEWS = { chase: 0, room: 1, top: 2, orbit: 3 };

function parseArgs(argv) {
  const a = {};
  for (let i = 0; i < argv.length; i++) {
    const k = argv[i];
    if (k.startsWith('--')) a[k.slice(2)] = argv[i + 1];
  }
  return a;
}

const args = parseArgs(process.argv.slice(2));
const runDir = args.run;
const frameDir = args.frames;
const metaPath = args.json;
const view = (args.view || 'chase').toLowerCase();
const fps = Number(args.fps || 24);
const width = Number(args.width || 1600);
const height = Number(args.height || 900);
const timeScale = Number(args['time-scale'] || 1.0);
const label = args.label || '';
const cdpHttp = args.cdp || 'http://127.0.0.1:9333';
const pageUrl = args.url || 'http://127.0.0.1:8137/index.html?skip=brain,spikes';
const simStartArg = args['sim-start'] !== undefined ? Number(args['sim-start']) : null;
const simEndArg = args['sim-end'] !== undefined ? Number(args['sim-end']) : null;

if (!runDir || !frameDir || !metaPath) {
  console.error('driver: --run, --frames and --json are required');
  process.exit(2);
}
if (!(view in VIEWS)) {
  console.error('driver: --view must be one of ' + Object.keys(VIEWS).join('|'));
  process.exit(2);
}

// ---------------------------------------------------------------- read the run
function readCsv(path) {
  const text = readFileSync(path, 'utf8');
  const lines = text.split(/\r?\n/).filter((l) => l.length && !l.startsWith('#'));
  const head = lines[0].split(',');
  const rows = [];
  for (let i = 1; i < lines.length; i++) {
    const parts = lines[i].split(',');
    if (parts.length !== head.length) continue;
    const row = {};
    for (let c = 0; c < head.length; c++) row[head[c]] = Number(parts[c]);
    rows.push(row);
  }
  return { head, rows };
}

const trace = readCsv(join(runDir, 'trace.csv'));
const summary = JSON.parse(readFileSync(join(runDir, 'summary.json'), 'utf8'));
const cfg = summary.config || {};
const arena = cfg.arena_mm || cfg.room || null;

// ------------------------------------------------- stimuli recorded by the run
// Two later experiments put objects in the room and record them per run:
//
//   * the fruit: summary.json `fruit` (present, centre_mm, radius_mm,
//     rendered_grey, distance_mm, optics), and trace.csv columns fruit_dist /
//     fruit_az / fruit_cols;
//   * the hand: summary.json + hand.json `stimulus` (palm_half_mm, origin_mm,
//     contact_mm, start_ms, duration_ms, mode, approach_dir) and the per-sample
//     palm centre in hand.csv (hx, hy, hz).
//
// A run that records none of it draws neither object and the panel says the
// field is not recorded. Nothing is placed at an assumed position.
const fruit = summary.fruit || null;
const handTrace = (function () {
  try { return readCsv(join(runDir, 'hand.csv')); } catch (e) { return null; }
})();
const handStim = (function () {
  try {
    const hj = JSON.parse(readFileSync(join(runDir, 'hand.json'), 'utf8'));
    if (hj && hj.stimulus) return hj.stimulus;
  } catch (e) { /* no hand.json: the summary's hand block carries the same stimulus block */ }
  return summary.hand && summary.hand.stimulus ? summary.hand.stimulus : null;
})();

const num = (x) => (typeof x === 'number' && isFinite(x) ? x : null);
const mm1 = (x) => (num(x) === null ? '?' : (Math.round(x * 10) / 10).toString());
const vecText = (v) => (Array.isArray(v) && v.length === 3 ? '(' + v.map(mm1).join(',') + ')' : '?');

// Palm frame: `approach_dir` in the records is the palm's own normal (axes[2]),
// the direction it presents to the fly (src/room.rs `hand_axes` / `describe`).
// The other two axes are rebuilt here with the sim's own construction:
// width = z x normal, length = normal x width.
function cross3(a, b) {
  return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
}
function unit3(v) {
  const n = Math.hypot(v[0], v[1], v[2]) || 1;
  return [v[0] / n, v[1] / n, v[2] / n];
}
function handAxes(normal) {
  const n = unit3(normal);
  let w = cross3([0, 0, 1], n);
  if (Math.hypot(w[0], w[1], w[2]) < 1e-3) w = cross3([1, 0, 0], n);
  const wu = unit3(w);
  return [wu, unit3(cross3(n, wu)), n];
}

const fruitSpec = fruit && fruit.present === true && Array.isArray(fruit.centre_mm) && num(fruit.radius_mm)
  ? { c: fruit.centre_mm.map((x) => Number(x)), r: Number(fruit.radius_mm), grey: !!fruit.rendered_grey }
  : null;
const handSpec = handStim && Array.isArray(handStim.approach_dir) && Array.isArray(handStim.palm_half_mm)
  ? {
      half: handStim.palm_half_mm.map((x) => Number(x)),
      axes: handAxes(handStim.approach_dir.map((x) => Number(x))),
      mode: handStim.mode,
    }
  : null;

// The `target / food place` row, from the run's own fruit block. Runs that
// predate that block keep the old wording -- byte for byte, because their videos
// are already rendered and that string is what those frames say.
//
// The fruit is the only target place any run records; the sugar cube's place is
// not a recorded field (the scene draws it at the visualiser's own default), so
// the row reports the fruit and nothing else.
const targetText = (function () {
  if (!fruit) return 'not recorded (summary has no fruit/target place)';
  if (fruit.present !== true) return 'absent [summary.fruit.present=false]';
  const r = num(fruit.radius_mm) === null ? 'r not recorded' : 'r ' + mm1(fruit.radius_mm) + ' mm';
  const c = Array.isArray(fruit.centre_mm) ? vecText(fruit.centre_mm) + ' mm, ' : '';
  return c + r + ' [summary.fruit]';
})();

const palmText = handStim
  ? (handStim.mode === 'approaching' ? 'approach' : String(handStim.mode)) +
    ', start ' + mm1(handStim.start_ms) + ' ms, ' + mm1(handStim.duration_ms) + ' ms, ' +
    mm1(handStim.palm_speed_mm_s) + ' mm/s [hand.json]'
  : null;

const t0All = trace.rows[0].t;
const t1All = trace.rows[trace.rows.length - 1].t;
const tStart = simStartArg !== null ? simStartArg : t0All;
const tEnd = simEndArg !== null ? simEndArg : t1All;
const simSpan = Math.max(1e-3, tEnd - tStart);
const nFrames = Math.max(2, Math.round(simSpan * timeScale * fps));
const step = simSpan / nFrames;              // simulated seconds per output frame
const videoSeconds = nFrames / fps;

// summary.json config keys, as recorded by this build of the analyser. Anything
// the run did not record is reported as absent rather than guessed.
const roomScale = typeof cfg.room_scale === 'number' ? cfg.room_scale : 1;
const roomHeight = typeof cfg.room_height_mm === 'number' ? cfg.room_height_mm : null;
const roomText = arena
  ? (arena.x[1] - arena.x[0]) + 'x' + (arena.y[1] - arena.y[0]) + 'x' + (arena.z[1] - arena.z[0]) +
    ' mm  (scale ' + roomScale + ', height ' + (roomHeight === null ? 'scaled' : roomHeight + ' mm') + ')'
  : 'not recorded in summary.json config';
const configKeys = Object.keys(cfg).sort().join(', ');
const flagsText = 'flags: summary.json records only [' + configKeys +
  ']; env flags (e.g. FLYVERSE_NO_FRUIT, FLYVERSE_ROOM_SCALE) are NOT stored, so the ' +
  'label below is supplied by the pipeline caller from the run name.';

const meta = {
  run: runDir,
  runName: runDir.replace(/\/+$/, '').split('/').pop(),
  seed: cfg.seed,
  seconds: cfg.seconds,
  room: roomText,
  target: targetText,
  pack: cfg.pack,
  every: (cfg.sample_every_windows !== undefined ? cfg.sample_every_windows + ' control windows (' + cfg.window_ms + ' ms each)' : 'not recorded'),
  timeScale,
  fps,
  frames: nFrames,
  videoSeconds,
  view,
  label,
  flags: flagsText,
  traceFile: join(runDir, 'trace.csv'),
  traceColumns: trace.head.length,
  traceSamples: trace.rows.length,
  simSpan,
  roomScale,
  roomHeight,
  // What the replay actually placed in the room, from the run's own records.
  // Null means the run recorded neither the object nor its place, and the
  // renderer drew nothing there.
  palmText,
  fruitLive: trace.head.indexOf('fruit_dist') >= 0 && trace.head.indexOf('fruit_cols') >= 0,
  handLive: !!handTrace,
  fruitSpec,
  handSpec,
};

// ---------------------------------------------------------------- interpolation
// Linear interpolation between the two trace samples that bracket `t`. The trace
// carries no quaternion, so the attitude is rebuilt from the recorded ZYX Euler
// angles with the same ZYX convention the sim writes them with (body.rs:qeuler).
function qmul(a, b) {
  return [
    a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
    a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
    a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
    a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
  ];
}
function quatFromZyx(yaw, pitch, roll) {
  const qz = [Math.cos(yaw / 2), 0, 0, Math.sin(yaw / 2)];
  const qy = [Math.cos(pitch / 2), 0, Math.sin(pitch / 2), 0];
  const qx = [Math.cos(roll / 2), Math.sin(roll / 2), 0, 0];
  const q = qmul(qmul(qz, qy), qx);
  const n = Math.hypot(q[0], q[1], q[2], q[3]) || 1;
  return [q[0] / n, q[1] / n, q[2] / n, q[3] / n];
}

let cursor = 0;
function sampleAt(t) {
  const rows = trace.rows;
  while (cursor > 0 && rows[cursor].t > t) cursor--;
  while (cursor < rows.length - 1 && rows[cursor + 1].t <= t) cursor++;
  const a = rows[cursor];
  const b = rows[Math.min(cursor + 1, rows.length - 1)];
  const span = b.t - a.t;
  const f = span > 1e-9 ? Math.min(1, Math.max(0, (t - a.t) / span)) : 0;
  const mix = (k) => (a[k] === undefined || b[k] === undefined ? 0 : a[k] + (b[k] - a[k]) * f);
  return { a, b, f, mix, raw: a };
}

// Series for the overlay's sparkline and top-down path. Downsampled to keep the
// injected payload small; the playhead reads the interpolated value, not this.
const maxPts = 1500;
const stride = Math.max(1, Math.floor(trace.rows.length / maxPts));
const ser = { t: [], speed: [], steerDiff: [], x: [], y: [] };
for (let i = 0; i < trace.rows.length; i += stride) {
  const r = trace.rows[i];
  ser.t.push(r.t);
  ser.speed.push(r.speed);
  ser.steerDiff.push(r.steer_l - r.steer_r);
  ser.x.push(r.x); ser.y.push(r.y);
}
const last = trace.rows[trace.rows.length - 1];
if (ser.t[ser.t.length - 1] !== last.t) {
  ser.t.push(last.t); ser.speed.push(last.speed);
  ser.steerDiff.push(last.steer_l - last.steer_r); ser.x.push(last.x); ser.y.push(last.y);
}
ser.arena = arena ? { x: arena.x, y: arena.y } : { x: [-300, 300], y: [-220, 220] };

// The palm's centre at time `t`, mm, world frame: hand.csv's hx,hy,hz, which is
// where the run's own optics ray-cast against it. Its own cursor, like the
// trace's. If a run records the hand but not the per-sample file (hand.json
// only), the recorded trajectory is walked instead: origin before the launch,
// straight to contact over duration_ms, contact after it, and parked at the
// origin for the static control -- the same rule as `HandTraj::hand_at`.
let hcur = 0;
function handFrame(t) {
  if (handTrace && handTrace.rows.length) {
    const rows = handTrace.rows;
    while (hcur > 0 && rows[hcur].t > t) hcur--;
    while (hcur < rows.length - 1 && rows[hcur + 1].t <= t) hcur++;
    const a = rows[hcur];
    const b = rows[Math.min(hcur + 1, rows.length - 1)];
    const span = b.t - a.t;
    const f = span > 1e-9 ? Math.min(1, Math.max(0, (t - a.t) / span)) : 0;
    const mix = (k) => (a[k] === undefined || b[k] === undefined ? 0 : a[k] + (b[k] - a[k]) * f);
    return { c: [mix('hx'), mix('hy'), mix('hz')], mix };
  }
  if (!handStim) return null;
  const o = handStim.origin_mm.map((x) => Number(x));
  const c = handStim.contact_mm.map((x) => Number(x));
  if (handStim.mode !== 'approaching') return { c: o, mix: null };
  const start = Number(handStim.start_ms) / 1000;
  const dur = Math.max(1e-6, Number(handStim.duration_ms) / 1000);
  const u = Math.min(1, Math.max(0, (t - start) / dur));
  return { c: [0, 1, 2].map((i) => o[i] + (c[i] - o[i]) * u), mix: null };
}

// ---------------------------------------------------------------- frame payload
function frameOf(t, idx) {
  const s = sampleAt(t);
  const x = s.mix('x'), y = s.mix('y'), z = s.mix('z');
  const yaw = s.mix('yaw'), pitch = s.mix('pitch'), roll = s.mix('roll');
  const modeCode = Math.round(s.mix('mode'));
  const mode = MODES[modeCode] || 'GROUND';
  const bump = (k) => Math.round(s.mix(k));      // counters are monotone step functions
  const body = {
    pos: [x, y, z],
    quat: quatFromZyx(yaw, pitch, roll),
    mode,
    airborne: modeCode >= 1 && modeCode <= 3,     // sim::airborne(): Takeoff|Cruise|Landing
    speed: s.mix('speed'),
    pitch, roll, yaw,
    wing_amp: s.mix('wing_amp'),
    // legs_supported is not a trace column, so it is deliberately omitted: the
    // HUD shows '-' rather than a number nobody recorded.
  };
  const has = (k) => s.a[k] !== undefined;
  const motor = (k) => (has(k) ? s.mix(k) : undefined);
  const payload = {
    seq: idx,
    sim_ms: t * 1000,
    wall_rt: null,
    paused: false,
    stimulus: 'replay (trace.csv)',
    body,
    motors: {
      flight_power_l: motor('pow_l'), flight_power_r: motor('pow_r'),
      flight_steer_l: motor('steer_l'), flight_steer_r: motor('steer_r'),
      walk_l: motor('walk_l'), walk_r: motor('walk_r'),
      land_l: motor('land_l'), land_r: motor('land_r'),
      mn9: motor('mn9'),
    },
    neural: {
      // Only what the trace actually holds.
      spikes_window: motor('win_spikes'), spikes_total: motor('tot_spikes'),
      active_neurons: null, mean_rate_hz: null,
      takeoff_drive: motor('dn_filt'), hall: motor('sapp'),
      odor_l: motor('odor_l'), odor_r: motor('odor_r'),
      flow_l: motor('flow_l'), flow_r: motor('flow_r'),
    },
    events: {
      eats: bump('eats'), takeoffs: bump('takeoffs'),
      landings: bump('landings'), wall_hits: bump('wall_hits'),
    },
  };
  const overlay = {
    t, mode, airborne: body.airborne, z, x, y,
    speed: s.mix('speed'), yawRate: s.mix('yaw_rate'),
    steerL: motor('steer_l') || 0, steerR: motor('steer_r') || 0,
    steerDiff: (motor('steer_l') || 0) - (motor('steer_r') || 0),
    powL: motor('pow_l') || 0, powR: motor('pow_r') || 0,
    wingAmp: s.mix('wing_amp'),
    wallDist: s.mix('wall_dist'),
    wallHits: bump('wall_hits'), takeoffs: bump('takeoffs'), landings: bump('landings'),
    loom: motor('loom') || 0, odorL: motor('odor_l') || 0, odorR: motor('odor_r') || 0,
    flowL: motor('flow_l') || 0, flowR: motor('flow_r') || 0,
    winSpikes: bump('win_spikes'), totSpikes: bump('tot_spikes'),
  };
  // The fruit's and the palm's own live read-outs, added to the panel only for
  // runs that recorded them (meta.fruitLive / meta.handLive, above).
  if (meta.fruitLive) {
    overlay.fruitDist = s.mix('fruit_dist');
    overlay.fruitCols = Math.round(s.mix('fruit_cols'));
  }
  let stim = null;
  if (handTrace || handStim) {
    const hf = handFrame(t);
    if (hf) {
      if (hf.mix) {
        overlay.palmDist = hf.mix('hand_dist');
        overlay.palmSurf = hf.mix('hand_surf');
      }
      stim = { hand_c: hf.c };
    }
  }
  return { payload, overlay, stim };
}

// ---------------------------------------------------------------- drive the page
mkdirSync(frameDir, { recursive: true });
const target = await firstPageTarget(cdpHttp);
const cdp = new CDP(target.webSocketDebuggerUrl);
await cdp.connect();
await cdp.send('Page.enable');
await cdp.send('Runtime.enable');
await cdp.send('Emulation.setDeviceMetricsOverride', {
  width, height, deviceScaleFactor: 1, mobile: false,
});

const loaded = new Promise((resolve) => cdp.on('Page.loadEventFired', resolve));
await cdp.send('Page.navigate', { url: pageUrl });
await loaded;

// Wait for the fly rig (rig.json + ~40 STL meshes) before stepping anything.
let ready = false;
for (let i = 0; i < 600; i++) {
  ready = await cdp.eval('!!(window.__flyverse && window.__flyverse.replay && window.__flyverse.anim && window.__flyverse.anim.loaded)');
  if (ready) break;
  await new Promise((r) => setTimeout(r, 100));
}
if (!ready) {
  const diag = await cdp.eval('JSON.stringify({booted: !!window.__fvBooted, err: window.__fvErrors || [], fly: !!(window.__flyverse && window.__flyverse.replay)})');
  throw new Error('visualiser rig never became ready: ' + diag);
}
await cdp.eval('window.__flyverse.replay.start()');
const roomApplied = await cdp.eval(`window.__flyverse.replay.setRoom(${roomScale}, ${roomHeight === null ? 'null' : roomHeight})`);
const camMode = await cdp.eval(`window.__flyverse.replay.setCam(${VIEWS[view]})`);
// The run's own fruit and palm, or nothing at all when it recorded neither. The
// objects the page draws are the run's recorded geometry: `fruit.radius_mm` and
// `hand.palm_half_mm` come from the files, not from the renderer.
const stimApplied = await cdp.eval('window.__flyverse.replay.setStimulus(' +
  JSON.stringify({ fruit: fruitSpec, hand: handSpec }) + ')');
if (fruitSpec || handSpec) {
  process.stderr.write('[driver] stimulus placed: fruit=' + JSON.stringify(!!stimApplied.fruit) +
    ' hand=' + JSON.stringify(!!stimApplied.hand) + '\n');
}

await cdp.eval(OVERLAY_JS);
await cdp.eval('window.__fvev.series(' + JSON.stringify(ser) + ')');
await cdp.eval('window.__fvev.meta(' + JSON.stringify(meta) + ')');

// Deterministic camera warm-up: settle the chase lerp on the first frame before
// the first capture, so frame 0 is not a swoop in from the scene default.
const warm = frameOf(tStart, 0);
for (let i = 0; i < 60; i++) {
  await cdp.eval('window.__flyverse.replay.step(' + tStart + ',' + (1 / 60) +
    ',' + JSON.stringify(warm.payload) + ',' + JSON.stringify(warm.stim || null) + ')');
}

process.stderr.write(`[driver] ${nFrames} frames, ${step.toFixed(5)} s sim/frame, view=${view} (cam ${camMode}), room=${JSON.stringify(roomApplied)}\n`);

const pad = (n) => String(n).padStart(6, '0');
for (let k = 0; k < nFrames; k++) {
  const tk = k === nFrames - 1 ? tEnd : tStart + step * (k + 1);
  const { payload, overlay, stim } = frameOf(tk, k);
  await cdp.eval('window.__flyverse.replay.step(' + tk + ',' + step + ',' + JSON.stringify(payload) +
    ',' + JSON.stringify(stim || null) + ')');
  await cdp.eval('window.__fvev.set(' + JSON.stringify(overlay) + ')');
  const shot = await cdp.send('Page.captureScreenshot', { format: 'png', fromSurface: true });
  writeFileSync(join(frameDir, pad(k) + '.png'), Buffer.from(shot.data, 'base64'));
  if (k % 50 === 0 || k === nFrames - 1) {
    process.stderr.write(`[driver] frame ${k + 1}/${nFrames} t=${tk.toFixed(3)}s\n`);
  }
}

const pageErrors = await cdp.eval('JSON.stringify(window.__fvev.errors ? window.__fvev.errors() : [])');
cdp.close();

const out = {
  ...meta,
  framesWritten: nFrames,
  stepSeconds: step,
  tStart, tEnd,
  simPanelsStart: tStart, simSpan,
  camMode,
  roomApplied,
  stimApplied,
  pageErrors: JSON.parse(pageErrors),
};
writeFileSync(metaPath, JSON.stringify(out, null, 2));
process.stderr.write('[driver] wrote ' + nFrames + ' PNG frames to ' + frameDir + '\n');